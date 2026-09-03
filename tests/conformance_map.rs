//! Every normative rule in `spec/` is proved by a test, and every test that
//! claims a rule claims one that exists.
//!
//! # What this enforces, and why mechanically
//!
//! Before M8, `spec/` and `tests/` were both good and unconnected. A document
//! stated a rule in prose, a test asserted a behaviour, and whether the two were
//! about the same thing was left to a reader. Nothing could answer *"which test
//! proves this sentence?"* or, worse, *"which sentences does nothing prove?"*
//!
//! `spec/conformance.md` defines the scheme: a normative rule carries an
//! identifier at its definition site, `**[MEM-002]**`, and a test declares what it
//! covers with a `conformance: MEM-002` marker. This file is what makes that more
//! than a convention.
//!
//! Three properties, and each one fails for a different reason:
//!
//! 1. **An identifier is defined exactly once.** A duplicate is a numbering
//!    mistake, and it is caught at the moment it is made rather than the first
//!    time someone follows the wrong one.
//! 2. **Every identifier a test claims exists.** A test claiming a rule that was
//!    renamed or deleted is a *stale claim*, and a stale claim is worse than no
//!    claim: it reads as coverage.
//! 3. **Every identifier has at least one test.** This is the property worth
//!    having. It makes it impossible to add a rule to `spec/` and forget to prove
//!    it, because the build fails until it is proved.
//!
//! Property 3 is also why identifiers are added **as an area is covered** rather
//! than all at once. An identifier is a commitment that something verifies the
//! rule. Assigning one to a rule nothing tests would record the gap in a second
//! place instead of closing it.
//!
//! # Coverage is reported, not asserted
//!
//! How much of `spec/` carries identifiers at all is printed, not pinned. It
//! should grow, and a test that fails when it grows would be an obstacle rather
//! than a gate. Run with `--nocapture` to read it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `**[ABC-001]**` at a definition site. The bold markers are required: they are
/// what distinguishes a rule being *defined* from a rule being *referenced* in
/// prose elsewhere in the same document.
///
/// Fenced code blocks are stripped first. `spec/conformance.md` documents the
/// scheme by *showing* a definition inside a fence, and on this checker's first
/// run that example was read as a real definition and reported as a duplicate of
/// the rule it was illustrating. A specification that cannot show its own syntax
/// is the wrong trade; skipping fences is the right one.
fn definitions(text: &str) -> Vec<String> {
    let prose = strip_fences(text);
    let mut out = Vec::new();
    for part in prose.split("**[").skip(1) {
        if let Some(end) = part.find("]**") {
            let id = &part[..end];
            if is_identifier(id) {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// Everything outside ``` fences. Odd-numbered segments are inside a fence.
fn strip_fences(text: &str) -> String {
    const NEWLINE: &str = "\n";
    text.split("```")
        .step_by(2)
        .collect::<Vec<_>>()
        .join(NEWLINE)
}

/// `conformance: ABC-001, ABC-002` in a comment, in either language.
fn claims(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in text.split("conformance:").skip(1) {
        let line = part.lines().next().unwrap_or("");
        for token in line.split(',') {
            let id = token.trim().trim_end_matches(['.', ';', '*', ')']);
            if is_identifier(id) {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// `ABC-001` or `ABCD-001`: two to four uppercase letters, a dash, three digits.
fn is_identifier(candidate: &str) -> bool {
    let Some((prefix, number)) = candidate.split_once('-') else {
        return false;
    };
    (2..=4).contains(&prefix.len())
        && prefix.chars().all(|c| c.is_ascii_uppercase())
        && number.len() == 3
        && number.chars().all(|c| c.is_ascii_digit())
}

fn files_under(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.') || n == "target" || n == "snapshots");
            if !skip {
                out.extend(files_under(&path, extensions));
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.contains(&e))
        {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn relative(path: &Path) -> String {
    path.strip_prefix(crate_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn every_normative_rule_is_defined_once_and_proved_at_least_once() {
    let root = crate_root();

    // ── what spec/ declares ──────────────────────────────────────────────────
    let spec_files = files_under(&root.join("spec"), &["md"]);
    assert!(
        spec_files.len() > 20,
        "the spec/ walk found only {} documents — a walk that finds nothing \
         would satisfy every assertion below",
        spec_files.len()
    );

    let mut defined: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut documents_with_identifiers = 0usize;
    for path in &spec_files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let ids = definitions(&text);
        if !ids.is_empty() {
            documents_with_identifiers += 1;
        }
        for id in ids {
            defined.entry(id).or_default().push(relative(path));
        }
    }

    // ── what tests claim ─────────────────────────────────────────────────────
    let test_files = files_under(&root.join("tests"), &["sz", "rs"]);
    let mut claimed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in &test_files {
        // Do not read this file's own examples as claims.
        if path.file_name().and_then(|n| n.to_str()) == Some("conformance_map.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for id in claims(&text) {
            claimed.entry(id).or_default().push(relative(path));
        }
    }

    // ── 1. defined exactly once ──────────────────────────────────────────────
    let duplicates: Vec<String> = defined
        .iter()
        .filter(|(_, where_)| where_.len() > 1)
        .map(|(id, where_)| format!("  {id} — defined in {}", where_.join(", ")))
        .collect();
    assert!(
        duplicates.is_empty(),
        "{} identifier(s) defined more than once. A number is never reused, so \
         this is a numbering mistake rather than a choice:\n{}",
        duplicates.len(),
        duplicates.join("\n")
    );

    // ── 2. no claim to a rule that does not exist ────────────────────────────
    let stale: Vec<String> = claimed
        .iter()
        .filter(|(id, _)| !defined.contains_key(*id))
        .map(|(id, where_)| format!("  {id} — claimed by {}", where_.join(", ")))
        .collect();
    assert!(
        stale.is_empty(),
        "{} test(s) claim a rule that spec/ does not define. Either the rule was \
         renamed or deleted and the marker was left behind, or the marker has a \
         typo. A stale claim reads as coverage, which is worse than none:\n{}",
        stale.len(),
        stale.join("\n")
    );

    // ── 3. every rule is proved ──────────────────────────────────────────────
    let unproved: Vec<&String> = defined
        .keys()
        .filter(|id| !claimed.contains_key(*id))
        .collect();
    assert!(
        unproved.is_empty(),
        "{} normative rule(s) have no test:\n  {}\n\n\
         An identifier is a commitment that something verifies the rule. Either \
         write the test and mark it `conformance: <id>`, or find an existing test \
         that already proves it and mark that — spec/conformance.md prefers reuse. \
         If the rule is not ready to be proved, it is not ready to have a number.",
        unproved.len(),
        unproved
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    // ── coverage, reported ───────────────────────────────────────────────────
    let prefixes: BTreeSet<&str> = defined
        .keys()
        .filter_map(|id| id.split('-').next())
        .collect();
    println!("\n── conformance coverage ──");
    println!("spec documents            : {}", spec_files.len());
    println!("  carrying identifiers    : {documents_with_identifiers}");
    println!("normative rules defined   : {}", defined.len());
    println!(
        "  proved                  : {} (all of them — asserted)",
        defined.len()
    );
    println!("areas                     : {:?}", prefixes);
    println!(
        "test files claiming rules : {}",
        claimed.values().flatten().collect::<BTreeSet<_>>().len()
    );
    println!("──────────────────────────\n");
}

#[test]
fn the_identifier_grammar_accepts_what_it_should_and_nothing_else() {
    // The scanners are the whole mechanism, so their edges are worth pinning:
    // a loose pattern would silently absorb ordinary prose as rules.
    assert!(is_identifier("MEM-001"));
    assert!(is_identifier("ARR-999"));
    assert!(
        is_identifier("LEXX-001"),
        "four-letter prefixes are allowed"
    );
    assert!(!is_identifier("M-001"), "one letter is too short");
    assert!(!is_identifier("MEMORY-001"), "six letters is too long");
    assert!(!is_identifier("mem-001"), "lowercase is not an identifier");
    assert!(
        !is_identifier("MEM-1"),
        "the number is exactly three digits"
    );
    assert!(!is_identifier("MEM-0001"));
    assert!(!is_identifier("MEM001"), "the dash is required");
    assert!(
        !is_identifier("SZ4002"),
        "a diagnostic code is not a rule id"
    );

    // A definition needs the bold markers; a bare mention in prose does not
    // define anything, or every cross-reference would become a duplicate.
    assert_eq!(definitions("**[MEM-001]** a rule"), vec!["MEM-001"]);
    assert!(definitions("see MEM-001 for the rule").is_empty());
    assert!(definitions("[MEM-001] without bold").is_empty());

    // A fenced example is documentation, not a definition. Found by this
    // checker's first run, on spec/conformance.md's own illustration.
    assert!(
        definitions(
            "```markdown
**[MEM-002]** shown as an example
```"
        )
        .is_empty(),
        "a fenced example must not define a rule"
    );
    assert_eq!(
        definitions(
            "**[ARR-001]** real
```
**[ARR-002]** illustrative
```
"
        ),
        vec!["ARR-001"],
        "and stripping fences must not swallow the prose around them"
    );

    // Claims are comma-separated and tolerate trailing punctuation.
    assert_eq!(
        claims("// conformance: MEM-001, MEM-002\n"),
        vec!["MEM-001", "MEM-002"]
    );
    assert_eq!(claims("//! conformance: MEM-012\n"), vec!["MEM-012"]);
    assert!(claims("// nothing to see here\n").is_empty());
}
