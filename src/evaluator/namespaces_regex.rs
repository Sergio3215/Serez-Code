// Regex namespace — a small, dependency-free regular-expression engine.
//
// Pure Rust (no external crate), to keep the ecosystem self-contained. It is a
// backtracking engine compiled to a tiny bytecode, with a bounded step budget so
// a pathological pattern can never hang or blow the stack (it returns "no match"
// instead — safe by construction).
//
// Supported syntax:
//   literals, `.` (any char except newline), `\d \D \w \W \s \S` and escapes
//   (`\. \\ \n \t \r` …), character classes `[abc]` `[a-z]` `[^…]`, anchors
//   `^` `$`, groups `( … )` and non-capturing `(?: … )`, alternation `|`,
//   quantifiers `* + ?` and `{n}` `{n,}` `{n,m}`, each optionally lazy (`*?` …).
//
// API:
//   Regex.test(pattern, text)               → bool     (matches anywhere)
//   Regex.match(pattern, text)              → [any]?   [whole, g1, g2, …] or null
//   Regex.findAll(pattern, text)            → [string] all non-overlapping matches
//   Regex.replace(pattern, text, repl)      → string   replace all ($0/$&, $1..$9, $$)
//   Regex.split(pattern, text)              → [string] split on matches

use crate::ast;
use crate::region::{ObjectData, OwnedValue};
use super::EvalResult;

// ── AST ─────────────────────────────────────────────────────────────────────
#[derive(Clone)]
enum ClassItem { Ch(char), Range(char, char), Digit, NotDigit, Word, NotWord, Space, NotSpace }

#[derive(Clone)]
enum Ast {
    Empty,
    Char(char),
    Any,
    Class { negated: bool, items: Vec<ClassItem> },
    Start,
    End,
    Concat(Vec<Ast>),
    Alt(Vec<Ast>),
    Group { cap: Option<usize>, inner: Box<Ast> },
    Repeat { inner: Box<Ast>, min: usize, max: Option<usize>, greedy: bool },
}

// ── Parser ──────────────────────────────────────────────────────────────────
struct Parser {
    chars: Vec<char>,
    pos: usize,
    ngroups: usize, // capturing groups seen so far (0 = none); group ids are 1-based
}

impl Parser {
    fn new(pat: &str) -> Self {
        Parser { chars: pat.chars().collect(), pos: 0, ngroups: 0 }
    }
    fn peek(&self) -> Option<char> { self.chars.get(self.pos).copied() }
    fn next(&mut self) -> Option<char> { let c = self.chars.get(self.pos).copied(); if c.is_some() { self.pos += 1; } c }
    fn eat(&mut self, c: char) -> bool { if self.peek() == Some(c) { self.pos += 1; true } else { false } }

    // alt := concat ('|' concat)*
    fn parse_alt(&mut self) -> Result<Ast, String> {
        let mut branches = vec![self.parse_concat()?];
        while self.eat('|') {
            branches.push(self.parse_concat()?);
        }
        if branches.len() == 1 { Ok(branches.pop().unwrap()) } else { Ok(Ast::Alt(branches)) }
    }

