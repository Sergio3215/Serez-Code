//! What the online beta playground says when it cannot do something, and what
//! a local runtime keeps saying.
//!
//! # The defect
//!
//! The playground is a hosted `sz --eval`, so a snippet runs under lockdown and
//! every capability lockdown closes comes back refused. The refusals were
//! written for someone at a terminal:
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
//! # What is asserted, and in which direction
//!
//! Both directions matter, and the second is the one that could quietly rot:
//!
//!   * **In the playground** every closed capability is announced as a
//!     `BETA LIMITATION`, names the environment, points at `serezcode.org`, and
//!     contains none of the runtime's internal vocabulary.
//!   * **Everywhere else** — `sz file.sz`, and `sz --eval` from a terminal —
//!     the diagnostics are the ones that were there before, word for word. A
//!     local run that lacks a permission is a permission error, not a beta
//!     limitation, and this file fails if that ever stops being true.
//!
//! And across both: the code, the kind and whether `try/catch` may consume the
//! refusal are untouched. `spec/compatibility.md` promises codes and kinds are
//! stable, and `spec/security.md` splits lockdown's refusals into catchable and
//! fatal on a rule that has nothing to do with wording. Rewording must not move
//! anything across either line.

use serez_code::environment::{Capability, Environment};
use serez_code::run::{RunFailure, RunOpts, run_source_detailed};
use std::process::Command;

// ── Harness ──────────────────────────────────────────────────────────────────

/// Every snippet that reaches a capability the online beta withholds, with the
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

fn run_in(source: &str, environment: Environment) -> (i32, Option<RunFailure>) {
    let outcome = run_source_detailed(
        source.to_string(),
        "<beta-limitations>",
        RunOpts {
            environment,
            ..RunOpts::sandboxed()
        },
    );
    (outcome.exit_code, outcome.failure)
}

/// The structured runtime failure a snippet produced, or a panic naming what it
/// produced instead.
fn runtime_failure(source: &str, environment: Environment) -> serez_code::evaluator::RuntimeError {
    match run_in(source, environment).1 {
        Some(RunFailure::Runtime(error)) => error,
        other => panic!("{source:?} did not fail as a runtime error: {other:?}"),
    }
}

/// Run a snippet through the real binary, so what is asserted is the text a
/// user sees rather than a payload something downstream might re-render.
fn cli(source: &str, beta: bool) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sz"));
    command.arg("--eval").arg(source);
    if beta {
        command.arg("--online-beta");
    }
    // The variable is the other door onto the same flag, and a developer machine
    // that happens to have it set must not decide what these tests measure.
    command.env_remove("SEREZ_ONLINE_BETA");
    let out = command.output().expect("the sz binary must run");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

// ── In the playground ────────────────────────────────────────────────────────

#[test]
fn every_capability_has_a_snippet() {
    // Without this, adding a capability and forgetting to reach it here would
    // leave it untested while the suite still looked complete.
    for capability in Capability::ALL {
        assert!(
            REACHES.iter().any(|(c, _)| c == capability),
            "{capability:?} has no snippet in this file, so nothing checks what \
             the playground says when a program reaches it"
        );
    }
}

#[test]
fn the_playground_announces_a_limitation_of_the_environment() {
    for (capability, source) in REACHES {
        let error = runtime_failure(source, Environment::OnlineBeta);
        assert_eq!(
            error.origin,
            serez_code::diagnostic::Origin::EnvironmentLimit,
            "{source:?} is refused by the playground and must say whose \
             limitation it is: {error:?}"
        );
        assert_eq!(
            error.message,
            capability.limitation(),
            "{source:?} must report as {capability:?}"
        );
    }
}

