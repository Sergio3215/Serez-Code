//! What an `--eval` run says when it cannot do something, and what a run with a
//! file behind it keeps saying.
//!
//! # The defect
//!
//! The online playground is a hosted `sz --eval`, so a snippet runs under
//! lockdown and every capability lockdown closes comes back refused. The
//! refusals were written for someone at a terminal:
//!
//! ```text
//! ❌ ERROR [SZ6001]: fetch to 'pokeapi.co' is not available here — this code
//! runs as untrusted source, and the network is closed unless the host allows
//! it explicitly.
//! ```
//!
//! For a reader whose first contact with Serez Code is a web page, that reads
//! as *`fetch` is broken*. Nothing in it says the capability exists, works, and
//! is simply not switched on here.
//!
//! # Why the entry point decides, and nothing is passed
//!
//! There used to be a `--online-beta` flag saying "this is the playground". It
//! said twice what `--eval` already says once: a snippet handed in as a string
//! has no file, therefore no `serez.json`, therefore nothing that *"declare it
//! in serez.json"* could possibly refer to. So [`RunMode::Eval`] is the whole
//! condition, and a deployment has nothing to remember to switch on.
//!
//! # What is asserted, and in which direction
//!
//! Both directions matter, and the second is the one that could quietly rot:
//!
//!   * **Under `--eval`** every closed capability is announced as a
//!     `BETA LIMITATION`, names the environment, points at `serezcode.org`, and
//!     contains none of the runtime's internal vocabulary.
//!   * **With a file behind the run** the diagnostics are the ones that were
//!     there before, word for word. A local run that lacks a permission is a
//!     permission error, not a beta limitation, and this file fails if that ever
//!     stops being true.
//!
//! And across both: the code, the kind and whether `try/catch` may consume the
//! refusal are untouched. `spec/compatibility.md` promises codes and kinds are
//! stable, and `spec/security.md` splits lockdown's refusals into catchable and
//! fatal on a rule that has nothing to do with wording. Rewording must not move
//! anything across either line.

use serez_code::environment::Capability;
use serez_code::run::{RunFailure, RunMode, RunOpts, run_source_detailed};
use std::process::Command;

// ── Harness ──────────────────────────────────────────────────────────────────