    // concat := repeat*
    fn parse_concat(&mut self) -> Result<Ast, String> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' { break; }
            items.push(self.parse_repeat()?);
        }
        if items.is_empty() { Ok(Ast::Empty) }
        else if items.len() == 1 { Ok(items.pop().unwrap()) }
        else { Ok(Ast::Concat(items)) }
    }

    // repeat := atom quantifier?
    fn parse_repeat(&mut self) -> Result<Ast, String> {
        let atom = self.parse_atom()?;
        let (min, max) = match self.peek() {
            Some('*') => { self.pos += 1; (0, None) }
            Some('+') => { self.pos += 1; (1, None) }
            Some('?') => { self.pos += 1; (0, Some(1)) }
            Some('{') => {
                if let Some(bounds) = self.try_parse_brace() { bounds }
                else { return Ok(atom); } // a lone '{' is a literal (handled in atom next round)
            }
            _ => return Ok(atom),
        };
        let greedy = !self.eat('?'); // trailing '?' makes the quantifier lazy
        Ok(Ast::Repeat { inner: Box::new(atom), min, max, greedy })
    }

    // {n} {n,} {n,m}  — returns None (and leaves pos) if not a valid brace form
    fn try_parse_brace(&mut self) -> Option<(usize, Option<usize>)> {
        let save = self.pos;
        self.pos += 1; // consume '{'
        let n = self.parse_number();
        if n.is_none() { self.pos = save; return None; }
        let min = n.unwrap();
        let max;
        if self.eat(',') {
            if self.peek() == Some('}') { max = None; }
            else {
                match self.parse_number() { Some(m) => max = Some(m), None => { self.pos = save; return None; } }
            }
        } else {
            max = Some(min);
        }
        if !self.eat('}') { self.pos = save; return None; }
        Some((min, max))
    }

    fn parse_number(&mut self) -> Option<usize> {
        let start = self.pos;
        while let Some(c) = self.peek() { if c.is_ascii_digit() { self.pos += 1; } else { break; } }
        if self.pos == start { return None; }
        let s: String = self.chars[start..self.pos].iter().collect();
        s.parse::<usize>().ok()
    }

    fn parse_atom(&mut self) -> Result<Ast, String> {
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let cap;
                if self.peek() == Some('?') && self.chars.get(self.pos + 1) == Some(&':') {
                    self.pos += 2; // non-capturing "(?:"
                    cap = None;
                } else {
                    self.ngroups += 1;
                    cap = Some(self.ngroups);
                }
                let inner = self.parse_alt()?;
                if !self.eat(')') { return Err("unbalanced '(' in pattern".to_string()); }
                Ok(Ast::Group { cap, inner: Box::new(inner) })
            }
            Some('[') => self.parse_class(),
            Some('.') => { self.pos += 1; Ok(Ast::Any) }
            Some('^') => { self.pos += 1; Ok(Ast::Start) }
            Some('$') => { self.pos += 1; Ok(Ast::End) }
            Some('\\') => { self.pos += 1; self.parse_escape() }
            Some(')') => Err("unexpected ')' in pattern".to_string()),
            Some(c) => { self.pos += 1; Ok(Ast::Char(c)) }
            None => Ok(Ast::Empty),
        }
    }

    fn parse_escape(&mut self) -> Result<Ast, String> {
        match self.next() {
            Some('d') => Ok(Ast::Class { negated: false, items: vec![ClassItem::Digit] }),
            Some('D') => Ok(Ast::Class { negated: false, items: vec![ClassItem::NotDigit] }),
            Some('w') => Ok(Ast::Class { negated: false, items: vec![ClassItem::Word] }),
            Some('W') => Ok(Ast::Class { negated: false, items: vec![ClassItem::NotWord] }),
            Some('s') => Ok(Ast::Class { negated: false, items: vec![ClassItem::Space] }),
            Some('S') => Ok(Ast::Class { negated: false, items: vec![ClassItem::NotSpace] }),
            Some('n') => Ok(Ast::Char('\n')),
            Some('t') => Ok(Ast::Char('\t')),
            Some('r') => Ok(Ast::Char('\r')),
            Some('0') => Ok(Ast::Char('\0')),
            Some(c) => Ok(Ast::Char(c)), // \. \\ \( … → literal
            None => Err("trailing '\\' in pattern".to_string()),
        }
    }

    fn parse_class(&mut self) -> Result<Ast, String> {
        self.pos += 1; // consume '['
        let negated = self.eat('^');
        let mut items = Vec::new();
        // A ']' immediately after '[' or '[^' is a literal.
        if self.peek() == Some(']') { items.push(ClassItem::Ch(']')); self.pos += 1; }
        while let Some(c) = self.peek() {
            if c == ']' { break; }
            let lo = if c == '\\' {
                self.pos += 1;
                match self.next() {
                    Some('d') => { items.push(ClassItem::Digit); continue; }
                    Some('D') => { items.push(ClassItem::NotDigit); continue; }
                    Some('w') => { items.push(ClassItem::Word); continue; }
                    Some('W') => { items.push(ClassItem::NotWord); continue; }
                    Some('s') => { items.push(ClassItem::Space); continue; }
                    Some('S') => { items.push(ClassItem::NotSpace); continue; }
                    Some('n') => '\n', Some('t') => '\t', Some('r') => '\r',
                    Some(e) => e,
                    None => return Err("unterminated '[' in pattern".to_string()),
                }
            } else { self.pos += 1; c };
            // range: lo '-' hi (but '-' at end is literal)
            if self.peek() == Some('-') && self.chars.get(self.pos + 1).map_or(false, |&x| x != ']') {
                self.pos += 1; // consume '-'
                let hi = match self.next() {
                    Some('\\') => self.next().unwrap_or('\\'),
                    Some(h) => h,
                    None => return Err("unterminated range in class".to_string()),
                };
                items.push(ClassItem::Range(lo, hi));
            } else {
                items.push(ClassItem::Ch(lo));
            }
        }
        if !self.eat(']') { return Err("unterminated '[' in pattern".to_string()); }
        Ok(Ast::Class { negated, items })
    }
}

