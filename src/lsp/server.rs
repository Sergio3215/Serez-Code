// The LSP server proper: a synchronous JSON-RPC loop over stdio.
// One analysis pass per didOpen/didChange (full-document sync); requests
// (completion/hover/definition/documentSymbol) answer from the last analysis.
use super::analysis::{self, Analysis, SymbolKind};
use super::builtins;
use super::rpc;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;

pub fn run() -> i32 {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut server = Server::default();

    while let Some(body) = rpc::read_message(&mut input) {
        let message: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if server.handle(&message, &mut output) {
            break; // exit notification
        }
    }
    if server.got_shutdown { 0 } else { 1 }
}

/// A symbol contributed by an imported file, remembering where it lives.
struct ImportedSym {
    sym: analysis::SymbolInfo,
    uri: String,
}

#[derive(Default)]
struct Server {
    docs: HashMap<String, Analysis>,
    /// Per open document: symbols gathered from its (transitive) imports.
    doc_imports: HashMap<String, Vec<ImportedSym>>,
    /// Imported-file cache keyed by filesystem path:
    /// (mtime, symbols, that file's own import paths).
    import_cache: HashMap<String, (std::time::SystemTime, Vec<analysis::SymbolInfo>, Vec<String>)>,
    got_shutdown: bool,
}

impl Server {
    /// Handle one message. Returns true when the server must exit.
    fn handle(&mut self, message: &Value, output: &mut impl Write) -> bool {
        let method = message["method"].as_str().unwrap_or("");
        let id = message.get("id").cloned();
        let params = &message["params"];

        match method {
            // ── lifecycle ────────────────────────────────────────────────
            "initialize" => {
                let result = json!({
                    "capabilities": {
                        "textDocumentSync": 1, // full
                        "completionProvider": { "triggerCharacters": ["."] },
                        "hoverProvider": true,
                        "definitionProvider": true,
                        "documentSymbolProvider": true,
                        "referencesProvider": true,
                        "renameProvider": true,
                        "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
                    },
                    "serverInfo": {
                        "name": "sz-lsp",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                });
                self.respond(output, id, result);
            }
            "initialized" => {}
            "shutdown" => {
                self.got_shutdown = true;
                self.respond(output, id, Value::Null);
            }
            "exit" => return true,

            // ── document sync ────────────────────────────────────────────
            "textDocument/didOpen" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_string();
                let text = params["textDocument"]["text"].as_str().unwrap_or("");
                self.update_document(output, uri, text);
            }
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_string();
                // full sync: the last change carries the whole document
                if let Some(text) = params["contentChanges"]
                    .as_array()
                    .and_then(|c| c.last())
                    .and_then(|c| c["text"].as_str())
                {
                    self.update_document(output, uri, text);
                }
            }
            "textDocument/didClose" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
                self.docs.remove(uri);
                self.notify(output, "textDocument/publishDiagnostics",
                    json!({ "uri": uri, "diagnostics": [] }));
            }
            "textDocument/didSave" => {}

            // ── language features ────────────────────────────────────────
            "textDocument/completion" => {
                let result = self.completion(params);
                self.respond(output, id, result);
            }
            "completionItem/resolve" => {
                // no resolveProvider advertised; echo for lenient clients
                self.respond(output, id, params.clone());
            }
            "textDocument/hover" => {
                let result = self.hover(params);
                self.respond(output, id, result);
            }
            "textDocument/definition" => {
                let result = self.definition(params);
                self.respond(output, id, result);
            }
            "textDocument/documentSymbol" => {
                let result = self.document_symbols(params);
                self.respond(output, id, result);
            }
            "textDocument/references" => {
                let result = self.references(params);
                self.respond(output, id, result);
            }
            "textDocument/rename" => {
                let result = self.rename(params);
                self.respond(output, id, result);
            }
            "textDocument/signatureHelp" => {
                let result = self.signature_help(params);
                self.respond(output, id, result);
            }

