// Per-document analysis: diagnostics (parser + type checker) and a
// token-level symbol index (functions, classes, enums, variables, imports).
//
// The symbol index deliberately works on tokens, not the AST: tokens carry
// line/column (the AST mostly doesn't) and keep working while the file has
// parse errors — which is the normal state while the user is typing.
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::token::{Token, TokenType};
use crate::type_checker::TypeChecker;

#[derive(Debug, Clone)]
pub struct Diag {
    /// 1-based; 0 means "unknown" (mapped to the start of the file).
    pub line: usize,
    pub column: usize,
    pub message: String,
    /// LSP severity: 1 = Error, 2 = Warning.
    pub severity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymbolKind {
    Function,
    Method,
    Constructor,
    Class,
    Interface,
    Enum,
    Variable,
    Constant,
    Import,
}

impl SymbolKind {
    /// LSP SymbolKind number.
    pub fn lsp(self) -> u8 {
        match self {
            SymbolKind::Function => 12,
            SymbolKind::Method => 6,
            SymbolKind::Constructor => 9,
            SymbolKind::Class => 5,
            SymbolKind::Interface => 11,
            SymbolKind::Enum => 10,
            SymbolKind::Variable => 13,
            SymbolKind::Constant => 14,
            SymbolKind::Import => 2, // Module
        }
    }

