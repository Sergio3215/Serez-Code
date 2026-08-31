//! The compatibility suite for diagnostic codes.
//!
//! `compatibility.md` states the promise plainly: "Diagnostic **codes**
//! (`SZ1xxx`–`SZ7xxx`) and **kinds** are stable. A failure keeps its code and
//! kind, or the change is treated as breaking." Nothing enforced it.
//!
//! The 63 `err_*` and 85 `sec_*` conformance programs assert only that the exit
//! was non-zero and that a `❌` appeared somewhere on stderr — never which code.
//! A failure could move from `SZ4003` to `SZ4009`, or lose its code entirely,
//! and all 148 would still pass. Twelve codes were named by a Rust test
//! somewhere; eight were named by none.
//!
//! These tests drive the built `sz` binary, so what they pin is the code a user
//! actually sees, not an internal mapping that a boundary might rewrite.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `(code, what produces it, source)`.
///
/// Every source here was run against the binary and the code it emits was
/// recorded from the output — derived from measurement, not from what the
/// registry says it ought to be. Three of the first attempts were wrong, which
/// is the reason for saying so: `"x".repeat(...)` is not a method and reported
/// `SZ4001`, a traversal read reported `SZ4005` rather than a security code,
/// and the import fixture had been deleted by its own setup.
const CASES: &[(&str, &str, &str)] = &[
    ("SZ1001", "an unexpected source character", "let x = 1; @"),
    ("SZ1002", "an unterminated string", "out \"abc"),
    (
        "SZ1003",
        "an unterminated block comment",
        "out 1; /* never closed",
    ),
    ("SZ1004", "a malformed hex integer", "let x = 0x;"),
    ("SZ2000", "a syntax error", "let x = ;"),
    // SZ2001 is built below: it needs a chain deeper than MAX_PARSE_DEPTH.
    (
        "SZ3000",
        "an advisory type diagnostic",
        "fn int f(int a) { return a; }\nf(\"s\");",
    ),
    ("SZ4000", "integer overflow", "out 9223372036854775807 + 1;"),
    ("SZ4001", "an unknown name", "out nope;"),
    ("SZ4002", "a type mismatch", "out true + 1;"),
    ("SZ4003", "an index out of bounds", "let a = [1]; out a[9];"),
    ("SZ4004", "division by zero", "out 1 / 0;"),
    (
        "SZ4005",
        "reading a file that is not there",
        "out File.read(\"no_such_file_xyz.txt\");",
    ),
    (
        "SZ6001",
        "a namespace used without its permission",
        "out DateTime.now();",
    ),
    (
        "SZ6002",
        "a resource ceiling",
        "out \"x\".padStart(999999999, \"-\");",
    ),
    (
        "SZ6003",
        "a gated call outside unsafe",
        "use permissions { OS }\nOS.exec(\"whoami\");",
    ),
    (
        "SZ6004",
        "a command aimed at a protected system path",
        "use permissions { OS }\nunsafe { OS.exec(\"C:\\\\Windows\\\\System32\\\\cmd.exe\", []); }",
    ),
];

/// Codes the registry lists that nothing currently emits.
///
/// This list is allowed to shrink and must not grow. A code in the registry
/// that nothing produces is a promise with no behaviour behind it.
const UNEMITTED: &[(&str, &str)] = &[
    (
        "SZ5001",
        "A missing module is thrown as the historical catchable `ModuleNotFound:` \
         string rather than raised as a structured error, so it carries no code \
         and no span. Turning a thrown string into an Error object is a public \
         breaking change; see compatibility.md. Recorded in modules.md.",
    ),
    (
        "SZ4999",
        "Unreachable by design: it reports a failure the runtime signalled \
         without recording a diagnostic, and nothing reachable does that. \
         `no_reachable_construct_produces_an_unstructured_outcome` in \
         runtime_outcome.rs keeps it that way, and \
         `the_boundary_actually_prints_the_unstructured_diagnostic` keeps the \
         message wired to the arm that needs it.",
    ),
];

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sz")
}

fn workspace() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sz_codes_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// Run one source file and return everything the binary printed.
fn output_for(dir: &Path, name: &str, source: &str) -> String {
    let file = dir.join(name);
    std::fs::write(&file, source).expect("the fixture must be writable");
    let out = Command::new(binary())
        .arg(&file)
        .output()
        .expect("the sz binary must run");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn every_documented_diagnostic_code_is_still_produced_by_its_construct() {
    let dir = workspace();
    let mut wrong: Vec<String> = Vec::new();

    for (index, (code, label, source)) in CASES.iter().enumerate() {
        let text = output_for(&dir, &format!("case_{index}.sz"), source);
        if !text.contains(code) {
            wrong.push(format!(
                "{code} ({label}) is no longer what this reports:\n{}",
                text.trim()
            ));
        }
    }

    // An AST deeper than MAX_PARSE_DEPTH. Built rather than written out because
    // it needs to be past 512 levels, and an operator chain costs one level per
    // operator — see limits.md.
    let chain = format!("let x = {};", vec!["1"; 800].join(" + "));
    let text = output_for(&dir, "depth.sz", &chain);
    if !text.contains("SZ2001") {
        wrong.push(format!(
            "SZ2001 (an AST past MAX_PARSE_DEPTH) is no longer what this reports:\n{}",
            text.trim()
        ));
    }

    // A module that does not parse: the importer must report SZ5002 in addition
    // to the module's own SZ2000, or the reason the import failed is lost.
    std::fs::write(dir.join("broken_module.sz"), "let x = ;\n").expect("fixture");
    let text = output_for(&dir, "importer.sz", "import \"./broken_module\";");
    if !text.contains("SZ5002") {
        wrong.push(format!(
            "SZ5002 (importing a module that does not parse) is no longer what this reports:\n{}",
            text.trim()
        ));
    }

    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        wrong.is_empty(),
        "compatibility.md promises a failure keeps its code, or the change is \
         breaking:\n\n{}",
        wrong.join("\n\n")
    );
}

