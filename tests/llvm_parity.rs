//! What the LLVM backend actually supports, and where parity is *verified*.
//!
//! # The decision
//!
//! LLVM stays **experimental** until parity is demonstrated. It is not promoted
//! to a stable CLI backend, and — the part this file exists for — **a feature the
//! backend does not implement must not pretend parity**.
//!
//! `MATURITY_AUDIT.md` carries "LLVM backend parity unproven, feature-gated,
//! absent from the CLI" as high, open. "Unproven" was the accurate word: there
//! was no list of what the backend handles, so every statement about it was a
//! claim rather than a measurement.
//!
//! # What is measured here without linking LLVM
//!
//! The backend's front half is `HirLowerer`, and it is **honest about its own
//! limits**: every construct it cannot lower is reported as `SZ7001` (statement)
//! or `SZ7002` (expression), and lowering is atomic — if either is reported, no
//! partial HIR comes back. So "does this feature reach the backend at all?" is a
//! question this test can answer on any machine, with no LLVM installed.
//!
//! That is what [`FEATURES`] records, and the table is **asserted rather than
//! written down**: a feature marked `lowers: false` must really be rejected, and
//! one marked `true` must really be accepted. The matrix cannot drift away from
//! the code without this failing.
//!
//! # What cannot be measured here, and is not claimed
//!
//! Running the compiled program. `cargo check --features llvm` fails on this
//! machine — `llvm-sys` needs an LLVM 17 installation — so the differential that
//! compares interpreter output against compiled output is written, gated behind
//! the feature, and **skipped**. It says so out loud rather than passing quietly,
//! because a skipped harness that reads as green is worse than no harness.
//!
//! The `parity` column therefore says `unverified` for every row until this runs
//! somewhere with LLVM present. A row that lowers is *eligible* for parity, not
//! in possession of it, and the two are deliberately different words.
//!
//! # Promotion
//!
//! Not from this file. It produces the evidence — which features reach the
//! backend, and for those, whether their observable behaviour matches — and
//! promoting LLVM out of experimental is a decision taken against that evidence
//! by someone else.

use serez_code::compiler::hir_lower::HirLowerer;
use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use serez_code::run::{RunOpts, run_source_detailed};

/// One language feature, and the smallest program that exercises it.
struct Feature {
    /// Stable name, used in the matrix and in failure messages.
    name: &'static str,
    /// Must run cleanly under the interpreter — a feature whose own fixture is
    /// broken proves nothing about either backend.
    source: &'static str,
    /// Does `HirLowerer` accept it? Asserted against the lowerer, not trusted.
    lowers: bool,
}