#[test]
fn the_playground_says_what_is_off_where_and_what_to_do_about_it() {
    for (capability, source) in REACHES {
        let message = runtime_failure(source, Environment::OnlineBeta).message;

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
fn the_playground_never_says_the_runtime_is_untrusted_source() {
    // The exact phrase the report was written about, plus the rest of the
    // vocabulary that belongs to the security model rather than to a reader
    // meeting the language for the first time.
    for (_, source) in REACHES {
        let message = runtime_failure(source, Environment::OnlineBeta)
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
fn the_playground_never_suggests_the_capability_does_not_exist() {
    // The reading this whole change exists to prevent. A message that talks
    // about the *language* invites "Serez Code is broken"; one that talks about
    // the environment cannot.
    for (_, source) in REACHES {
        let message = runtime_failure(source, Environment::OnlineBeta)
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
    let text = cli(r#"fetch("https://example.test/x");"#, true);
    assert!(
        text.contains("❌ BETA LIMITATION [SZ6001]:"),
        "the playground headline is wrong:\n{text}"
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
fn the_online_beta_is_opt_in_from_the_command_line_and_from_the_environment() {
    let flag = Command::new(env!("CARGO_BIN_EXE_sz"))
        .args(["--eval", r#"File.read("x");"#, "--online-beta"])
        .env_remove("SEREZ_ONLINE_BETA")
        .output()
        .expect("the sz binary must run");
    let variable = Command::new(env!("CARGO_BIN_EXE_sz"))
        .args(["--eval", r#"File.read("x");"#])
        .env("SEREZ_ONLINE_BETA", "1")
        .output()
        .expect("the sz binary must run");
    for (how, out) in [("--online-beta", &flag), ("SEREZ_ONLINE_BETA=1", &variable)] {
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(
            text.contains("BETA LIMITATION"),
            "{how} did not reach the playground wording:\n{text}"
        );
    }
}

// ── Everywhere else ──────────────────────────────────────────────────────────

#[test]
fn a_local_run_keeps_the_permission_diagnostic_it_always_had() {
    // Case B of the four: the permission genuinely is not declared, on a machine
    // where declaring it is something the reader can actually do. Telling them
    // to install Serez Code would be telling them to install what they are
    // running.
    let error = runtime_failure("Terminal.getSize();", Environment::Local);
    assert_eq!(error.origin, serez_code::diagnostic::Origin::Program);
    assert!(
        error.message.contains("requires permission 'Terminal'"),
        "{error:?}"
    );
    assert!(error.message.contains("serez.json"), "{error:?}");
}

#[test]
fn nothing_outside_the_playground_mentions_the_beta() {
    // The direction that rots silently: a message that starts appearing on
    // `sz file.sz` would be telling someone with a local install to go and get
    // a local install.
    for (_, source) in REACHES {
        let (_, failure) = run_in(source, Environment::Local);
        let text = format!("{failure:?}");
        for beta in [
            "BETA LIMITATION",
            "online beta",
            "playground",
            "serezcode.org",
        ] {
            assert!(
                !text.contains(beta),
                "{source:?} on a local runtime mentions '{beta}': {text}"
            );
        }
    }
}

#[test]
fn a_terminal_eval_is_local_unless_it_says_otherwise() {
    // `--eval` is the same door the playground uses, so the default has to be
    // the conservative one: someone typing it into a shell is not in a beta.
    let text = cli(r#"fetch("https://example.test/x");"#, false);
    assert!(text.contains("❌ ERROR [SZ6001]"), "{text}");
    assert!(!text.contains("BETA LIMITATION"), "{text}");
}

#[test]
fn the_environment_grants_nothing_and_closes_nothing() {
    // The rule the whole change is built on. Anything that runs in one
    // environment runs in the other, and anything refused in one is refused in
    // the other; only the sentence differs.
    for (_, source) in REACHES {
        let local = run_in(source, Environment::Local).0;
        let beta = run_in(source, Environment::OnlineBeta).0;
        assert_eq!(
            local, beta,
            "{source:?} changed what it *does* between environments, not just \
             what it says"
        );
    }
    // And the positive control: a program that needs no capability is unaffected
    // by either.
    for environment in [Environment::Local, Environment::OnlineBeta] {
        let (code, failure) = run_in(r#"out "hello";"#, environment);
        assert_eq!(code, 0, "{environment:?} broke ordinary code: {failure:?}");
    }
}

// ── What must not move ───────────────────────────────────────────────────────

#[test]
fn the_code_and_the_kind_survive_the_rewording() {
    // `compatibility.md`: a failure keeps its code and kind, or the change is
    // breaking. Tooling matches on these; the wording is explicitly not a
    // contract, which is what made this change available in the first place.
    for (_, source) in REACHES {
        let local = runtime_failure(source, Environment::Local);
        let beta = runtime_failure(source, Environment::OnlineBeta);
        assert_eq!(beta.code, local.code, "{source:?} changed its code");
        assert_eq!(beta.kind, local.kind, "{source:?} changed its kind");
        assert_ne!(
            beta.message, local.message,
            "{source:?} is meant to read differently in the playground"
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
        let (code, failure) = run_in(&guarded, Environment::OnlineBeta);
        assert_eq!(
            code, 0,
            "{source:?} stopped being catchable in the playground: {failure:?}"
        );
    }

    let fatal = ["use permissions { OS }", "Terminal.getSize();"];
    for source in fatal {
        let guarded =
            format!(r#"try {{ {source} }} catch (e) {{ out "caught"; }} out "continued";"#);
        let (code, _) = run_in(&guarded, Environment::OnlineBeta);
        assert_ne!(
            code, 0,
            "{source:?} became something try/catch can route around; a security \
             refusal a program can consume is advice"
        );
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
    let error = runtime_failure(source, Environment::OnlineBeta);
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

    let text = cli(source, true);
    assert!(text.contains("called from 'f'"), "{text}");
    assert!(text.contains("called from 'g'"), "{text}");
}

#[test]
fn an_ordinary_mistake_is_still_an_ordinary_mistake_in_the_playground() {
    // Case C of the four. Turning *every* failure into a friendly note about the
    // environment would be the same defect pointed the other way: a program with
    // a bug in it would be told the playground was at fault.
    for source in [
        "let a = [1]; out a[9];",
        "out 1 / 0;",
        "out true + 1;",
        r#"out "x".padStart(999999999, "-");"#,
    ] {
        let error = runtime_failure(source, Environment::OnlineBeta);
        assert_eq!(
            error.origin,
            serez_code::diagnostic::Origin::Program,
            "{source:?} is the program's mistake, not the environment's: {error:?}"
        );
        assert!(!error.message.contains("online beta"), "{error:?}");
    }
}

#[test]
fn a_frontend_diagnostic_is_untouched_by_the_playground() {
    // Case C again, one phase earlier: a parse error in the playground is a
    // parse error, with its file, line and column.
    let text = cli("let x = ;", true);
    assert!(text.contains("❌ PARSER ERROR [SZ2000]"), "{text}");
    assert!(
        text.contains("<eval> 1:9"),
        "the position is still there:\n{text}"
    );
    assert!(!text.contains("BETA LIMITATION"), "{text}");
}