    /// LSP CompletionItemKind number.
    pub fn completion(self) -> u8 {
        match self {
            SymbolKind::Function => 3,
            SymbolKind::Method | SymbolKind::Constructor => 2,
            SymbolKind::Class => 7,
            SymbolKind::Interface => 8,
            SymbolKind::Enum => 13,
            SymbolKind::Variable => 6,
            SymbolKind::Constant => 21,
            SymbolKind::Import => 9,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    /// 1-based position of the name token.
    pub line: usize,
    pub column: usize,
    /// Signature-like one-liner (source slice), for hover/documentSymbol.
    pub detail: String,
    /// Enclosing class name, for methods/fields.
    pub container: Option<String>,
}

pub struct Analysis {
    pub lines: Vec<String>,
    pub diagnostics: Vec<Diag>,
    pub symbols: Vec<SymbolInfo>,
}

/// Analysis for `.szx` (JSX) documents: the serez parser does not understand
/// JSX blocks, so diagnostics would be pure noise — the token-level symbol
/// index still works (it tolerates arbitrary broken regions), so completion,
/// hover, outline, definition, references and rename stay available.
pub fn analyze_szx(text: &str) -> Analysis {
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let symbols = scan_symbols(text, &lines);
    Analysis { lines, diagnostics: Vec::new(), symbols }
}

/// Every identifier token equal to `name`: 1-based (line, first-char column).
/// Powers references/rename; token-based, so it works mid-keystroke.
pub fn occurrences(text: &str, name: &str) -> Vec<(usize, usize)> {
    collect_tokens(text)
        .iter()
        .filter(|t| t.token_type == TokenType::Ident && t.literal == name)
        .map(|t| (t.line, ident_start_col(t)))
        .collect()
}

/// The string paths of `import "…"` statements, in order.
pub fn import_paths(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("import") {
            continue;
        }
        if let Some(first) = trimmed.find('"') {
            let rest = &trimmed[first + 1..];
            if let Some(second) = rest.find('"') {
                out.push(rest[..second].to_string());
            }
        }
    }
    out
}

pub fn analyze(text: &str) -> Analysis {
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let mut diagnostics = Vec::new();

    // Parse: collect parser errors as Error diagnostics.
    let lexer = Lexer::new(text.to_string());
    let mut parser = Parser::new(lexer);
    parser.set_source(lines.clone());
    let program = parser.parse_program();
    for e in parser.take_errors() {
        diagnostics.push(Diag { line: e.line, column: e.column, message: e.message, severity: 1 });
    }

    // Type check: non-fatal in the CLI, so surfaced as warnings.
    let mut checker = TypeChecker::new(&program);
    checker.check();
    for e in checker.take_errors() {
        diagnostics.push(Diag { line: e.line, column: e.column, message: e.message, severity: 2 });
    }

    let symbols = scan_symbols(text, &lines);
    Analysis { lines, diagnostics, symbols }
}

// ── Symbol scanning ───────────────────────────────────────────────────────────

fn collect_tokens(text: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(text.to_string());
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token();
        let eof = tok.token_type == TokenType::Eof;
        tokens.push(tok);
        // Hard cap so a pathological file can't wedge the server.
        if eof || tokens.len() > 200_000 {
            break;
        }
    }
    tokens
}

/// 1-based column of the FIRST char of an identifier-like token. The lexer
/// stamps every token with its first char's position, so this is the column
/// itself (kept as a named helper for intent at the call sites).
fn ident_start_col(tok: &Token) -> usize {
    tok.column.max(1)
}

fn is_type_token(tt: &TokenType) -> bool {
    matches!(
        tt,
        TokenType::KwInt | TokenType::KwDecimal | TokenType::KwDec | TokenType::KwString
        | TokenType::KwBool | TokenType::KwAny | TokenType::KwVoid | TokenType::KwNull
    )
}

/// Slice the original source from `start` to `end` (1-based, inclusive of the
/// end token's first character), collapsed to one line. Used for signatures.
fn source_slice(lines: &[String], start: (usize, usize), end: (usize, usize)) -> String {
    let (sl, sc) = (start.0.saturating_sub(1), start.1.saturating_sub(1));
    let (el, ec) = (end.0.saturating_sub(1), end.1);
    let mut out = String::new();
    for li in sl..=el.min(lines.len().saturating_sub(1)) {
        let line = &lines[li];
        let chars: Vec<char> = line.chars().collect();
        let from = if li == sl { sc.min(chars.len()) } else { 0 };
        let to = if li == el { ec.min(chars.len()) } else { chars.len() };
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(chars[from..to].iter().collect::<String>().trim());
        if out.len() > 160 {
            out.truncate(160);
            out.push('…');
            break;
        }
    }
    out
}

/// Token-level symbol scan. Heuristic by design (must work on broken files):
/// - `let|const NAME` (incl. destructuring) → variable/constant
/// - `class|interface|enum NAME` → type; brace-depth tracking assigns members
/// - `fn ... NAME (` → function
/// - `NAME ( ... ) =>` or (inside a class) `NAME ( ... ) {` → function/method
/// - `import "path"` → import
fn scan_symbols(_text: &str, lines: &[String]) -> Vec<SymbolInfo> {
    let tokens = collect_tokens(_text);
    let mut symbols = Vec::new();
    let mut depth: i32 = 0;
    // (class_name, brace depth at which its body opened)
    let mut class_stack: Vec<(String, i32)> = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let t = &tokens[i];
        match t.token_type {
            TokenType::LBrace => depth += 1,
            TokenType::RBrace => {
                depth -= 1;
                if let Some((_, d)) = class_stack.last() {
                    if depth < *d {
                        class_stack.pop();
                    }
                }
            }
            TokenType::Let | TokenType::KwConst => {
                let kind = if t.token_type == TokenType::KwConst {
                    SymbolKind::Constant
                } else {
                    SymbolKind::Variable
                };
                // let NAME = …  /  let [a, b] = …  /  let {k, k: alias} = …
                let mut j = i + 1;
                let destructure = matches!(
                    tokens.get(j).map(|t| &t.token_type),
                    Some(TokenType::LBracket) | Some(TokenType::LBrace)
                );
                let mut names: Vec<&Token> = Vec::new();
                if destructure {
                    while j < tokens.len()
                        && !matches!(tokens[j].token_type, TokenType::Assign | TokenType::Semicolon | TokenType::Eof)
                    {
                        if tokens[j].token_type == TokenType::Ident {
                            // in `{key: alias}` only the alias binds; keep the
                            // last ident of each comma group
                            if matches!(tokens.get(j + 1).map(|t| &t.token_type), Some(TokenType::Colon)) {
                                j += 1;
                                continue;
                            }
                            names.push(&tokens[j]);
                        }
                        j += 1;
                    }
                } else if let Some(nt) = tokens.get(j) {
                    if nt.token_type == TokenType::Ident {
                        names.push(nt);
                    }
                }
                for nt in names {
                    symbols.push(SymbolInfo {
                        name: nt.literal.clone(),
                        kind,
                        line: nt.line,
                        column: ident_start_col(nt),
                        detail: lines
                            .get(nt.line.saturating_sub(1))
                            .map(|l| l.trim().trim_end_matches('{').trim().to_string())
                            .unwrap_or_default(),
                        container: class_stack.last().map(|(n, _)| n.clone()),
                    });
                }
            }
            TokenType::KwClass | TokenType::KwInterface | TokenType::KwEnum => {
                if let Some(nt) = tokens.get(i + 1) {
                    if nt.token_type == TokenType::Ident {
                        let kind = match t.token_type {
                            TokenType::KwClass => SymbolKind::Class,
                            TokenType::KwInterface => SymbolKind::Interface,
                            _ => SymbolKind::Enum,
                        };
                        symbols.push(SymbolInfo {
                            name: nt.literal.clone(),
                            kind,
                            line: nt.line,
                            column: ident_start_col(nt),
                            detail: lines
                                .get(nt.line.saturating_sub(1))
                                .map(|l| l.trim().trim_end_matches('{').trim().to_string())
                                .unwrap_or_default(),
                            container: None,
                        });
                        if kind == SymbolKind::Class {
                            // body opens at the next LBrace; members live at depth+1
                            class_stack.push((nt.literal.clone(), depth + 1));
                        }
                    }
                }
            }
            TokenType::Function => {
                // fn [type…] NAME ( — the name is the last ident before `(`
                let mut j = i + 1;
                while j < tokens.len() && j - i < 8 {
                    if tokens[j].token_type == TokenType::Ident
                        && matches!(tokens.get(j + 1).map(|t| &t.token_type), Some(TokenType::LParen))
                    {
                        push_callable(&tokens, j, lines, &class_stack, &mut symbols);
                        break;
                    }
                    if !is_type_token(&tokens[j].token_type)
                        && !matches!(
                            tokens[j].token_type,
                            TokenType::Ident | TokenType::LBracket | TokenType::RBracket
                            | TokenType::Question | TokenType::Asterisk
                            | TokenType::Lt | TokenType::Gt | TokenType::Comma
                        )
                    {
                        break;
                    }
                    j += 1;
                }
            }
            TokenType::Ident => {
                // NAME ( … ) => — a typed standalone function; inside a class,
                // NAME ( … ) { is a method/constructor. Skip call syntax
                // `obj.m(...)` (previous token is a dot).
                let prev = i.checked_sub(1).map(|p| &tokens[p].token_type);
                let prev_is_dot = matches!(prev, Some(TokenType::Dot) | Some(TokenType::QuestionDot));
                let prev_is_fn = matches!(prev, Some(TokenType::Function));
                let prev_is_new = matches!(prev, Some(TokenType::KwNew));
                if !prev_is_dot && !prev_is_fn && !prev_is_new
                    && matches!(tokens.get(i + 1).map(|t| &t.token_type), Some(TokenType::LParen))
                {
                    if let Some(close) = matching_paren(&tokens, i + 1) {
                        let after = tokens.get(close + 1).map(|t| &t.token_type);
                        let in_class = class_stack
                            .last()
                            .map(|(_, d)| depth == *d)
                            .unwrap_or(false);
                        let is_arrow_fn = matches!(after, Some(TokenType::Arrow));
                        let is_class_method = in_class && matches!(after, Some(TokenType::LBrace));
                        if is_arrow_fn || is_class_method {
                            push_callable(&tokens, i, lines, &class_stack, &mut symbols);
                        }
                    }
                }
            }
            TokenType::KwImport => {
                if let Some(nt) = tokens.get(i + 1) {
                    if nt.token_type == TokenType::String {
                        symbols.push(SymbolInfo {
                            name: nt.literal.clone(),
                            kind: SymbolKind::Import,
                            line: nt.line,
                            column: nt.column,
                            detail: format!("import \"{}\"", nt.literal),
                            container: None,
                        });
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    symbols
}

/// Index of the RParen matching the LParen at `open`, if any.
fn matching_paren(tokens: &[Token], open: usize) -> Option<usize> {
    let mut level = 0;
    for (k, t) in tokens.iter().enumerate().skip(open) {
        match t.token_type {
            TokenType::LParen => level += 1,
            TokenType::RParen => {
                level -= 1;
                if level == 0 {
                    return Some(k);
                }
            }
            TokenType::Eof => break,
            _ => {}
        }
    }
    None
}

/// Record a function/method whose name token is at `name_idx` (followed by
/// `(`). The detail is the source from the start of the declaration line
/// through the closing paren, e.g. `fn int suma(int a, int b)`.
fn push_callable(
    tokens: &[Token],
    name_idx: usize,
    lines: &[String],
    class_stack: &[(String, i32)],
    symbols: &mut Vec<SymbolInfo>,
) {
    let name_tok = &tokens[name_idx];
    let close = match matching_paren(tokens, name_idx + 1) {
        Some(c) => c,
        None => return,
    };
    let container = class_stack.last().map(|(n, _)| n.clone());
    let kind = match &container {
        Some(class_name) if *class_name == name_tok.literal => SymbolKind::Constructor,
        Some(_) => SymbolKind::Method,
        None => SymbolKind::Function,
    };
    let close_tok = &tokens[close];
    let detail = source_slice(
        lines,
        (name_tok.line, 1),
        (close_tok.line, close_tok.column + 1),
    );
    symbols.push(SymbolInfo {
        name: name_tok.literal.clone(),
        kind,
        line: name_tok.line,
        column: ident_start_col(name_tok),
        detail,
        container,
    });
}

// ── Cursor helpers (used by completion/hover/definition) ─────────────────────

pub fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The identifier under the given 0-based (line, character) position, plus an
/// optional receiver if the word is written as `receiver.word`.
pub fn word_at(lines: &[String], line: usize, character: usize) -> Option<(String, Option<String>)> {
    let text = lines.get(line)?;
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let mut idx = character.min(chars.len());
    // allow hovering just past the last char of a word
    if idx >= chars.len() || !is_ident_char(chars[idx]) {
        if idx == 0 || !is_ident_char(chars[idx - 1]) {
            return None;
        }
        idx -= 1;
    }
    let mut start = idx;
    while start > 0 && is_ident_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = idx;
    while end + 1 < chars.len() && is_ident_char(chars[end + 1]) {
        end += 1;
    }
    let word: String = chars[start..=end].iter().collect();
    if word.is_empty() || word.chars().next().map(|c| c.is_numeric()).unwrap_or(true) {
        return None;
    }
    let receiver = receiver_before(&chars, start);
    Some((word, receiver))
}

/// If the text right before `pos` is `receiver.` return the receiver name.
fn receiver_before(chars: &[char], pos: usize) -> Option<String> {
    if pos == 0 || chars.get(pos - 1) != Some(&'.') {
        return None;
    }
    let mut end = pos - 1; // at '.'
    let mut start = end;
    while start > 0 && is_ident_char(chars[start - 1]) {
        start -= 1;
    }
    if start == end {
        return None;
    }
    end -= 0;
    let recv: String = chars[start..end].iter().collect();
    if recv.is_empty() { None } else { Some(recv) }
}

/// Completion context at a 0-based position: text of the current word prefix
/// and the receiver if the cursor sits after `receiver.`.
pub struct CompletionCx {
    pub prefix: String,
    pub receiver: Option<String>,
}

pub fn completion_context(lines: &[String], line: usize, character: usize) -> CompletionCx {
    let empty = String::new();
    let text = lines.get(line).unwrap_or(&empty);
    let chars: Vec<char> = text.chars().collect();
    let upto = character.min(chars.len());
    let mut start = upto;
    while start > 0 && is_ident_char(chars[start - 1]) {
        start -= 1;
    }
    let prefix: String = chars[start..upto].iter().collect();
    let receiver = receiver_before(&chars, start);
    CompletionCx { prefix, receiver }
}

/// Best definition for `name` seen from `from_line` (0-based): prefer
/// functions/classes/enums anywhere, then the closest declaration above the
/// use, then anything with the name.
pub fn find_definition<'a>(
    symbols: &'a [SymbolInfo],
    name: &str,
    from_line: usize,
) -> Option<&'a SymbolInfo> {
    let matches: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.name == name).collect();
    if matches.is_empty() {
        return None;
    }
    if let Some(s) = matches.iter().find(|s| {
        matches!(
            s.kind,
            SymbolKind::Function | SymbolKind::Class | SymbolKind::Enum | SymbolKind::Interface
        )
    }) {
        return Some(s);
    }
    matches
        .iter()
        .filter(|s| s.line <= from_line + 1)
        .last()
        .or(matches.first())
        .copied()
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"import "utils.sz";

fn int suma(int a, int b) {
    return a + b;
}

let total = suma(1, 2);
const LIMITE = 100;
let [x, y] = [1, 2];

class Animal {
    public Animal(string n) {
        this.nombre = n;
    }
    public string getNombre() {
        return this.nombre;
    }
}

enum Color { Red, Green, Blue }

fn int doble(int n) {
    return n * 2;
}
"#;

    fn sym<'a>(symbols: &'a [SymbolInfo], name: &str) -> &'a SymbolInfo {
        symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("symbol '{}' not found in {:?}", name,
                symbols.iter().map(|s| &s.name).collect::<Vec<_>>()))
    }

    #[test]
    fn clean_file_has_no_diagnostics() {
        let a = analyze(SAMPLE);
        assert!(a.diagnostics.is_empty(), "unexpected: {:?}", a.diagnostics);
    }

    #[test]
    fn parse_error_produces_positioned_diagnostic() {
        // note: the parser recovers silently from several malformed inputs
        // (`let x = ;` parses without error) — use one that does report
        let a = analyze("if (true {\n    out 1;\n}\n");
        assert!(!a.diagnostics.is_empty());
        let d = &a.diagnostics[0];
        assert_eq!(d.line, 1);
        assert_eq!(d.severity, 1);
    }

    #[test]
    fn type_error_produces_warning_diagnostic() {
        let a = analyze("fn int f(int a) {\n    return a;\n}\nf(1, 2, 3);\n");
        let warn = a.diagnostics.iter().find(|d| d.severity == 2)
            .expect("expected an arity warning");
        assert_eq!(warn.line, 4);
        assert!(warn.message.contains("argument"), "{}", warn.message);
    }

    #[test]
    fn scans_functions_classes_and_variables() {
        let a = analyze(SAMPLE);
        let f = sym(&a.symbols, "suma");
        assert_eq!(f.kind, SymbolKind::Function);
        assert_eq!(f.line, 3);
        assert!(f.detail.contains("fn int suma(int a, int b)"), "{}", f.detail);

        assert_eq!(sym(&a.symbols, "total").kind, SymbolKind::Variable);
        assert_eq!(sym(&a.symbols, "LIMITE").kind, SymbolKind::Constant);
        assert_eq!(sym(&a.symbols, "x").kind, SymbolKind::Variable);
        assert_eq!(sym(&a.symbols, "y").kind, SymbolKind::Variable);
        assert_eq!(sym(&a.symbols, "Animal").kind, SymbolKind::Class);
        assert_eq!(sym(&a.symbols, "Color").kind, SymbolKind::Enum);
        assert_eq!(sym(&a.symbols, "utils.sz").kind, SymbolKind::Import);

        // second typed function (the named arrow form `int doble(int n) => {}`
        // is NOT valid serez — the parser now reports it; see t6 regression)
        let d = sym(&a.symbols, "doble");
        assert_eq!(d.kind, SymbolKind::Function);
        assert!(d.detail.contains("int doble(int n)"), "{}", d.detail);
    }

    #[test]
    fn class_members_have_container() {
        let a = analyze(SAMPLE);
        let ctor = sym(&a.symbols, "Animal");
        assert_eq!(ctor.kind, SymbolKind::Class);
        let m = sym(&a.symbols, "getNombre");
        assert_eq!(m.kind, SymbolKind::Method);
        assert_eq!(m.container.as_deref(), Some("Animal"));
        // constructor: same name as the class, inside it
        let c = a.symbols.iter().find(|s| s.name == "Animal" && s.kind == SymbolKind::Constructor);
        assert!(c.is_some(), "constructor not detected");
    }

    #[test]
    fn calls_are_not_reported_as_functions() {
        let a = analyze(SAMPLE);
        // `suma(1, 2)` on line 7 must not create a second `suma` symbol
        let count = a.symbols.iter().filter(|s| s.name == "suma").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn word_and_receiver_detection() {
        let lines: Vec<String> = vec!["let r = File.read(path);".to_string()];
        let (word, recv) = word_at(&lines, 0, 14).expect("word");
        assert_eq!(word, "read");
        assert_eq!(recv.as_deref(), Some("File"));

        let (word, recv) = word_at(&lines, 0, 9).expect("word");
        assert_eq!(word, "File");
        assert_eq!(recv, None);
    }

    #[test]
    fn completion_context_detects_prefix_and_receiver() {
        let lines: Vec<String> = vec!["  OS.ex".to_string()];
        let cx = completion_context(&lines, 0, 7);
        assert_eq!(cx.prefix, "ex");
        assert_eq!(cx.receiver.as_deref(), Some("OS"));

        let cx = completion_context(&lines, 0, 5);
        assert_eq!(cx.prefix, "");
        assert_eq!(cx.receiver.as_deref(), Some("OS"));
    }

    #[test]
    fn definition_prefers_functions_and_nearest_let() {
        let a = analyze(SAMPLE);
        let d = find_definition(&a.symbols, "suma", 6).expect("def");
        assert_eq!(d.line, 3);
        let d = find_definition(&a.symbols, "total", 20).expect("def");
        assert_eq!(d.line, 7);
    }

    #[test]
    fn symbols_survive_parse_errors() {
        // while typing, the file is usually broken — symbols must still come out
        let a = analyze("fn int suma(int a, int b) {\n    return a + b;\n}\nif (true {\nlet z = 1;\n");
        assert!(!a.diagnostics.is_empty());
        assert_eq!(sym(&a.symbols, "suma").kind, SymbolKind::Function);
        assert_eq!(sym(&a.symbols, "z").kind, SymbolKind::Variable);
    }
}
