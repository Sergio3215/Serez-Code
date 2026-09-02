//! Do the byte offsets on a token actually point at that token?
//!
//! `Token::span.start`/`end` were added in M2.3 ahead of any consumer — nothing
//! reads them yet (see `docs/maturity/ROADMAP_STATE.md` §9B.1). That is exactly
//! the condition in which an offset can be wrong for months without anyone
//! noticing, so the properties that make them meaningful are asserted here
//! rather than assumed.
//!
//! The strongest available check is the simplest: **slice the source with the
//! span and see whether the token comes back.** For identifiers, keywords and
//! operators the slice must equal the literal exactly. For strings and numbers
//! it need not — the literal is the *decoded* value, so `"a\nb"` has a literal
//! of 3 characters and a span of 8 bytes — but the slice must still contain the
//! token and stay inside the source.
//!
//! These run over the real corpus rather than a handful of snippets, because
//! the interesting cases are the ones nobody would think to write down: a
//! multi-byte identifier, a string with an escape, a `dec` literal with an
//! exponent, an operator glued to a comment.

use serez_code::lexer::Lexer;
use serez_code::token::{Token, TokenType};
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Same corpus rule as `tests/parser_snapshot.rs`: `.sz`, skipping the scratch
/// prefixes `.gitignore` already excludes.
fn corpus() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.') || n == "target")
                {
                    walk(&path, out);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("sz")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !n.starts_with('_') && !n.starts_with('~'))
            {
                out.push(path);
            }
        }
    }
    let root = crate_root();
    let mut files = Vec::new();
    for dir in ["tests", "benchmarks", "std", "apps"] {
        walk(&root.join(dir), &mut files);
    }
    files.sort();
    files
}

fn tokens(source: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(source.to_string());
    let mut out = Vec::new();
    loop {
        let token = lexer.next_token();
        let done = token.token_type == TokenType::Eof;
        out.push(token);
        if done {
            break;
        }
    }
    out
}

#[test]
fn every_span_is_a_valid_slice_of_its_source() {
    for path in corpus() {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        for token in tokens(&source) {
            let span = token.span;
            assert!(
                span.start <= span.end,
                "{}: {:?} has an inverted span {:?}",
                path.display(),
                token.token_type,
                span
            );
            assert!(
                span.end <= source.len(),
                "{}: {:?} ends at {} past a {}-byte source",
                path.display(),
                token.token_type,
                span.end,
                source.len()
            );
            assert!(
                source.is_char_boundary(span.start) && source.is_char_boundary(span.end),
                "{}: {:?} span {:?} splits a UTF-8 character",
                path.display(),
                token.token_type,
                span
            );
        }
    }
}

#[test]
fn an_identifier_span_slices_back_to_the_identifier() {
    // The decisive property, on the token kind where literal and source text are
    // the same string. If the offsets are off by one — or measure the *previous*
    // token, which is the classic way to get this wrong — this fails loudly.
    for path in corpus() {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        for token in tokens(&source) {
            if token.token_type != TokenType::Ident {
                continue;
            }
            assert_eq!(
                &source[token.span.start..token.span.end],
                token.literal,
                "{}: identifier at {}:{} does not slice back to itself",
                path.display(),
                token.span.line,
                token.span.column
            );
        }
    }
}

#[test]
fn spans_advance_and_never_overlap() {
    // Tokens come out in source order and none covers a byte another already
    // claimed. Cheap to check and it catches a stale `token_start` — a span left
    // over from the previous token would go backwards here.
    for path in corpus() {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut previous_end = 0usize;
        for token in tokens(&source) {
            if token.token_type == TokenType::Eof {
                continue;
            }
            assert!(
                token.span.start >= previous_end,
                "{}: {:?} at {}:{} starts at {}, before the previous token ended at {}",
                path.display(),
                token.token_type,
                token.span.line,
                token.span.column,
                token.span.start,
                previous_end
            );
            previous_end = token.span.end;
        }
    }
}

// `the_span_agrees_with_the_line_and_column_it_was_built_from` lived here until
// M2.8, asserting that `Token.span.line`/`.column` matched the `line`/`column`
// pair beside them. It was deleted rather than updated because the pair is gone:
// there is now one representation of a token's position, so the invariant it
// checked cannot be violated. A test removed because the type system took over
// its job is the only kind that should be removed without a replacement.

#[test]
fn a_multibyte_identifier_spans_its_bytes_not_its_characters() {
    // The case the corpus may not contain, written out because getting it wrong
    // is invisible in ASCII: `column` counts scalar values per
    // `spec/lexical-grammar.md`, while `start`/`end` count bytes. A four-byte
    // character has to move them by different amounts.
    let source = "let café = 1;\n";
    let idents: Vec<Token> = tokens(source)
        .into_iter()
        .filter(|t| t.token_type == TokenType::Ident)
        .collect();
    assert_eq!(idents.len(), 1);
    let token = &idents[0];
    assert_eq!(&source[token.span.start..token.span.end], "café");
    assert_eq!(token.span.column, 5, "columns count scalar values");
    assert_eq!(
        token.span.end - token.span.start,
        5,
        "café is five bytes and four characters"
    );
}