fn class_item_matches(item: &ClassItem, c: char) -> bool {
    match item {
        ClassItem::Ch(x) => *x == c,
        ClassItem::Range(a, b) => c >= *a && c <= *b,
        ClassItem::Digit => c.is_ascii_digit(),
        ClassItem::NotDigit => !c.is_ascii_digit(),
        ClassItem::Word => c.is_alphanumeric() || c == '_',
        ClassItem::NotWord => !(c.is_alphanumeric() || c == '_'),
        ClassItem::Space => c.is_whitespace(),
        ClassItem::NotSpace => !c.is_whitespace(),
    }
}

// ── Bytecode ────────────────────────────────────────────────────────────────
#[derive(Clone)]
enum Inst {
    Char(char),
    Any,
    Class { negated: bool, items: Vec<ClassItem> },
    Start,
    End,
    Save(usize),
    Split(usize, usize), // try .0 first, then .1
    Jump(usize),
    Match,
}

struct Program { insts: Vec<Inst>, ngroups: usize }

fn compile(ast: &Ast, ngroups: usize) -> Program {
    let mut insts = Vec::new();
    emit(ast, &mut insts);
    insts.push(Inst::Match);
    Program { insts, ngroups }
}

fn emit(ast: &Ast, out: &mut Vec<Inst>) {
    match ast {
        Ast::Empty => {}
        Ast::Char(c) => out.push(Inst::Char(*c)),
        Ast::Any => out.push(Inst::Any),
        Ast::Class { negated, items } => out.push(Inst::Class { negated: *negated, items: items.clone() }),
        Ast::Start => out.push(Inst::Start),
        Ast::End => out.push(Inst::End),
        Ast::Concat(v) => { for a in v { emit(a, out); } }
        Ast::Group { cap, inner } => {
            if let Some(k) = cap {
                out.push(Inst::Save(2 * k));
                emit(inner, out);
                out.push(Inst::Save(2 * k + 1));
            } else {
                emit(inner, out);
            }
        }
        Ast::Alt(branches) => {
            // Split into first vs (rest); chain. Collect jumps-to-end to patch.
            let mut jmp_ends = Vec::new();
            for i in 0..branches.len() {
                if i + 1 < branches.len() {
                    let split_at = out.len();
                    out.push(Inst::Split(0, 0)); // patch below
                    let body = out.len();
                    emit(&branches[i], out);
                    let jmp_at = out.len();
                    out.push(Inst::Jump(0)); // to end, patch later
                    jmp_ends.push(jmp_at);
                    let next = out.len();
                    out[split_at] = Inst::Split(body, next);
                } else {
                    emit(&branches[i], out);
                }
            }
            let end = out.len();
            for j in jmp_ends { out[j] = Inst::Jump(end); }
        }
        Ast::Repeat { inner, min, max, greedy } => {
            for _ in 0..*min { emit(inner, out); }
            match max {
                None => {
                    // star: L: Split(body,out) [greedy]; body: inner; Jump L; out:
                    let l = out.len();
                    let split_at = out.len();
                    out.push(Inst::Split(0, 0));
                    let body = out.len();
                    emit(inner, out);
                    out.push(Inst::Jump(l));
                    let end = out.len();
                    out[split_at] = if *greedy { Inst::Split(body, end) } else { Inst::Split(end, body) };
                }
                Some(m) => {
                    let extra = m.saturating_sub(*min);
                    let mut splits = Vec::new();
                    for _ in 0..extra {
                        let split_at = out.len();
                        out.push(Inst::Split(0, 0));
                        splits.push((split_at, *greedy));
                        let body = out.len();
                        emit(inner, out);
                        // set the "take body" target now; the skip target is patched to end
                        out[split_at] = if *greedy { Inst::Split(body, usize::MAX) } else { Inst::Split(usize::MAX, body) };
                    }
                    let end = out.len();
                    for (s, g) in splits {
                        if let Inst::Split(a, b) = out[s] {
                            out[s] = if g { Inst::Split(a, end) } else { Inst::Split(end, b) };
                        }
                    }
                }
            }
        }
    }
}