/// Every snippet that reaches a capability an `--eval` run withholds, with the
/// capability it should be announced as.
///
/// One row per entry in [`Capability::ALL`]; `every_capability_has_a_snippet`
/// keeps it that way, so a capability added to the runtime cannot be added
/// without a program that actually reaches it.
const REACHES: &[(Capability, &str)] = &[
    (Capability::Network, r#"fetch("https://example.test/x");"#),
    (Capability::Filesystem, r#"File.read("data.txt");"#),
    (Capability::Modules, r#"import "math";"#),
    (
        Capability::ModelWeights,
        r#"Autodiff.saveWeights("w.szw");"#,
    ),
    (Capability::PermissionGrants, "use permissions { OS }"),
    (
        Capability::ProcessExecution,
        r#"unsafe { OS.exec("whoami"); }"#,
    ),
    (Capability::Terminal, "Terminal.getSize();"),
    (Capability::EnvironmentVariables, r#"Env.get("PATH");"#),
    (Capability::SystemInfo, "System.osName();"),
    (Capability::Sockets, r#"Socket.connect("a.test", 80);"#),
    (Capability::Media, r#"Media.playSound("a.wav");"#),
    (Capability::Gui, r#"Gui.createWindow("t", 100, 100);"#),
    (Capability::BackgroundTasks, r#"Task.spawn("w.sz", "");"#),
    (Capability::Clock, "DateTime.now();"),
];

/// Run a snippet in `mode`, with lockdown on either way.
///
/// Lockdown is held constant on purpose: it is what actually closes these
/// capabilities, so holding it fixed and moving only the mode isolates the one
/// thing under test — the wording. [`RunMode::File`] here is the embedder's
/// locked-down file run, which is the strictest form of "not `--eval`": if beta
/// wording were going to leak into a run that is not `--eval`, this is where it
/// would.
fn run_in(source: &str, mode: RunMode) -> (i32, Option<RunFailure>) {
    let outcome = run_source_detailed(
        source.to_string(),
        "<beta-limitations>",
        RunOpts {
            mode,
            ..RunOpts::sandboxed()
        },
    );
    (outcome.exit_code, outcome.failure)
}

/// The structured runtime failure a snippet produced, or a panic naming what it
/// produced instead.
fn runtime_failure(source: &str, mode: RunMode) -> serez_code::evaluator::RuntimeError {
    match run_in(source, mode).1 {
        Some(RunFailure::Runtime(error)) => error,
        other => panic!("{source:?} did not fail as a runtime error: {other:?}"),
    }
}

/// Run a snippet through the real binary as `sz --eval`, so what is asserted is
/// the text a user sees rather than a payload something downstream might
/// re-render.
///
/// `SEREZ_ONLINE_BETA` is cleared because it used to be the other door onto the
/// removed flag: a developer machine that still has it set must not be able to
/// change what these tests measure, in either direction.
fn cli(source: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_sz"))
        .args(["--eval", source])
        .env_remove("SEREZ_ONLINE_BETA")
        .output()
        .expect("the sz binary must run");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Run a snippet through the real binary as `sz file.sz` — the other door, and
/// the one this file exists to keep unchanged.
///
/// The file goes in a directory of its own with no `serez.json` beside it, so
/// the program starts with no permissions and the refusal under test is a
/// genuine missing declaration rather than an artefact of the fixture.
/// `extra_env` is for asserting that a variable does *not* matter.
fn cli_file(source: &str, tag: &str, extra_env: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir().join(format!("sz_beta_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join("program.sz");
    std::fs::write(&path, source).expect("the program must be written");

    let mut command = Command::new(env!("CARGO_BIN_EXE_sz"));
    command.arg(&path).env_remove("SEREZ_ONLINE_BETA");
    for (key, value) in extra_env {
        command.env(key, value);
    }
    let out = command.output().expect("the sz binary must run");

    let _ = std::fs::remove_dir_all(&dir);
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

// ── Under `--eval` ───────────────────────────────────────────────────────────

#[test]
fn every_capability_has_a_snippet() {
    // Without this, adding a capability and forgetting to reach it here would
    // leave it untested while the suite still looked complete.
    for capability in Capability::ALL {
        assert!(
            REACHES.iter().any(|(c, _)| c == capability),
            "{capability:?} has no snippet in this file, so nothing checks what \
             an --eval run says when a program reaches it"
        );
    }
}

#[test]
fn an_eval_run_announces_a_limitation_of_the_environment() {
    for (capability, source) in REACHES {
        let error = runtime_failure(source, RunMode::Eval);
        assert_eq!(
            error.origin,
            serez_code::diagnostic::Origin::EnvironmentLimit,
            "{source:?} is refused under --eval and must say whose limitation it \
             is: {error:?}"
        );
        assert_eq!(
            error.message,
            capability.limitation(),
            "{source:?} must report as {capability:?}"
        );
    }
}

#[test]
fn an_eval_run_says_what_is_off_where_and_what_to_do_about_it() {
    for (capability, source) in REACHES {
        let message = runtime_failure(source, RunMode::Eval).message;

        // 1. What is off, and where. Not "an error occurred".
        assert!(
            message.contains("is disabled in this online beta environment."),
            "{source:?} does not say where the limitation lives: {message}"
        );
        // 2. Why: a playground, not a reduced language.
        assert!(
            message.contains("This playground is intended for quick testing"),
            "{source:?} does not say the environment is a demo: {message}"
        );
        // 3. What to do, naming the thing they typed.
        assert!(
            message.contains("serezcode.org"),
            "{source:?} does not say where the full runtime is: {message}"
        );
        assert!(
            message.contains(capability.example()),
            "{source:?} does not name what the reader was reaching for: {message}"
        );
    }
}

#[test]
fn an_eval_run_never_says_the_runtime_is_untrusted_source() {
    // The exact phrase the report was written about, plus the rest of the
    // vocabulary that belongs to the security model rather than to a reader
    // meeting the language for the first time.
    for (_, source) in REACHES {
        let message = runtime_failure(source, RunMode::Eval)
            .message
            .to_lowercase();
        for jargon in [
            "untrusted source",
            "untrusted",
            "lockdown",
            "sandbox",
            "permission policy",
            "the host allows",
            "serez.json",
        ] {
            assert!(
                !message.contains(jargon),
                "{source:?} leaks '{jargon}' into the message a playground user \
                 reads: {message}"
            );
        }
    }
}

#[test]
fn an_eval_run_never_suggests_the_capability_does_not_exist() {
    // The reading this whole change exists to prevent. A message that talks
    // about the *language* invites "Serez Code is broken"; one that talks about
    // the environment cannot.
    for (_, source) in REACHES {
        let message = runtime_failure(source, RunMode::Eval)
            .message
            .to_lowercase();
        for wrong in [
            "not supported",
            "unsupported",
            "does not exist",
            "not implemented",
            "unknown function",
            "no such",
        ] {
            assert!(
                !message.contains(wrong),
                "{source:?} reads as a missing feature rather than a switched-off \
                 one: {message}"
            );
        }
    }
}

#[test]
fn the_rendered_headline_is_a_limitation_and_not_an_error() {
    // Measured against the binary: `❌ ERROR` is what made the report's example
    // look like a defect in the language, and it is the one word that must not
    // be there.
    let text = cli(r#"fetch("https://example.test/x");"#);
    assert!(
        text.contains("❌ BETA LIMITATION [SZ6001]:"),
        "the --eval headline is wrong:\n{text}"
    );
    assert!(
        !text.contains("ERROR"),
        "a working capability is still announced as an error:\n{text}"
    );
    // The message starts on its own line: three paragraphs hung off `: ` bury
    // the sentence that answers the reader's question.
    assert!(
        text.contains("[SZ6001]:\nNetwork access is disabled"),
        "the message did not start on its own line:\n{text}"
    );
}

#[test]
fn eval_needs_no_flag_to_reach_the_beta_wording() {
    // The point of the refactor: the plain command a playground runs, with
    // nothing appended and nothing exported, is already the beta context.
    for (_, source) in REACHES {
        let text = cli(source);
        assert!(
            text.contains("BETA LIMITATION"),
            "plain `sz --eval` did not reach the beta wording for {source:?}:\n{text}"
        );
        assert!(
            text.contains("serezcode.org"),
            "{source:?} did not say where the full runtime is:\n{text}"
        );
    }
}

// ── The removed surface ──────────────────────────────────────────────────────

#[test]
fn online_beta_is_no_longer_a_flag_the_cli_accepts() {
    // It was redundant with `--eval` and it is gone. A run that still passes it
    // must be told, rather than being silently humoured — the flag loop already
    // rejects unknown flags, and this pins that `--online-beta` is now one.
    let out = Command::new(env!("CARGO_BIN_EXE_sz"))
        .args(["--online-beta", "program.sz"])
        .output()
        .expect("the sz binary must run");
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(
        text.contains("Unknown flag '--online-beta'"),
        "the removed flag is still accepted somewhere:\n{text}"
    );
    assert_ne!(out.status.code(), Some(0));
}

#[test]
fn the_help_text_documents_no_flag_that_no_longer_exists() {
    let out = Command::new(env!("CARGO_BIN_EXE_sz"))
        .arg("--help")
        .output()
        .expect("the sz binary must run");
    let text = String::from_utf8_lossy(&out.stdout);
    for gone in ["--online-beta", "SEREZ_ONLINE_BETA"] {
        assert!(
            !text.contains(gone),
            "`sz --help` still offers '{gone}':\n{text}"
        );
    }
    // And the door that replaced it is still documented.
    assert!(text.contains("--eval"), "{text}");
}

#[test]
fn the_environment_variable_no_longer_decides_anything() {
    // `SEREZ_ONLINE_BETA=1` was the second door onto the removed flag. A run
    // with a file behind it must be unmoved by it: the variable is not
    // consulted, so it cannot turn a local permission error into a beta
    // limitation.
    let text = cli_file(
        "Terminal.getSize();\n",
        "envvar",
        &[("SEREZ_ONLINE_BETA", "1")],
    );
    assert!(
        !text.contains("BETA LIMITATION"),
        "a removed environment variable still steers the wording:\n{text}"
    );
    assert!(
        text.contains("requires permission 'Terminal'"),
        "the local diagnostic changed:\n{text}"
    );
}

// ── With a file behind the run ───────────────────────────────────────────────

#[test]
fn a_local_run_keeps_the_permission_diagnostic_it_always_had() {
    // Case B of the four: the permission genuinely is not declared, on a machine
    // where declaring it is something the reader can actually do. Telling them
    // to install Serez Code would be telling them to install what they are
    // running.
    let error = runtime_failure("Terminal.getSize();", RunMode::File);
    assert_eq!(error.origin, serez_code::diagnostic::Origin::Program);
    assert!(
        error.message.contains("requires permission 'Terminal'"),
        "{error:?}"
    );
    assert!(error.message.contains("serez.json"), "{error:?}");
}

#[test]
fn sz_file_sz_reports_a_missing_permission_as_an_error_through_the_binary() {
    // The same case measured end to end, because `sz file.sz` is the command
    // this refactor must leave untouched and the mode is now inferred rather
    // than passed.
    let text = cli_file("Terminal.getSize();\n", "perm", &[]);
    assert!(text.contains("❌ ERROR [SZ6001]"), "{text}");
    assert!(text.contains("requires permission 'Terminal'"), "{text}");
    assert!(text.contains("serez.json"), "{text}");
    assert!(!text.contains("BETA LIMITATION"), "{text}");
    assert!(!text.contains("serezcode.org"), "{text}");
}

#[test]
fn nothing_outside_eval_mentions_the_beta() {
    // The direction that rots silently: a message that starts appearing on
    // `sz file.sz` would be telling someone with a local install to go and get
    // a local install.
    for (_, source) in REACHES {
        let (_, failure) = run_in(source, RunMode::File);
        let text = format!("{failure:?}");
        for beta in [
            "BETA LIMITATION",
            "online beta",
            "playground",
            "serezcode.org",
        ] {
            assert!(
                !text.contains(beta),
                "{source:?} on a file run mentions '{beta}': {text}"
            );
        }
    }
}

#[test]
fn the_mode_grants_nothing_and_closes_nothing() {
    // The rule the whole design is built on. Anything that runs in one mode runs
    // in the other, and anything refused in one is refused in the other; only
    // the sentence differs.
    for (_, source) in REACHES {
        let file = run_in(source, RunMode::File).0;
        let eval = run_in(source, RunMode::Eval).0;
        assert_eq!(
            file, eval,
            "{source:?} changed what it *does* between modes, not just what it says"
        );
    }
    // And the positive control: a program that needs no capability is unaffected
    // by either.
    for mode in [RunMode::File, RunMode::Eval] {
        let (code, failure) = run_in(r#"out "hello";"#, mode);
        assert_eq!(code, 0, "{mode:?} broke ordinary code: {failure:?}");
    }
}

#[test]
fn ordinary_code_still_runs_and_prints_under_eval() {
    // The other half of "closes nothing", measured through the binary: the
    // playground's common case is a snippet that needs no capability at all.
    let text = cli("let n = 20 + 22;\nout n;\n");
    assert!(
        text.contains("42"),
        "ordinary code stopped working:\n{text}"
    );
    assert!(!text.contains("BETA LIMITATION"), "{text}");
    assert!(!text.contains("❌"), "{text}");
}

// ── What must not move ───────────────────────────────────────────────────────

#[test]
fn the_code_and_the_kind_survive_the_rewording() {
    // `compatibility.md`: a failure keeps its code and kind, or the change is
    // breaking. Tooling matches on these; the wording is explicitly not a
    // contract, which is what made this change available in the first place.
    for (_, source) in REACHES {
        let file = runtime_failure(source, RunMode::File);
        let eval = runtime_failure(source, RunMode::Eval);
        assert_eq!(eval.code, file.code, "{source:?} changed its code");
        assert_eq!(eval.kind, file.kind, "{source:?} changed its kind");
        assert_ne!(
            eval.message, file.message,
            "{source:?} is meant to read differently under --eval"
        );
    }
}

#[test]
fn the_catchable_ones_stay_catchable_and_the_fatal_ones_stay_fatal() {
    // `spec/security.md` splits lockdown's refusals on whether they refuse an
    // *action* or refuse to hand out *capability*. A reworded refusal that
    // crossed that line would be a security change wearing a UX change's
    // clothes.
    let catchable = [
        r#"File.read("x");"#,
        r#"import "math";"#,
        r#"Autodiff.saveWeights("w.szw");"#,
    ];
    for source in catchable {
        let guarded = format!(
            r#"let seen = ""; try {{ {source} }} catch (e) {{ seen = e.code; }} out seen;"#
        );
        for mode in [RunMode::File, RunMode::Eval] {
            let (code, failure) = run_in(&guarded, mode);
            assert_eq!(
                code, 0,
                "{source:?} stopped being catchable in {mode:?}: {failure:?}"
            );
        }
    }

    let fatal = ["use permissions { OS }", "Terminal.getSize();"];
    for source in fatal {
        let guarded =
            format!(r#"try {{ {source} }} catch (e) {{ out "caught"; }} out "continued";"#);
        for mode in [RunMode::File, RunMode::Eval] {
            let (code, _) = run_in(&guarded, mode);
            assert_ne!(
                code, 0,
                "{source:?} became something try/catch can route around in \
                 {mode:?}; a security refusal a program can consume is advice"
            );
        }
    }
}

#[test]
fn a_stack_trace_still_names_the_frames_and_their_positions() {
    // The three paragraphs are longer than what they replaced, and the frames
    // print underneath them. A renderer that swallowed the trace would take away
    // the only thing that says *where* in the program the call was.
    let source = "fn f() { return fetch(\"https://example.test/x\"); }\n\
                  fn g() { return f(); }\n\
                  out g();\n";
    let error = runtime_failure(source, RunMode::Eval);
    let names: Vec<&str> = error.stack.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["f", "g"], "innermost first: {error:?}");
    for frame in &error.stack {
        assert!(
            frame.span.is_known(),
            "a frame lost its position: {frame:?}"
        );
    }
    assert!(
        error.span.is_known(),
        "the failure lost its span: {error:?}"
    );

    let text = cli(source);
    assert!(text.contains("called from 'f'"), "{text}");
    assert!(text.contains("called from 'g'"), "{text}");
}

#[test]
fn an_ordinary_mistake_is_still_an_ordinary_mistake_under_eval() {
    // Case C of the four. Turning *every* failure into a friendly note about the
    // environment would be the same defect pointed the other way: a program with
    // a bug in it would be told the playground was at fault.
    for source in [
        "let a = [1]; out a[9];",
        "out 1 / 0;",
        "out true + 1;",
        r#"out "x".padStart(999999999, "-");"#,
    ] {
        let error = runtime_failure(source, RunMode::Eval);
        assert_eq!(
            error.origin,
            serez_code::diagnostic::Origin::Program,
            "{source:?} is the program's mistake, not the environment's: {error:?}"
        );
        assert!(!error.message.contains("online beta"), "{error:?}");
    }
}

#[test]
fn a_frontend_diagnostic_is_untouched_under_eval() {
    // Case C again, one phase earlier: a parse error under `--eval` is a parse
    // error, with its file, line and column.
    let text = cli("let x = ;");
    assert!(text.contains("❌ PARSER ERROR [SZ2000]"), "{text}");
    assert!(
        text.contains("<eval> 1:9"),
        "the position is still there:\n{text}"
    );
    assert!(!text.contains("BETA LIMITATION"), "{text}");
}