            // ── everything else ──────────────────────────────────────────
            _ => {
                if let Some(id) = id {
                    // unknown *request* → MethodNotFound
                    let error = json!({ "code": -32601, "message": format!("method '{}' not implemented", method) });
                    rpc::write_message(output, &json!({ "jsonrpc": "2.0", "id": id, "error": error }));
                }
                // unknown notifications ($/cancelRequest, …) are ignored
            }
        }
        false
    }

    fn respond(&self, output: &mut impl Write, id: Option<Value>, result: Value) {
        if let Some(id) = id {
            rpc::write_message(output, &json!({ "jsonrpc": "2.0", "id": id, "result": result }));
        }
    }

    fn notify(&self, output: &mut impl Write, method: &str, params: Value) {
        rpc::write_message(output, &json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn update_document(&mut self, output: &mut impl Write, uri: String, text: &str) {
        // .szx (JSX): the parser doesn't speak JSX — symbols/completion only,
        // no diagnostics (they would be pure noise).
        let analysis = if uri.ends_with(".szx") {
            analysis::analyze_szx(text)
        } else {
            analysis::analyze(text)
        };
        let diagnostics: Vec<Value> = analysis
            .diagnostics
            .iter()
            .map(|d| {
                json!({
                    "range": diag_range(&analysis.lines, d.line, d.column),
                    "severity": d.severity,
                    "source": "sz",
                    "message": d.message,
                })
            })
            .collect();
        self.notify(output, "textDocument/publishDiagnostics",
            json!({ "uri": uri, "diagnostics": diagnostics }));
        let imported = self.collect_import_symbols(&uri, &analysis.lines);
        self.doc_imports.insert(uri.clone(), imported);
        self.docs.insert(uri, analysis);
    }

    /// Symbols from the document's imports, followed transitively (bounded).
    /// Files are read from disk and cached by mtime, so keystrokes don't
    /// re-read an unchanged module graph.
    fn collect_import_symbols(&mut self, uri: &str, lines: &[String]) -> Vec<ImportedSym> {
        let mut out: Vec<ImportedSym> = Vec::new();
        let dir = match uri.rfind('/').and_then(|p| uri_to_path(&uri[..p + 1])) {
            Some(d) => d,
            None => return out,
        };
        let mut visited: std::collections::HashSet<String> = Default::default();
        let mut queue: Vec<(String, String)> = analysis::import_paths(lines)
            .into_iter()
            .map(|p| (dir.clone(), p))
            .collect();
        let mut budget = 32; // transitive file cap: keeps pathological graphs cheap

        while let Some((base, import)) = queue.pop() {
            if budget == 0 {
                break;
            }
            let mut rel = import;
            if !rel.ends_with(".sz") && !rel.ends_with(".szx") {
                rel.push_str(".sz");
            }
            let full = std::path::Path::new(&base)
                .join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
            let canon = match full.canonicalize() {
                Ok(c) => c,
                Err(_) => continue, // package/module imports without a local file
            };
            let key = canon.to_string_lossy().to_string();
            if !visited.insert(key.clone()) {
                continue;
            }
            budget -= 1;

            let mtime = std::fs::metadata(&canon).and_then(|m| m.modified()).ok();
            let cached = mtime.and_then(|mt| {
                self.import_cache
                    .get(&key)
                    .filter(|(t, _, _)| *t == mt)
                    .map(|(_, syms, nested)| (syms.clone(), nested.clone()))
            });
            let (symbols, nested) = match cached {
                Some(hit) => hit,
                None => {
                    let text = match std::fs::read_to_string(&canon) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    let a = analysis::analyze_szx(&text); // symbol scan only
                    let nested = analysis::import_paths(&a.lines);
                    if let Some(mt) = mtime {
                        self.import_cache
                            .insert(key.clone(), (mt, a.symbols.clone(), nested.clone()));
                    }
                    (a.symbols, nested)
                }
            };

            let file_uri = path_to_uri(&canon);
            let file_dir = canon
                .parent()
                .map(|p| p.to_string_lossy().trim_start_matches(r"\\?\").to_string())
                .unwrap_or_else(|| base.clone());
            for sym in symbols {
                if sym.kind != SymbolKind::Import {
                    out.push(ImportedSym { sym, uri: file_uri.clone() });
                }
            }
            for n in nested {
                queue.push((file_dir.clone(), n));
            }
        }
        out
    }

    // ── completion ───────────────────────────────────────────────────────────

    fn completion(&self, params: &Value) -> Value {
        let (doc, line, character) = match self.at(params) {
            Some(x) => x,
            None => return Value::Null,
        };
        let cx = analysis::completion_context(&doc.lines, line, character);
        let mut items: Vec<Value> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let mut push = |items: &mut Vec<Value>, label: &str, kind: u8, detail: &str| {
            if seen.iter().any(|s| s == label) {
                return;
            }
            seen.push(label.to_string());
            items.push(json!({ "label": label, "kind": kind, "detail": detail }));
        };

        if let Some(receiver) = &cx.receiver {
            if let Some(methods) = builtins::namespace_methods(receiver) {
                for m in methods {
                    push(&mut items, m, 2, &format!("{}.{} — nativo", receiver, m));
                }
            } else if receiver == "this" {
                let class = enclosing_class(doc, line);
                for s in &doc.symbols {
                    if s.container.is_some() && (class.is_none() || s.container == class) {
                        push(&mut items, &s.name, s.kind.completion(), &s.detail);
                    }
                }
            } else {
                // unknown receiver: offer every value method, tagged by origin
                for (origin, methods) in builtins::VALUE_METHODS {
                    for m in *methods {
                        push(&mut items, m, 2, &format!("método de {}", origin));
                    }
                }
            }
        } else {
            for k in builtins::KEYWORDS {
                push(&mut items, k, 14, "keyword");
            }
            for k in builtins::TYPE_KEYWORDS {
                push(&mut items, k, 14, "tipo");
            }
            for (ns, methods) in builtins::NAMESPACES {
                push(&mut items, ns, 9, &format!("namespace nativo ({} métodos)", methods.len()));
            }
            for (name, sig) in builtins::BUILTIN_FUNCTIONS {
                push(&mut items, name, 3, sig);
            }
            for s in &doc.symbols {
                if s.kind != SymbolKind::Import {
                    push(&mut items, &s.name, s.kind.completion(), &s.detail);
                }
            }
            // Symbols from imported files (marked with their origin).
            if let Some(imports) = params["textDocument"]["uri"]
                .as_str()
                .and_then(|u| self.doc_imports.get(u))
            {
                for i in imports {
                    let file = i.uri.rsplit('/').next().unwrap_or(&i.uri);
                    let detail = format!("{} — import {}", i.sym.detail, percent_decode(file));
                    push(&mut items, &i.sym.name, i.sym.kind.completion(), &detail);
                }
            }
        }
        // client-side filtering handles the prefix; sending everything is fine
        let _ = cx.prefix;
        json!(items)
    }

    // ── hover ────────────────────────────────────────────────────────────────

    fn hover(&self, params: &Value) -> Value {
        let (doc, line, character) = match self.at(params) {
            Some(x) => x,
            None => return Value::Null,
        };
        let (word, receiver) = match analysis::word_at(&doc.lines, line, character) {
            Some(w) => w,
            None => return Value::Null,
        };

        let text = if let Some(recv) = receiver.as_deref().filter(|r| builtins::is_namespace(r)) {
            if builtins::namespace_methods(recv).map(|m| m.contains(&word.as_str())).unwrap_or(false) {
                format!("```serez-code\n{}.{}(…)\n```\nMétodo nativo del namespace `{}`.", recv, word, recv)
            } else {
                return Value::Null;
            }
        } else if builtins::is_namespace(&word) && receiver.is_none() {
            let doc_line = builtins::namespace_doc(&word).unwrap_or("");
            let count = builtins::namespace_methods(&word).map(|m| m.len()).unwrap_or(0);
            format!("```serez-code\n{}\n```\n{} ({} métodos)", word, doc_line, count)
        } else if let Some(symbol) = analysis::find_definition(&doc.symbols, &word, line) {
            let mut s = format!("```serez-code\n{}\n```\n", symbol.detail);
            if let Some(container) = &symbol.container {
                s.push_str(&format!("Miembro de `{}` — ", container));
            }
            s.push_str(&format!("declarado en la línea {}.", symbol.line));
            s
        } else if let Some(imp) = params["textDocument"]["uri"]
            .as_str()
            .and_then(|u| self.doc_imports.get(u))
            .and_then(|v| v.iter().find(|i| i.sym.name == word))
        {
            let file = imp.uri.rsplit('/').next().unwrap_or(&imp.uri);
            format!(
                "```serez-code\n{}\n```\nImportado de `{}` (línea {}).",
                imp.sym.detail, percent_decode(file), imp.sym.line
            )
        } else if let Some(sig) = builtins::builtin_function(&word) {
            format!("```serez-code\n{}\n```\nFunción builtin.", sig)
        } else {
            return Value::Null;
        };

        json!({ "contents": { "kind": "markdown", "value": text } })
    }

    // ── definition ───────────────────────────────────────────────────────────

    fn definition(&self, params: &Value) -> Value {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_string();
        let (doc, line, character) = match self.at(params) {
            Some(x) => x,
            None => return Value::Null,
        };

        // jump-to-file on `import "path"` lines
        if let Some(target) = import_target(&doc.lines, line, &uri) {
            return json!({
                "uri": target,
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
            });
        }

        let (word, receiver) = match analysis::word_at(&doc.lines, line, character) {
            Some(w) => w,
            None => return Value::Null,
        };
        if receiver.as_deref().map(|r| builtins::is_namespace(r)).unwrap_or(false) {
            return Value::Null; // native methods have no source location
        }
        match analysis::find_definition(&doc.symbols, &word, line) {
            Some(symbol) => {
                let l = symbol.line.saturating_sub(1);
                let c = symbol.column.saturating_sub(1);
                json!({
                    "uri": uri,
                    "range": {
                        "start": { "line": l, "character": c },
                        "end": { "line": l, "character": c + symbol.name.chars().count() },
                    },
                })
            }
            // Not local — maybe it comes from an imported file.
            None => match self.doc_imports.get(&uri).and_then(|v| v.iter().find(|i| i.sym.name == word)) {
                Some(imp) => {
                    let l = imp.sym.line.saturating_sub(1);
                    let c = imp.sym.column.saturating_sub(1);
                    json!({
                        "uri": imp.uri,
                        "range": {
                            "start": { "line": l, "character": c },
                            "end": { "line": l, "character": c + imp.sym.name.chars().count() },
                        },
                    })
                }
                None => Value::Null,
            },
        }
    }

    // ── document symbols ─────────────────────────────────────────────────────

    fn document_symbols(&self, params: &Value) -> Value {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let doc = match self.docs.get(uri) {
            Some(d) => d,
            None => return Value::Null,
        };
        let to_symbol = |s: &analysis::SymbolInfo, children: Vec<Value>| -> Value {
            let l = s.line.saturating_sub(1);
            let c = s.column.saturating_sub(1);
            let range = json!({
                "start": { "line": l, "character": c },
                "end": { "line": l, "character": c + s.name.chars().count() },
            });
            let mut v = json!({
                "name": s.name,
                "detail": s.detail,
                "kind": s.kind.lsp(),
                "range": range,
                "selectionRange": range,
            });
            if !children.is_empty() {
                v["children"] = json!(children);
            }
            v
        };

        let mut out: Vec<Value> = Vec::new();
        for s in &doc.symbols {
            if s.container.is_some() || s.kind == SymbolKind::Import {
                continue;
            }
            if s.kind == SymbolKind::Class {
                let children: Vec<Value> = doc
                    .symbols
                    .iter()
                    .filter(|m| m.container.as_deref() == Some(s.name.as_str()))
                    .map(|m| to_symbol(m, Vec::new()))
                    .collect();
                out.push(to_symbol(s, children));
            } else {
                out.push(to_symbol(s, Vec::new()));
            }
        }
        json!(out)
    }

    // ── references / rename ─────────────────────────────────────────────────

    /// Every occurrence of the identifier under the cursor, as LSP Locations.
    fn occurrence_locations(&self, params: &Value) -> Option<(String, String, Vec<(usize, usize)>)> {
        let uri = params["textDocument"]["uri"].as_str()?.to_string();
        let (doc, line, character) = self.at(params)?;
        let (word, _receiver) = analysis::word_at(&doc.lines, line, character)?;
        let text = doc.lines.join("\n");
        let occ = analysis::occurrences(&text, &word);
        Some((uri, word, occ))
    }

    fn references(&self, params: &Value) -> Value {
        let (uri, word, occ) = match self.occurrence_locations(params) {
            Some(x) => x,
            None => return Value::Null,
        };
        let len = word.chars().count();
        let locations: Vec<Value> = occ
            .iter()
            .map(|(l, c)| {
                let (l, c) = (l.saturating_sub(1), c.saturating_sub(1));
                json!({
                    "uri": uri,
                    "range": {
                        "start": { "line": l, "character": c },
                        "end": { "line": l, "character": c + len },
                    },
                })
            })
            .collect();
        json!(locations)
    }

    fn rename(&self, params: &Value) -> Value {
        let new_name = match params["newName"].as_str() {
            Some(n) => n.to_string(),
            None => return Value::Null,
        };
        let valid = !new_name.is_empty()
            && !new_name.chars().next().unwrap().is_ascii_digit()
            && new_name.chars().all(analysis::is_ident_char);
        if !valid {
            return Value::Null;
        }
        let (uri, word, occ) = match self.occurrence_locations(params) {
            Some(x) => x,
            None => return Value::Null,
        };
        let len = word.chars().count();
        let edits: Vec<Value> = occ
            .iter()
            .map(|(l, c)| {
                let (l, c) = (l.saturating_sub(1), c.saturating_sub(1));
                json!({
                    "range": {
                        "start": { "line": l, "character": c },
                        "end": { "line": l, "character": c + len },
                    },
                    "newText": new_name,
                })
            })
            .collect();
        json!({ "changes": { uri: edits } })
    }

    // ── signature help ───────────────────────────────────────────────────────

    fn signature_help(&self, params: &Value) -> Value {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("").to_string();
        let (doc, line, character) = match self.at(params) {
            Some(x) => x,
            None => return Value::Null,
        };
        let text = match doc.lines.get(line) {
            Some(t) => t,
            None => return Value::Null,
        };
        let chars: Vec<char> = text.chars().collect();
        let upto = character.min(chars.len());

        // Walk left from the cursor to the unclosed '(' of the current call,
        // counting top-level commas on the way (→ activeParameter).
        let mut depth = 0i32;
        let mut commas = 0usize;
        let mut open: Option<usize> = None;
        for i in (0..upto).rev() {
            match chars[i] {
                ')' => depth += 1,
                '(' if depth == 0 => { open = Some(i); break; }
                '(' => depth -= 1,
                ',' if depth == 0 => commas += 1,
                _ => {}
            }
        }
        let open = match open {
            Some(o) => o,
            None => return Value::Null,
        };

        // Callee name (and optional `Receiver.`) right before the paren.
        let mut end = open;
        while end > 0 && chars[end - 1] == ' ' {
            end -= 1;
        }
        let mut start = end;
        while start > 0 && analysis::is_ident_char(chars[start - 1]) {
            start -= 1;
        }
        if start == end {
            return Value::Null;
        }
        let name: String = chars[start..end].iter().collect();
        let receiver = if start > 0 && chars[start - 1] == '.' {
            let mut rs = start - 1;
            while rs > 0 && analysis::is_ident_char(chars[rs - 1]) {
                rs -= 1;
            }
            Some(chars[rs..start - 1].iter().collect::<String>())
        } else {
            None
        };

        // Signature label: user symbol (local or imported), builtin, or
        // namespace method.
        let label = if let Some(sym) = doc
            .symbols
            .iter()
            .find(|s| s.name == name && matches!(s.kind, SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor))
        {
            sym.detail.clone()
        } else if let Some(imp) = self.doc_imports.get(&uri).and_then(|v| {
            v.iter().find(|i| i.sym.name == name
                && matches!(i.sym.kind, SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor))
        }) {
            imp.sym.detail.clone()
        } else if let Some(sig) = builtins::builtin_function(&name) {
            sig.to_string()
        } else if let Some(recv) = receiver.filter(|r| builtins::is_namespace(r)) {
            if builtins::namespace_methods(&recv).map(|m| m.contains(&name.as_str())).unwrap_or(false) {
                format!("{}.{}(…)", recv, name)
            } else {
                return Value::Null;
            }
        } else {
            return Value::Null;
        };

        // Parameter labels from the signature's own parens.
        let parameters: Vec<Value> = label
            .find('(')
            .and_then(|o| label[o + 1..].find(')').map(|c| label[o + 1..o + 1 + c].to_string()))
            .map(|inner| {
                inner
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty() && p != "…")
                    .map(|p| json!({ "label": p }))
                    .collect()
            })
            .unwrap_or_default();

        json!({
            "signatures": [{ "label": label, "parameters": parameters }],
            "activeSignature": 0,
            "activeParameter": commas,
        })
    }

    /// (analysis, 0-based line, 0-based character) for a positional request.
    fn at(&self, params: &Value) -> Option<(&Analysis, usize, usize)> {
        let uri = params["textDocument"]["uri"].as_str()?;
        let doc = self.docs.get(uri)?;
        let line = params["position"]["line"].as_u64()? as usize;
        let character = params["position"]["character"].as_u64()? as usize;
        Some((doc, line, character))
    }
}

/// Range for a diagnostic at 1-based (line, column): covers the identifier at
/// that spot, or one character. line == 0 (unknown) maps to the file start.
fn diag_range(lines: &[String], line: usize, column: usize) -> Value {
    if line == 0 {
        return json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 1 },
        });
    }
    let l = line - 1;
    let mut c = column.saturating_sub(1);
    let mut end = c + 1;
    if let Some(text) = lines.get(l) {
        let chars: Vec<char> = text.chars().collect();
        if c < chars.len() && analysis::is_ident_char(chars[c]) {
            // extend over the whole identifier in both directions (the lexer
            // stamps multi-char tokens at their last char, not the first)
            while c > 0 && analysis::is_ident_char(chars[c - 1]) {
                c -= 1;
            }
            let mut e = end - 1;
            while e + 1 < chars.len() && analysis::is_ident_char(chars[e + 1]) {
                e += 1;
            }
            end = e + 1;
        } else if c >= chars.len() {
            // token at EOL (e.g. missing `;`): underline the last character
            let len = chars.len().max(1);
            return json!({
                "start": { "line": l, "character": len - 1 },
                "end": { "line": l, "character": len },
            });
        }
    }
    json!({
        "start": { "line": l, "character": c },
        "end": { "line": l, "character": end },
    })
}

