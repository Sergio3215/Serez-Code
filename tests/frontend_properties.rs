//! Properties the frontend must hold for input nobody wrote by hand.
//!
//! # Why this exists
//!
//! `tests/frontend_robustness.rs` covers malformed input as a **list**: truncated
//! literals, unterminated constructs, odd Unicode, each one a case somebody
//! thought of. That is the right instrument for shapes a person can predict, and
//! it is the wrong one for the shapes nobody predicted — which is where crashes
//! live. `docs/maturity/ROADMAP_STATE.md` §6 records the gap plainly: depth
//! ceilings, caps and traversal checks all exist, and *fuzzing or property
//! testing of any kind* does not.
//!
//! This is the missing half. It states properties and generates input to attack
//! them, rather than enumerating inputs and asserting outputs.
//!
//! # No dependency, on purpose
//!
//! `proptest` and `arbitrary` are the obvious answers and are declined for the
//! reason `parser_snapshot` declines `DefaultHasher`: this repository keeps its
//! test infrastructure free of crates whose behaviour it does not control, and a
//! generator is thirty lines. The PRNG below is xorshift64\*, seeded by a
//! constant, so **a failure is reproducible by re-running the test** rather than
//! by copying a seed out of a log.
//!
//! # The properties
//!
//! **P1 — the frontend never panics.** Not on any generated input. A panic is not
//! a diagnostic: it has no line number, no exit code the CLI chose, and nothing
//! for the LSP to underline.
//!
//! **P2 — every diagnostic points inside its own source.** A span whose line
//! exceeds the file's, or whose byte offset runs past the end, is a defect that
//! reaches users as a caret in the wrong place or an editor underlining nothing.
//! M2 spent a milestone giving nodes real spans; this is what keeps them honest
//! on input M2 never saw.
//!
//! **P3 — parsing is deterministic.** The same bytes produce the same tree and
//! the same diagnostics. Anything else means the frontend depends on something
//! it should not — iteration order, an address, a clock — and every snapshot in
//! `tests/snapshots/` silently becomes a coin flip.
//!
//! # Where the input comes from
//!
//! Random bytes find shallow bugs quickly and then stop finding anything, because
//! almost nothing random is nearly-valid. The generators that matter are the ones
//! that start from **real source** and damage it: a truncation is what a
//! half-saved file looks like, and a single-character mutation is what a typo
//! looks like. Both are far likelier to reach deep parser paths than soup is, and
//! both are what the corpus makes possible.

use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use std::path::PathBuf;

/// Stack for the generator thread.
///
/// Generated input reaches the parser's depth ceiling on purpose, and a
/// debug-build `parse_expression` frame is ~8 KiB against `cargo test`'s 2 MiB.
/// §5.15 records this wall; `parser_snapshot` and `scope_resolution` both met it.
const MEASUREMENT_STACK: usize = 32 * 1024 * 1024;

/// xorshift64\*: small, well-distributed, and reproducible without a seed file.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // A zero state is a fixed point for xorshift, so it is not reachable.
        Rng(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next() % bound as u64) as usize
        }
    }

    /// One of `items`, uniformly. Takes a slice of references so it works for
    /// `&[&str]` as well as `&[char]`.
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A sample of real corpus files, to damage.
///
/// Sampled rather than exhaustive: the point is variety of *shape*, and reading
/// several hundred files to mutate a few dozen of them would make the test slow
/// without making it stronger.
fn corpus_sample(rng: &mut Rng, count: usize) -> Vec<(String, String)> {
    let tests = crate_root().join("tests");
    let mut all = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&tests) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sz") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
                        .to_string();
                    all.push((name, text));
                }
            }
        }
    }
    all.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(
        all.len() > 100,
        "the corpus walk found {} files; the generators below need real source",
        all.len()
    );

    let mut out = Vec::new();
    for _ in 0..count {
        out.push(all[rng.below(all.len())].clone());
    }
    out
}

/// Tokens and fragments a Serez program is made of, for soup.
const FRAGMENTS: &[&str] = &[
    "fn",
    "let",
    "const",
    "class",
    "interface",
    "enum",
    "return",
    "out",
    "if",
    "else",
    "while",
    "for",
    "in",
    "match",
    "switch",
    "case",
    "try",
    "catch",
    "finally",
    "throw",
    "unsafe",
    "import",
    "export",
    "native",
    "new",
    "this",
    "super",
    "public",
    "private",
    "abstract",
    "sealed",
    "yield",
    "break",
    "continue",
    "int",
    "string",
    "bool",
    "decimal",
    "dec",
    "void",
    "any",
    "true",
    "false",
    "null",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    ";",
    ",",
    ".",
    ":",
    "=>",
    "=",
    "==",
    "!=",
    "<",
    ">",
    "+",
    "-",
    "*",
    "/",
    "%",
    "**",
    "&&",
    "||",
    "!",
    "?",
    "...",
    "\"unterminated",
    "\"s\"",
    "1",
    "1.5",
    "1m",
    "0x",
    "0xFF",
    "//",
    "/*",
    "*/",
    "é",
    "🙂",
    "\u{0}",
    "\\",
    "@",
    "#",
    "$",
    "`",
];