#[test]
fn the_registry_and_this_suite_name_the_same_codes() {
    // A code can also be broken by removing it from the registry, or by adding
    // one that nothing here covers. Read `spec/errors.md` and compare.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = std::fs::read_to_string(root.join("spec/errors.md"))
        .expect("spec/errors.md must be readable");

    let mut registry: Vec<String> = Vec::new();
    for line in spec.lines() {
        let line = line.trim();
        if !line.starts_with("| `SZ") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("| `") {
            if let Some(end) = rest.find('`') {
                let code = &rest[..end];
                // `errors.md` also carries a table of *ranges* — `SZ1xxx`,
                // `SZ2xxx` and so on. Those are not codes anything emits.
                if code.len() == 6 && code[2..].chars().all(|c| c.is_ascii_digit()) {
                    registry.push(code.to_string());
                }
            }
        }
    }
    registry.sort();
    registry.dedup();
    assert!(
        registry.len() > 10,
        "the registry table in spec/errors.md did not parse: {registry:?}"
    );

    let mut covered: Vec<String> = CASES.iter().map(|(code, _, _)| code.to_string()).collect();
    covered.push("SZ2001".to_string());
    covered.push("SZ5002".to_string());
    covered.extend(UNEMITTED.iter().map(|(code, _)| code.to_string()));
    covered.sort();
    covered.dedup();

    let missing: Vec<&String> = registry.iter().filter(|c| !covered.contains(c)).collect();
    assert!(
        missing.is_empty(),
        "spec/errors.md lists codes this suite does not pin: {missing:?}. Add a \
         case that produces each one, or record it in UNEMITTED with the reason."
    );

    let stale: Vec<&String> = covered.iter().filter(|c| !registry.contains(c)).collect();
    assert!(
        stale.is_empty(),
        "this suite pins codes the registry no longer lists: {stale:?}. Removing \
         a documented code is a breaking change."
    );
}
/// Every kind the runtime raises must appear in `errors.md`, either with a code
/// of its own or in the list of what falls through to `SZ4000`.
///
/// `runtime_error_code` maps eleven kinds and sends everything else to `SZ4000`,
/// so a kind added anywhere in the evaluator silently joins the generic bucket.
/// An earlier revision of `errors.md` said three kinds shared it. That was a
/// sample of what one probe happened to reach rather than a reading of the
/// source, and it understated the position by eleven — `GuiError` alone has
/// forty sites and was missing. This compares the two directly so the count
/// cannot drift again.
#[test]
fn kind_to_code_map_covers_every_kind_raised() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Every `rt_err_kind("X"` / `fatal_err_kind("X"` in the source.
    let mut raised: Vec<String> = Vec::new();
    let mut stack = vec![root.join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap_or_default();
            for marker in ["rt_err_kind(", "fatal_err_kind("] {
                let mut from = 0;
                while let Some(at) = source[from..].find(marker) {
                    let after = from + at + marker.len();
                    from = after;
                    let rest = source[after..].trim_start();
                    let Some(rest) = rest.strip_prefix('"') else {
                        continue;
                    };
                    if let Some(end) = rest.find('"') {
                        let kind = &rest[..end];
                        if !kind.is_empty()
                            && kind.chars().all(|c| c.is_ascii_alphabetic())
                            && !raised.contains(&kind.to_string())
                        {
                            raised.push(kind.to_string());
                        }
                    }
                }
            }
        }
    }
    raised.sort();
    assert!(
        raised.len() > 15,
        "the scan found too few kinds to be reading the source: {raised:?}"
    );

    let spec = std::fs::read_to_string(root.join("spec/errors.md"))
        .expect("spec/errors.md must be readable");
    let missing: Vec<&String> = raised
        .iter()
        .filter(|kind| !spec.contains(&format!("`{kind}`")))
        .collect();
    assert!(
        missing.is_empty(),
        "these kinds are raised by the runtime and named nowhere in errors.md: \
         {missing:?}. A kind with no code of its own falls through to SZ4000, so \
         it belongs in the table of what shares that bucket."
    );
}