/// The class whose body encloses `line` (0-based), judged by declaration
/// order: the last class declared at or before the line.
fn enclosing_class(doc: &Analysis, line: usize) -> Option<String> {
    doc.symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class && s.line <= line + 1)
        .last()
        .map(|s| s.name.clone())
}

/// If `line` (0-based) is an `import "path"` line, the target file's URI
/// (resolved against the document's directory), only if the file exists.
fn import_target(lines: &[String], line: usize, doc_uri: &str) -> Option<String> {
    let text = lines.get(line)?;
    let trimmed = text.trim_start();
    if !trimmed.starts_with("import") {
        return None;
    }
    let first_quote = text.find('"')?;
    let rest = &text[first_quote + 1..];
    let second_quote = rest.find('"')?;
    let mut import_path = rest[..second_quote].to_string();
    if !import_path.ends_with(".sz") {
        import_path.push_str(".sz");
    }

    let dir_uri = &doc_uri[..doc_uri.rfind('/')? + 1];
    // resolve against the document directory on disk before answering
    let dir_path = uri_to_path(dir_uri)?;
    let target = std::path::Path::new(&dir_path).join(import_path.replace('/', std::path::MAIN_SEPARATOR_STR));
    if !target.exists() {
        return None;
    }
    let encoded: String = import_path
        .split('/')
        .map(|seg| percent_encode(seg))
        .collect::<Vec<_>>()
        .join("/");
    Some(format!("{}{}", dir_uri, encoded))
}