/// Where a generated case came from, so a failure names the generator.
struct Case {
    label: String,
    source: String,
}

/// Truncation: what a half-written or half-saved file looks like.
fn truncations(rng: &mut Rng, count: usize) -> Vec<Case> {
    let mut out = Vec::new();
    for (name, text) in corpus_sample(rng, count) {
        if text.is_empty() {
            continue;
        }
        let mut cut = rng.below(text.len());
        // Source is a `String`, so a cut has to land on a character boundary.
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        out.push(Case {
            label: format!("truncate({name}, {cut})"),
            source: text[..cut].to_string(),
        });
    }
    out
}

/// One character replaced: what a typo looks like.
fn mutations(rng: &mut Rng, count: usize) -> Vec<Case> {
    let mut out = Vec::new();
    for (name, text) in corpus_sample(rng, count) {
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            continue;
        }
        let at = rng.below(chars.len());
        let replacement = rng.pick(&['{', '}', '(', ')', '"', '\\', ';', '\u{0}', 'é', '🙂']);
        let mutated: String = chars
            .iter()
            .enumerate()
            .map(|(i, c)| if i == at { replacement } else { *c })
            .collect();
        out.push(Case {
            label: format!("mutate({name}, char {at} -> {replacement:?})"),
            source: mutated,
        });
    }
    out
}

/// Fragment soup: syntactically shaped noise, which reaches paths random bytes
/// never do because the lexer accepts it.
fn soup(rng: &mut Rng, count: usize) -> Vec<Case> {
    let mut out = Vec::new();
    for n in 0..count {
        let length = 1 + rng.below(120);
        let mut source = String::new();
        for _ in 0..length {
            source.push_str(rng.pick(FRAGMENTS));
            source.push(if rng.below(4) == 0 { '\n' } else { ' ' });
        }
        out.push(Case {
            label: format!("soup({n}, {length} fragments)"),
            source,
        });
    }
    out
}

/// Nesting at and past the depth ceiling, in both shapes §5.15 names.
fn nesting() -> Vec<Case> {
    let mut out = Vec::new();
    for depth in [1usize, 8, 64, 511, 512, 513, 2000] {
        out.push(Case {
            label: format!("parens({depth})"),
            source: format!("let x = {}1{};", "(".repeat(depth), ")".repeat(depth)),
        });
        out.push(Case {
            label: format!("chain({depth})"),
            source: format!("let x = 1{};", " + 1".repeat(depth)),
        });
        out.push(Case {
            label: format!("blocks({depth})"),
            source: format!("{}out 1;{}", "{".repeat(depth), "}".repeat(depth)),
        });
    }
    out
}

/// Every generated case, in a stable order.
fn cases() -> Vec<Case> {
    // One constant seed. A failure is reproduced by re-running the test, not by
    // copying a number out of a log — which is the only reproduction instruction
    // anyone reliably follows.
    let mut rng = Rng::new(0x5E7E_2C0D_E000_0001);
    let mut out = Vec::new();
    out.extend(truncations(&mut rng, 300));
    out.extend(mutations(&mut rng, 300));
    out.extend(soup(&mut rng, 400));
    out.extend(nesting());
    out
}

/// Parse, and report every diagnostic as `code|line|column|start|end`.
fn parse_diagnostics(source: &str) -> Vec<String> {
    let mut lexer = Lexer::new(source.to_string());
    let mut parser = Parser::new(std::mem::replace(&mut lexer, Lexer::new(String::new())));
    parser.set_source(source.lines().map(str::to_string).collect());
    let program = parser.parse_program();
    let mut out: Vec<String> = parser
        .take_errors()
        .into_iter()
        .map(|d| {
            format!(
                "{}|{}|{}|{}|{}",
                d.code, d.span.line, d.span.column, d.span.start, d.span.end
            )
        })
        .collect();
    // The tree participates in determinism: a diagnostic list can match while
    // the tree does not.
    out.push(format!("tree:{:?}", program).len().to_string());
    out
}