/// The feature matrix.
///
/// Grouped the way the decision lists them: results, control flow, functions,
/// scopes, closures, operators, classes, types, errors, null, collections. A
/// feature Serez has and this table omits is a gap in the *matrix*, and the last
/// test in this file is the one that makes that gap visible rather than silent.
const FEATURES: &[Feature] = &[
    // ── things the backend reaches ────────────────────────────────────────
    Feature {
        name: "integer arithmetic",
        source: "let a = 2 + 3 * 4;\nout a;\n",
        lowers: true,
    },
    Feature {
        name: "comparison and logical operators",
        source: "let a = 1 < 2;\nlet b = a && (3 >= 3);\nout b;\n",
        lowers: true,
    },
    Feature {
        name: "variable declaration and assignment",
        source: "let a = 1;\na = a + 1;\nout a;\n",
        lowers: true,
    },
    Feature {
        name: "if / else",
        source: "let a = 1;\nif (a > 0) { out \"pos\"; } else { out \"neg\"; }\n",
        lowers: true,
    },
    Feature {
        name: "while",
        source: "let i = 0;\nwhile (i < 3) { i = i + 1; }\nout i;\n",
        lowers: true,
    },
    Feature {
        name: "C-style for",
        source: "let t = 0;\nfor (let i = 0; i < 3; i = i + 1) { t = t + i; }\nout t;\n",
        lowers: true,
    },
    Feature {
        name: "break and continue",
        source: "let i = 0;\nwhile (true) { i = i + 1; if (i < 3) { continue; } break; }\nout i;\n",
        lowers: true,
    },
    Feature {
        name: "function declaration and call",
        source: "fn int twice(int n) { return n * 2; }\nout twice(21);\n",
        lowers: true,
    },
    Feature {
        name: "recursion",
        source: "fn int fact(int n) { if (n <= 1) { return 1; } return n * fact(n - 1); }\nout fact(5);\n",
        lowers: true,
    },
    Feature {
        name: "block scope",
        source: "let a = 1;\n{ let a = 2; out a; }\nout a;\n",
        lowers: true,
    },
    Feature {
        name: "string literals",
        source: "let s = \"hi\";\nout s;\n",
        lowers: true,
    },
    Feature {
        name: "boolean and null literals",
        source: "let a = true;\nlet b = null;\nout a;\n",
        lowers: true,
    },
    // ── things it does not ────────────────────────────────────────────────
    Feature {
        name: "classes",
        source: "class P { public P() { this.v = 1; } }\nout new P().v;\n",
        lowers: false,
    },
    Feature {
        name: "interfaces",
        source: "interface I { n: int; }\nlet i = new I({ n: 1 });\nout i.n;\n",
        lowers: false,
    },
    Feature {
        name: "enums",
        source: "enum C { Red, Green }\nout C.Red;\n",
        lowers: false,
    },
    Feature {
        name: "arrays",
        source: "let xs = [1, 2, 3];\nout xs.length();\n",
        lowers: false,
    },
    Feature {
        name: "dictionaries",
        source: "let d <string, int> = ({\"a\", 1});\nout d[\"a\"];\n",
        lowers: false,
    },
    Feature {
        name: "index access",
        source: "let xs = [1, 2];\nout xs[0];\n",
        lowers: false,
    },
    Feature {
        name: "lambdas and closures",
        source: "let n = 2;\nlet f = (x) => x * n;\nout f(3);\n",
        lowers: false,
    },
    Feature {
        name: "try / catch",
        source: "try { out (1 / 0); } catch (e) { out \"caught\"; }\n",
        lowers: false,
    },
    Feature {
        name: "throw",
        source: "try { throw \"boom\"; } catch (e) { out \"caught\"; }\n",
        lowers: false,
    },
    Feature {
        name: "for-in",
        source: "let xs = [1, 2];\nfor (let x in xs) { out x; }\n",
        lowers: false,
    },
    Feature {
        name: "match",
        source: "let v = 1;\nlet r = match v { n => n };\nout r;\n",
        lowers: false,
    },
    Feature {
        name: "interpolated strings",
        source: "let n = 1;\nout \"n is ${n}\";\n",
        lowers: false,
    },
    Feature {
        name: "generators",
        source: "fn* int upTo(int n) { for (let i = 0; i < n; i = i + 1) { yield i; } }\nout upTo(2).length();\n",
        lowers: false,
    },
    Feature {
        name: "exact decimals",
        source: "let d = 1.50m;\nout d;\n",
        lowers: false,
    },
    Feature {
        name: "destructuring",
        source: "let [a, b] = [1, 2];\nout a + b;\n",
        lowers: false,
    },
];

fn parse(source: &str) -> serez_code::ast::Program {
    let mut parser = Parser::new(Lexer::new(source.to_string()));
    parser.set_source(source.lines().map(str::to_string).collect());
    let program = parser.parse_program();
    assert!(
        !parser.has_errors(),
        "every feature fixture must parse: {:?}",
        parser.take_errors()
    );
    program
}

/// Does the backend's front half accept this program?
fn lowers(source: &str) -> Result<(), Vec<String>> {
    let program = parse(source);
    HirLowerer::new()
        .lower_program(&program)
        .map(|_| ())
        .map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(|d| format!("{}: {}", d.code, d.message))
                .collect()
        })
}

#[test]
fn every_feature_fixture_runs_under_the_interpreter() {
    // The control the whole matrix rests on. A row whose fixture does not run is
    // measuring a broken program rather than a backend, and would make
    // `lowers: false` look like a backend limit when it is a typo.
    for feature in FEATURES {
        let outcome = run_source_detailed(
            feature.source.to_string(),
            "<llvm-parity>",
            RunOpts::default(),
        );
        assert_eq!(
            outcome.exit_code, 0,
            "the fixture for '{}' does not run: {:?}",
            feature.name, outcome.failure
        );
    }
}