/// Filesystem path → file:// URI (inverse of `uri_to_path`, same encoding
/// style as the URIs VS Code sends: drive colon percent-encoded).
fn path_to_uri(path: &std::path::Path) -> String {
    let s = path.display().to_string();
    let s = s.trim_start_matches(r"\\?\").replace('\\', "/");
    let encoded: String = s
        .split('/')
        .map(percent_encode)
        .collect::<Vec<_>>()
        .join("/");
    format!("file:///{}", encoded.trim_start_matches('/'))
}

/// file:// URI → filesystem path (enough for the URIs VS Code sends).
fn uri_to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let decoded = percent_decode(rest);
    // windows: /E:/dir → E:/dir
    let decoded = if decoded.len() > 2 && decoded.as_bytes()[0] == b'/' && decoded.as_bytes()[2] == b':' {
        decoded[1..].to_string()
    } else {
        decoded
    };
    Some(decoded)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode(seg: &str) -> String {
    let mut out = String::new();
    for b in seg.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str = "file:///E%3A/proyecto/app.sz";

    /// Feed one message to the server, return every message it wrote back.
    fn send(server: &mut Server, message: Value) -> Vec<Value> {
        let mut out: Vec<u8> = Vec::new();
        server.handle(&message, &mut out);
        // parse the Content-Length frames back
        let mut messages = Vec::new();
        let mut rest = &out[..];
        while let Some(pos) = rest.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&rest[..pos]);
            let len: usize = headers
                .lines()
                .find_map(|l| l.strip_prefix("Content-Length:"))
                .and_then(|v| v.trim().parse().ok())
                .expect("Content-Length header");
            let body = &rest[pos + 4..pos + 4 + len];
            messages.push(serde_json::from_slice(body).expect("valid JSON body"));
            rest = &rest[pos + 4 + len..];
        }
        messages
    }

    fn request(id: u64, method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    fn notification(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "method": method, "params": params })
    }

    fn open(server: &mut Server, text: &str) -> Vec<Value> {
        send(server, notification("textDocument/didOpen", json!({
            "textDocument": { "uri": URI, "languageId": "serez-code", "version": 1, "text": text }
        })))
    }

    fn at(line: u64, character: u64) -> Value {
        json!({
            "textDocument": { "uri": URI },
            "position": { "line": line, "character": character },
        })
    }

    #[test]
    fn initialize_reports_capabilities() {
        let mut server = Server::default();
        let replies = send(&mut server, request(1, "initialize", json!({})));
        assert_eq!(replies.len(), 1);
        let caps = &replies[0]["result"]["capabilities"];
        assert_eq!(caps["textDocumentSync"], 1);
        assert_eq!(caps["hoverProvider"], true);
        assert_eq!(caps["definitionProvider"], true);
        assert_eq!(replies[0]["result"]["serverInfo"]["name"], "sz-lsp");
    }

    #[test]
    fn did_open_publishes_diagnostics() {
        let mut server = Server::default();
        let replies = open(&mut server, "if (true {\n    out 1;\n}\n");
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0]["method"], "textDocument/publishDiagnostics");
        let diags = replies[0]["params"]["diagnostics"].as_array().unwrap();
        assert!(!diags.is_empty());
        assert_eq!(diags[0]["severity"], 1);
        assert_eq!(diags[0]["range"]["start"]["line"], 0);
    }

    #[test]
    fn did_change_clears_diagnostics_when_fixed() {
        let mut server = Server::default();
        open(&mut server, "if (true {\n    out 1;\n}\n");
        let replies = send(&mut server, notification("textDocument/didChange", json!({
            "textDocument": { "uri": URI, "version": 2 },
            "contentChanges": [ { "text": "if (true) {\n    out 1;\n}\n" } ],
        })));
        let diags = replies[0]["params"]["diagnostics"].as_array().unwrap();
        assert!(diags.is_empty(), "{:?}", diags);
    }

    #[test]
    fn completion_after_namespace_dot_lists_its_methods() {
        let mut server = Server::default();
        open(&mut server, "use permissions { File };\nFile.\n");
        let replies = send(&mut server, request(2, "textDocument/completion", at(1, 5)));
        let items = replies[0]["result"].as_array().unwrap();
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"read"), "{:?}", labels);
        assert!(labels.contains(&"write"), "{:?}", labels);
        assert!(!labels.contains(&"let"), "namespace completion must not list keywords");
    }

    #[test]
    fn completion_at_top_level_lists_keywords_namespaces_and_symbols() {
        let mut server = Server::default();
        open(&mut server, "fn int suma(int a, int b) {\n    return a + b;\n}\n\n");
        let replies = send(&mut server, request(3, "textDocument/completion", at(3, 0)));
        let items = replies[0]["result"].as_array().unwrap();
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"let"));
        assert!(labels.contains(&"File"));
        assert!(labels.contains(&"suma"));
        assert!(labels.contains(&"parseInt"));
    }

    #[test]
    fn hover_on_user_function_shows_signature() {
        let mut server = Server::default();
        open(&mut server, "fn int suma(int a, int b) {\n    return a + b;\n}\nout suma(1, 2);\n");
        let replies = send(&mut server, request(4, "textDocument/hover", at(3, 5)));
        let value = replies[0]["result"]["contents"]["value"].as_str().unwrap();
        assert!(value.contains("fn int suma(int a, int b)"), "{}", value);
    }

    #[test]
    fn hover_on_namespace_method() {
        let mut server = Server::default();
        open(&mut server, "let d = JSON.parse(\"{}\");\n");
        let replies = send(&mut server, request(5, "textDocument/hover", at(0, 14)));
        let value = replies[0]["result"]["contents"]["value"].as_str().unwrap();
        assert!(value.contains("JSON.parse"), "{}", value);
    }

    #[test]
    fn definition_of_function_points_at_declaration() {
        let mut server = Server::default();
        open(&mut server, "fn int suma(int a, int b) {\n    return a + b;\n}\nout suma(1, 2);\n");
        let replies = send(&mut server, request(6, "textDocument/definition", at(3, 5)));
        let result = &replies[0]["result"];
        assert_eq!(result["uri"], URI);
        assert_eq!(result["range"]["start"]["line"], 0);
        assert_eq!(result["range"]["start"]["character"], 7);
    }

    #[test]
    fn document_symbols_nest_class_members() {
        let mut server = Server::default();
        open(&mut server, "class Animal {\n    public Animal(string n) {\n        this.nombre = n;\n    }\n    public string getNombre() {\n        return this.nombre;\n    }\n}\nlet a = new Animal(\"Rex\");\n");
        let replies = send(&mut server, request(7, "textDocument/documentSymbol", json!({
            "textDocument": { "uri": URI }
        })));
        let symbols = replies[0]["result"].as_array().unwrap();
        let class = symbols.iter().find(|s| s["name"] == "Animal").expect("class symbol");
        assert_eq!(class["kind"], 5);
        let children = class["children"].as_array().expect("children");
        assert!(children.iter().any(|c| c["name"] == "getNombre"));
    }

    #[test]
    fn unknown_request_gets_method_not_found() {
        let mut server = Server::default();
        let replies = send(&mut server, request(9, "textDocument/foldingRange", json!({})));
        assert_eq!(replies[0]["error"]["code"], -32601);
    }

    #[test]
    fn references_lists_every_occurrence() {
        let mut server = Server::default();
        open(&mut server, "fn int suma(int a, int b) {\n    return a + b;\n}\nout suma(1, 2);\nout suma(3, 4);\n");
        let replies = send(&mut server, request(11, "textDocument/references", at(3, 5)));
        let locs = replies[0]["result"].as_array().unwrap();
        assert_eq!(locs.len(), 3, "{:?}", locs); // decl + 2 calls
        assert_eq!(locs[0]["range"]["start"]["line"], 0);
        assert_eq!(locs[0]["range"]["start"]["character"], 7);
    }

    #[test]
    fn rename_edits_every_occurrence_and_validates_name() {
        let mut server = Server::default();
        open(&mut server, "let total = 1;\nout total;\ntotal = total + 1;\n");
        let mut p = at(1, 5);
        p["newName"] = json!("acumulado");
        let replies = send(&mut server, request(12, "textDocument/rename", p));
        let edits = replies[0]["result"]["changes"][URI].as_array().unwrap();
        assert_eq!(edits.len(), 4, "{:?}", edits);
        assert!(edits.iter().all(|e| e["newText"] == "acumulado"));

        let mut bad = at(1, 5);
        bad["newName"] = json!("9invalid");
        let replies = send(&mut server, request(13, "textDocument/rename", bad));
        assert!(replies[0]["result"].is_null());
    }

    #[test]
    fn signature_help_shows_signature_and_active_parameter() {
        let mut server = Server::default();
        open(&mut server, "fn int suma(int a, int b) {\n    return a + b;\n}\nout suma(1, \n");
        let replies = send(&mut server, request(14, "textDocument/signatureHelp", at(3, 12)));
        let result = &replies[0]["result"];
        let label = result["signatures"][0]["label"].as_str().unwrap();
        assert!(label.contains("fn int suma(int a, int b)"), "{}", label);
        assert_eq!(result["activeParameter"], 1); // after the first comma
        let params = result["signatures"][0]["parameters"].as_array().unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0]["label"], "int a");
    }

    #[test]
    fn szx_document_gets_symbols_but_no_diagnostics() {
        let mut server = Server::default();
        let uri = "file:///E%3A/proyecto/app.szx";
        let replies = send(&mut server, notification("textDocument/didOpen", json!({
            "textDocument": { "uri": uri, "languageId": "serez-code-jsx", "version": 1,
                "text": "class App {\n    fn any render() {\n        return <View><Text>hola</Text></View>;\n    }\n}\n" }
        })));
        // JSX would drown the parser — diagnostics must stay empty…
        let diags = replies[0]["params"]["diagnostics"].as_array().unwrap();
        assert!(diags.is_empty(), "{:?}", diags);
        // …while the token-level outline still works.
        let replies = send(&mut server, request(15, "textDocument/documentSymbol", json!({
            "textDocument": { "uri": uri }
        })));
        let symbols = replies[0]["result"].as_array().unwrap();
        assert!(symbols.iter().any(|s| s["name"] == "App"), "{:?}", symbols);
    }

    #[test]
    fn definition_and_completion_reach_imported_files() {
        // Real files on disk: an imported module chain mod_a → mod_b.
        let dir = std::env::temp_dir().join("sz_lsp_multifile_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mod_a.sz"), "import \"mod_b.sz\";\nfn int duplicar(int n) {\n    return n * 2;\n}\n").unwrap();
        std::fs::write(dir.join("mod_b.sz"), "fn int triplicar(int n) {\n    return n * 3;\n}\n").unwrap();

        let main_uri = path_to_uri(&dir.join("main.sz"));
        let mut server = Server::default();
        send(&mut server, notification("textDocument/didOpen", json!({
            "textDocument": { "uri": main_uri, "languageId": "serez-code", "version": 1,
                "text": "import \"mod_a.sz\";\nout duplicar(21);\nout triplicar(7);\n" }
        })));

        // definition of `duplicar` (direct import) lands in mod_a.sz
        let p = json!({ "textDocument": { "uri": main_uri }, "position": { "line": 1, "character": 6 } });
        let replies = send(&mut server, request(16, "textDocument/definition", p));
        let uri = replies[0]["result"]["uri"].as_str().expect("definition uri");
        assert!(uri.ends_with("mod_a.sz"), "{}", uri);
        assert_eq!(replies[0]["result"]["range"]["start"]["line"], 1);

        // definition of `triplicar` (transitive import) lands in mod_b.sz
        let p = json!({ "textDocument": { "uri": main_uri }, "position": { "line": 2, "character": 6 } });
        let replies = send(&mut server, request(17, "textDocument/definition", p));
        let uri = replies[0]["result"]["uri"].as_str().expect("transitive definition uri");
        assert!(uri.ends_with("mod_b.sz"), "{}", uri);

        // completion offers the imported symbols
        let p = json!({ "textDocument": { "uri": main_uri }, "position": { "line": 2, "character": 0 } });
        let replies = send(&mut server, request(18, "textDocument/completion", p));
        let labels: Vec<&str> = replies[0]["result"].as_array().unwrap()
            .iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"duplicar"), "{:?}", labels);
        assert!(labels.contains(&"triplicar"), "{:?}", labels);
    }

    #[test]
    fn shutdown_then_exit() {
        let mut server = Server::default();
        let replies = send(&mut server, request(10, "shutdown", Value::Null));
        assert!(replies[0]["result"].is_null());
        let mut out: Vec<u8> = Vec::new();
        assert!(server.handle(&notification("exit", Value::Null), &mut out));
        assert!(server.got_shutdown);
    }
}