const STEP_LIMIT: usize = 1_000_000;
const DEPTH_LIMIT: usize = 8_000;

struct Vm<'a> { insts: &'a [Inst], text: &'a [char], steps: usize }

impl<'a> Vm<'a> {
    // Returns Some(end) if a match starting at the current sp/pc succeeds.
    fn run(&mut self, mut pc: usize, mut sp: usize, saves: &mut Vec<Option<usize>>, depth: usize) -> Option<usize> {
        if depth > DEPTH_LIMIT { return None; }
        loop {
            self.steps += 1;
            if self.steps > STEP_LIMIT { return None; }
            match &self.insts[pc] {
                Inst::Char(c) => {
                    if sp < self.text.len() && self.text[sp] == *c { pc += 1; sp += 1; } else { return None; }
                }
                Inst::Any => {
                    if sp < self.text.len() && self.text[sp] != '\n' { pc += 1; sp += 1; } else { return None; }
                }
                Inst::Class { negated, items } => {
                    if sp < self.text.len() {
                        let hit = items.iter().any(|it| class_item_matches(it, self.text[sp]));
                        if hit != *negated { pc += 1; sp += 1; } else { return None; }
                    } else { return None; }
                }
                Inst::Start => { if sp == 0 { pc += 1; } else { return None; } }
                Inst::End => { if sp == self.text.len() { pc += 1; } else { return None; } }
                Inst::Save(n) => {
                    let n = *n;
                    let old = saves[n];
                    saves[n] = Some(sp);
                    if let Some(r) = self.run(pc + 1, sp, saves, depth + 1) { return Some(r); }
                    saves[n] = old;
                    return None;
                }
                Inst::Jump(a) => { pc = *a; }
                Inst::Split(a, b) => {
                    let (a, b) = (*a, *b);
                    if let Some(r) = self.run(a, sp, saves, depth + 1) { return Some(r); }
                    pc = b;
                }
                Inst::Match => return Some(sp),
            }
        }
    }
}

// Search for the leftmost match at or after `from`. Returns (start, end, saves).
fn search(prog: &Program, text: &[char], from: usize) -> Option<(usize, usize, Vec<Option<usize>>)> {
    let mut start = from;
    loop {
        let mut saves = vec![None; 2 * (prog.ngroups + 1)];
        let mut vm = Vm { insts: &prog.insts, text, steps: 0 };
        if let Some(end) = vm.run(0, start, &mut saves, 0) {
            saves[0] = Some(start);
            saves[1] = Some(end);
            return Some((start, end, saves));
        }
        if start >= text.len() { return None; }
        start += 1;
    }
}

fn slice(text: &[char], a: usize, b: usize) -> String { text[a..b].iter().collect() }

impl super::Evaluator {
    pub(super) fn eval_regex_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        let method = dot_call.method.as_str();

        // First two args are always (pattern, text) strings.
        let need = match method { "replace" => 3, "test" | "match" | "findAll" | "split" => 2, _ => 0 };
        if need == 0 {
            return self.rt_err(format!("Regex has no method '{}'", method));
        }
        if dot_call.arguments.len() != need {
            return self.rt_err(format!("Regex.{} requires {} arguments", method, need));
        }

        let pattern = match self.eval_str_arg(&dot_call.arguments[0]) {
            Some(s) => s,
            None => return self.rt_err("Regex: pattern must be a string"),
        };
        let text_s = match self.eval_str_arg(&dot_call.arguments[1]) {
            Some(s) => s,
            None => return self.rt_err("Regex: text must be a string"),
        };