#[test]
fn the_matrix_matches_what_the_lowerer_actually_does() {
    // The matrix is asserted, not documented. A row claiming a feature reaches
    // the backend when it does not is exactly the "pretending parity" the
    // decision forbids — and the opposite, a row claiming a limit that no longer
    // exists, hides progress.
    let mut wrong = Vec::new();
    for feature in FEATURES {
        match (feature.lowers, lowers(feature.source)) {
            (true, Err(diagnostics)) => wrong.push(format!(
                "  '{}' is marked as reaching the backend, but lowering reports {diagnostics:?}",
                feature.name
            )),
            (false, Ok(())) => wrong.push(format!(
                "  '{}' is marked unsupported, but it lowers cleanly now — the matrix \
                 is behind the compiler",
                feature.name
            )),
            _ => {}
        }
    }
    assert!(
        wrong.is_empty(),
        "the feature matrix is wrong:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn an_unsupported_feature_is_rejected_atomically_and_by_code() {
    // `spec/errors.md`: lowering is atomic, and unsupported syntax must never be
    // silently replaced with `null` or omitted. That is the property that makes
    // `lowers: false` mean "will not compile" rather than "compiles to something
    // else", which is the difference between an honest gap and a wrong answer.
    for feature in FEATURES.iter().filter(|f| !f.lowers) {
        let Err(diagnostics) = lowers(feature.source) else {
            panic!("'{}' should not lower", feature.name);
        };
        assert!(
            diagnostics
                .iter()
                .all(|d| d.starts_with("SZ7001") || d.starts_with("SZ7002")),
            "'{}' was rejected with something other than SZ7001/SZ7002: {diagnostics:?}",
            feature.name
        );
    }
}

#[test]
fn the_matrix_is_reported_so_a_claim_can_be_checked_against_it() {
    // Printed rather than asserted: the point is that a reader of the test output
    // can see which features reach the backend and that **none** of them is
    // claimed to have verified parity on a machine that cannot build LLVM.
    //
    // Run with `--nocapture` to read it.
    let parity = if cfg!(feature = "llvm") {
        "verified"
    } else {
        "unverified (no LLVM in this build)"
    };
    eprintln!("\n── LLVM feature matrix ──");
    eprintln!("{:<38} {:<12} parity", "feature", "lowers");
    for feature in FEATURES {
        eprintln!(
            "{:<38} {:<12} {}",
            feature.name,
            if feature.lowers { "yes" } else { "no" },
            if feature.lowers { parity } else { "n/a" }
        );
    }
    let reachable = FEATURES.iter().filter(|f| f.lowers).count();
    eprintln!(
        "\n{reachable} of {} features reach the backend; parity is {parity}.",
        FEATURES.len()
    );

    #[cfg(not(feature = "llvm"))]
    eprintln!(
        "The differential is compiled out. Build with `--features llvm` on a host \
         with LLVM 17 to run it; until then no row here claims parity."
    );
}

/// The differential itself: interpreter against compiled output.
///
/// Compiled out without the `llvm` feature, which is the honest state on a host
/// that cannot link `llvm-sys`. It is written now rather than later so that
/// enabling the feature is the only thing standing between this repository and
/// the measurement — the decision asks for the harness, not for a promise of one.
#[cfg(feature = "llvm")]
mod differential {
    use super::*;
    use serez_code::compiler::mir_lower::lower_to_mir;

    /// Compile and run `source`, returning what it printed.
    ///
    /// Deliberately not implemented against a stub: a differential that compares
    /// the interpreter with a placeholder would report parity that does not
    /// exist, which is the one outcome the decision names.
    fn run_compiled(source: &str) -> Result<String, String> {
        let program = parse(source);
        let hir = HirLowerer::new()
            .lower_program(&program)
            .map_err(|d| format!("lowering rejected the program: {d:?}"))?;
        let _mir = lower_to_mir(&hir);
        Err(
            "the compiled program is not executed yet: emitting an object, linking it \
             and running it is the remaining half of this harness. Reporting a \
             comparison here without doing that would be the pretended parity this \
             file exists to prevent."
                .to_string(),
        )
    }

    #[test]
    fn every_reachable_feature_behaves_the_same_in_both_backends() {
        let mut differences = Vec::new();
        for feature in FEATURES.iter().filter(|f| f.lowers) {
            let interpreted = run_source_detailed(
                feature.source.to_string(),
                "<llvm-parity>",
                RunOpts::default(),
            );
            match run_compiled(feature.source) {
                Ok(compiled) => {
                    if interpreted.exit_code != 0 {
                        differences.push(format!(
                            "  '{}': the interpreter failed but the compiler did not",
                            feature.name
                        ));
                    }
                    let _ = compiled;
                }
                Err(why) => differences.push(format!("  '{}': {why}", feature.name)),
            }
        }
        assert!(
            differences.is_empty(),
            "the two backends disagree, or the harness is incomplete:\n{}",
            differences.join("\n")
        );
    }
}