fn check_all() {
    let cases = cases();
    assert!(
        cases.len() > 200,
        "the generators produced only {} cases",
        cases.len()
    );

    let mut panics = Vec::new();
    let mut bad_spans = Vec::new();
    let mut nondeterministic = Vec::new();
    // A generator whose every case parsed cleanly would satisfy P1, P2 and P3
    // while exercising none of the error paths they are about. Counted here and
    // asserted at the end, because a fuzzer that rejects nothing tests nothing.
    let mut rejected = 0usize;
    let mut diagnostics_seen = 0usize;

    for case in &cases {
        let lines = case.source.lines().count();
        let bytes = case.source.len();

        // ── P1: no panic ─────────────────────────────────────────────────────
        let source = case.source.clone();
        let first = std::panic::catch_unwind(move || parse_diagnostics(&source));
        let Ok(first) = first else {
            panics.push(case.label.clone());
            continue;
        };

        // ── P2: every span points inside this source ─────────────────────────
        // `first` always ends with the tree-size entry, so more than one entry
        // means the parser reported something.
        if first.len() > 1 {
            rejected += 1;
            diagnostics_seen += first.len() - 1;
        }
        for diagnostic in &first {
            let Some((code, rest)) = diagnostic.split_once('|') else {
                continue;
            };
            let fields: Vec<usize> = rest.split('|').filter_map(|f| f.parse().ok()).collect();
            if fields.len() != 4 {
                continue;
            }
            let (line, _column, start, end) = (fields[0], fields[1], fields[2], fields[3]);
            // Line 0 is the documented "unknown position"; it renders as no
            // position rather than as a wrong one, so it is not a violation.
            if line != 0 && line > lines.max(1) {
                bad_spans.push(format!(
                    "  {}: {code} at line {line}, but the source has {lines}",
                    case.label
                ));
            }
            if start > bytes || end > bytes || end < start {
                bad_spans.push(format!(
                    "  {}: {code} spans bytes {start}..{end} of a {bytes}-byte source",
                    case.label
                ));
            }
        }

        // ── P3: same bytes, same result ──────────────────────────────────────
        let source = case.source.clone();
        let second = std::panic::catch_unwind(move || parse_diagnostics(&source));
        match second {
            Ok(second) if second == first => {}
            Ok(_) => nondeterministic.push(case.label.clone()),
            Err(_) => panics.push(format!("{} (second run only)", case.label)),
        }
    }

    assert!(
        panics.is_empty(),
        "P1 — the frontend panicked on {} of {} generated inputs:\n  {}\n\n\
         A panic is not a diagnostic: no line number, no chosen exit code, nothing \
         for the LSP to underline. Reproduce by re-running this test; the seed is \
         a constant.",
        panics.len(),
        cases.len(),
        panics.join("\n  ")
    );

    assert!(
        bad_spans.is_empty(),
        "P2 — {} diagnostic span(s) point outside their own source:\n{}\n\n\
         A span past the end of the file is a caret in the wrong place for the CLI \
         and an editor underlining nothing.",
        bad_spans.len(),
        bad_spans.join("\n")
    );

    assert!(
        nondeterministic.is_empty(),
        "P3 — {} input(s) parsed differently on a second run:\n  {}\n\n\
         The frontend is depending on something it should not — iteration order, an \
         address, a clock — and every manifest in tests/snapshots/ is a coin flip \
         until it stops.",
        nondeterministic.len(),
        nondeterministic.join("\n  ")
    );

    // The load-bearing check. Without it, every assertion above could hold on a
    // generator that produced only valid programs, and the test would report
    // success for never having tried anything.
    assert!(
        // Half. The generators currently reach about two thirds, so this has
        // headroom; pinning it near the observed figure would make an ordinary
        // generator tweak look like a regression.
        rejected * 2 > cases.len(),
        "only {} of {} generated inputs were rejected. The generators are producing \
         valid programs, so P1, P2 and P3 are holding over paths they are not about \
         — the test would be passing for having tried nothing.",
        rejected,
        cases.len()
    );

    println!(
        "\nfrontend properties: {} generated inputs, {} rejected, {} diagnostics — \
         no panic, every span in range, every parse deterministic\n",
        cases.len(),
        rejected,
        diagnostics_seen
    );
}

#[test]
fn the_frontend_holds_its_properties_on_input_nobody_wrote() {
    std::thread::Builder::new()
        .stack_size(MEASUREMENT_STACK)
        .spawn(check_all)
        .expect("spawn the property thread")
        .join()
        .expect("the property thread panicked")
}

#[test]
fn the_generator_is_reproducible_and_actually_varies() {
    // A generator that returns the same case every time would satisfy every
    // property above and prove nothing, so both halves are pinned.
    let first: Vec<String> = cases().into_iter().map(|c| c.label).collect();
    let second: Vec<String> = cases().into_iter().map(|c| c.label).collect();
    assert_eq!(first, second, "the same seed must produce the same cases");

    let distinct: std::collections::BTreeSet<&String> = first.iter().collect();
    assert!(
        distinct.len() > first.len() * 9 / 10,
        "only {} of {} cases are distinct — the generator is repeating itself",
        distinct.len(),
        first.len()
    );
}