        // Parse + compile once for this call.
        let mut parser = Parser::new(&pattern);
        let ast = match parser.parse_alt() {
            Ok(a) => a,
            Err(e) => return self.rt_err(format!("Regex: invalid pattern — {}", e)),
        };
        if parser.pos != parser.chars.len() {
            return self.rt_err("Regex: invalid pattern — unexpected trailing characters".to_string());
        }
        let prog = compile(&ast, parser.ngroups);
        let text: Vec<char> = text_s.chars().collect();

        match method {
            "test" => {
                let hit = search(&prog, &text, 0).is_some();
                EvalResult::Value(self.alloc(ObjectData::Boolean(hit)))
            }
            "match" => {
                match search(&prog, &text, 0) {
                    None => EvalResult::Value(self.null_ref),
                    Some((_, _, saves)) => {
                        let mut elems: Vec<OwnedValue> = Vec::new();
                        for g in 0..=prog.ngroups {
                            match (saves[2 * g], saves[2 * g + 1]) {
                                (Some(a), Some(b)) => elems.push(OwnedValue::Str(slice(&text, a, b))),
                                _ => elems.push(OwnedValue::Null),
                            }
                        }
                        EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: elems }))
                    }
                }
            }
            "findAll" => {
                let mut out: Vec<OwnedValue> = Vec::new();
                let mut from = 0;
                loop {
                    match search(&prog, &text, from) {
                        None => break,
                        Some((s, e, _)) => {
                            out.push(OwnedValue::Str(slice(&text, s, e)));
                            from = if e > s { e } else { e + 1 }; // avoid stalling on zero-width
                            if from > text.len() { break; }
                        }
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: out }))
            }
            "split" => {
                let mut out: Vec<OwnedValue> = Vec::new();
                let mut last = 0;
                let mut from = 0;
                loop {
                    match search(&prog, &text, from) {
                        None => break,
                        Some((s, e, _)) => {
                            if e == s {
                                // zero-width match: skip to avoid infinite loop
                                if from >= text.len() { break; }
                                from += 1;
                                continue;
                            }
                            out.push(OwnedValue::Str(slice(&text, last, s)));
                            last = e;
                            from = e;
                        }
                    }
                }
                out.push(OwnedValue::Str(slice(&text, last, text.len())));
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: out }))
            }
            "replace" => {
                let repl = match self.eval_str_arg(&dot_call.arguments[2]) {
                    Some(s) => s,
                    None => return self.rt_err("Regex: replacement must be a string"),
                };
                let repl_chars: Vec<char> = repl.chars().collect();
                let mut result = String::new();
                let mut last = 0;
                let mut from = 0;
                loop {
                    match search(&prog, &text, from) {
                        None => break,
                        Some((s, e, saves)) => {
                            result.push_str(&slice(&text, last, s));
                            expand_replacement(&repl_chars, &text, &saves, prog.ngroups, &mut result);
                            last = e;
                            from = if e > s { e } else {
                                // zero-width: emit the char at s (if any) and advance
                                if s < text.len() { result.push(text[s]); last = s + 1; }
                                s + 1
                            };
                            if from > text.len() { break; }
                        }
                    }
                }
                result.push_str(&slice(&text, last, text.len()));
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }
            _ => self.rt_err(format!("Regex has no method '{}'", method)),
        }
    }
}

// Expand $0/$&, $1..$9 and $$ in a replacement string.
fn expand_replacement(repl: &[char], text: &[char], saves: &[Option<usize>], ngroups: usize, out: &mut String) {
    let mut i = 0;
    while i < repl.len() {
        let c = repl[i];
        if c == '$' && i + 1 < repl.len() {
            let nxt = repl[i + 1];
            if nxt == '$' { out.push('$'); i += 2; continue; }
            if nxt == '&' {
                if let (Some(a), Some(b)) = (saves[0], saves[1]) { out.push_str(&slice(text, a, b)); }
                i += 2; continue;
            }
            if nxt.is_ascii_digit() {
                let g = nxt.to_digit(10).unwrap() as usize;
                if g <= ngroups {
                    if let (Some(a), Some(b)) = (saves[2 * g], saves[2 * g + 1]) { out.push_str(&slice(text, a, b)); }
                    i += 2; continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
}
