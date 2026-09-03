//! `fetch` under lockdown, and the allowlist that is the only way through it.
//!
//! # What DEC-M7-006 decided
//!
//! Lockdown is the profile for source that arrived from somewhere else —
//! `--eval`, the playground. It closed `File`, `import`, URL import and
//! Autodiff's weight files, and deliberately left `fetch` open, on the reasoning
//! that lockdown was about the machine's own capabilities and the network was a
//! separate question. A conformance test pinned that, in both runners.
//!
//! The name was doing more work than the code. A request goes out from the
//! host's network position, which is the usual SSRF shape: cloud metadata
//! endpoints, services bound to localhost, the host as an open relay.
//!
//! Decided: **blocked by default under lockdown, reachable only through an
//! explicit allowlist**, and the allowlist belongs to the embedder rather than
//! to the program.
//!
//! # Why this file exists next to the conformance tests
//!
//! Both runners cover the gate itself, because refusing needs no network. They
//! cannot cover the **redirect** half, which is the part that is easy to get
//! wrong and easy not to notice:
//!
//! ```text
//! allowed.example  ->  302  ->  forbidden.internal
//! ```
//!
//! `ureq` follows redirects itself, so gating only the URL the program wrote
//! would let the second host be reached with nothing asking. These tests stand
//! up a real HTTP server and check every hop, including the case that must still
//! work — a redirect that stays on an allowed host.
//!
//! `localhost` and `127.0.0.1` are the two hosts throughout. They are one socket
//! and two names, which is exactly what a hostname allowlist matches on, so one
//! server can play both the permitted and the forbidden side.

use serez_code::permissions::{SecurityPolicy, host_of, resolve_location};
use serez_code::run::{RunFailure, RunOpts, run_source_detailed};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

// ── The policy, unit-tested without a socket ─────────────────────────────────

#[test]
fn outside_lockdown_every_url_is_allowed() {
    // The whole point of the default: `sz file.sz` is unchanged.
    let policy = SecurityPolicy::default();
    assert!(!policy.lockdown);
    assert!(policy.allows_fetch("http://anything.example/"));
    assert!(policy.allows_fetch("https://192.168.0.1:8080/x"));
    assert!(policy.allows_fetch("not even a url"));
}

#[test]
fn under_lockdown_nothing_is_allowed_until_a_host_is_added() {
    let mut policy = SecurityPolicy {
        lockdown: true,
        ..Default::default()
    };
    assert!(!policy.allows_fetch("http://allowed.test/"));
    policy.allow_fetch_host("allowed.test");
    assert!(policy.allows_fetch("http://allowed.test/path?q=1"));
    assert!(!policy.allows_fetch("http://evil.test/"));
}

#[test]
fn the_host_match_is_exact_and_case_insensitive() {
    let mut policy = SecurityPolicy {
        lockdown: true,
        ..Default::default()
    };
    policy.allow_fetch_host("Allowed.Test");
    assert!(policy.allows_fetch("http://ALLOWED.TEST/"));
    // No wildcards and no suffix matching: a subdomain is a different host, and
    // a name that merely ends with an allowed one is the classic bypass.
    assert!(!policy.allows_fetch("http://sub.allowed.test/"));
    assert!(!policy.allows_fetch("http://notallowed.test/"));
    assert!(!policy.allows_fetch("http://allowed.test.evil.test/"));
}

#[test]
fn userinfo_cannot_disguise_the_host() {
    // `http://allowed.test@evil.test/` reads as `evil.test` to every HTTP
    // client, and a gate that read the part before the `@` would wave it
    // through. This is the direction that matters.
    let mut policy = SecurityPolicy {
        lockdown: true,
        ..Default::default()
    };
    policy.allow_fetch_host("allowed.test");
    assert_eq!(
        host_of("http://allowed.test@evil.test/x").as_deref(),
        Some("evil.test")
    );
    assert!(!policy.allows_fetch("http://allowed.test@evil.test/x"));
    // And the harmless direction still resolves correctly.
    assert!(policy.allows_fetch("http://user@allowed.test/x"));
}

#[test]
fn a_url_whose_host_cannot_be_read_is_refused_rather_than_guessed() {
    let mut policy = SecurityPolicy {
        lockdown: true,
        ..Default::default()
    };
    policy.allow_fetch_host("allowed.test");
    for url in ["", "http://", "https:///path", "file:///etc/passwd", "junk"] {
        assert!(
            !policy.allows_fetch(url),
            "a gate must not fall open on input it cannot parse: {url:?}"
        );
    }
}

#[test]
fn a_port_is_not_part_of_the_host_and_a_v6_literal_keeps_its_brackets() {
    assert_eq!(host_of("http://a.test:8080/x").as_deref(), Some("a.test"));
    assert_eq!(host_of("http://[::1]:9/x").as_deref(), Some("[::1]"));
    assert_eq!(host_of("http://[::1]/x").as_deref(), Some("[::1]"));
}

#[test]
fn only_the_two_location_shapes_this_runtime_understands_resolve() {
    let here = "http://a.test:9/dir/page";
    assert_eq!(
        resolve_location(here, "http://b.test/next").as_deref(),
        Some("http://b.test/next")
    );
    assert_eq!(
        resolve_location(here, "/next").as_deref(),
        Some("http://a.test:9/next")
    );
    // Everything else is `None`, which the caller turns into a refusal rather
    // than a silent continue.
    for weird in ["//b.test/next", "../up", "next", "", "ftp://b.test/x"] {
        assert!(
            resolve_location(here, weird).is_none(),
            "{weird:?} must not resolve"
        );
    }
}

// ── The gate and the redirects, against a real server ────────────────────────

/// An HTTP server that answers three paths, on a port the OS picks.
///
/// Deliberately hand-rolled and tiny: the tests need a redirect to a *specific*
/// other hostname, which is not something a fixture file can express.
fn spawn_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = handle(&mut stream, port);
        }
    });
    port
}

fn handle(stream: &mut TcpStream, port: u16) -> std::io::Result<()> {
    let mut buf = [0u8; 2048];
    let read = stream.read(&mut buf)?;
    let request = String::from_utf8_lossy(&buf[..read]).into_owned();
    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();

    let response = match path.as_str() {
        "/ok" => {
            "HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nARRIVED".to_string()
        }
        // Same socket, different hostname — the forbidden side.
        "/to-forbidden" => format!(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/ok\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        "/to-same-host" => {
            "HTTP/1.1 302 Found\r\nLocation: /ok\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string()
        }
        _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
    };
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

/// Run a snippet the way `--eval` does, with `hosts` on the allowlist.
fn eval_sandboxed(source: String, hosts: &[&str]) -> (i32, Option<RunFailure>) {
    let outcome = run_source_detailed(
        source,
        "<lockdown-fetch>",
        RunOpts {
            fetch_allowlist: hosts.iter().map(|h| (*h).to_string()).collect(),
            ..RunOpts::sandboxed()
        },
    );
    (outcome.exit_code, outcome.failure)
}

#[test]
fn a_request_to_an_allowed_host_goes_through_under_lockdown() {
    let port = spawn_server();
    let (code, failure) = eval_sandboxed(
        format!(
            "let body = fetch(\"http://localhost:{port}/ok\");\nassert(body == \"ARRIVED\", body);\n"
        ),
        &["localhost"],
    );
    assert_eq!(
        code, 0,
        "an allowed host must reach the network: {failure:?}"
    );
}

#[test]
fn a_request_to_a_host_that_is_not_on_the_list_is_refused() {
    let port = spawn_server();
    let (code, failure) = eval_sandboxed(
        format!("out fetch(\"http://localhost:{port}/ok\");\n"),
        &["allowed.test"],
    );
    assert_ne!(code, 0);
    assert!(
        matches!(failure, Some(RunFailure::Runtime(ref e)) if e.kind.as_deref() == Some("PermissionError")),
        "expected a PermissionError, got {failure:?}"
    );
}

#[test]
fn with_no_allowlist_at_all_nothing_reaches_the_network() {
    let port = spawn_server();
    let (code, failure) =
        eval_sandboxed(format!("out fetch(\"http://localhost:{port}/ok\");\n"), &[]);
    assert_ne!(code, 0, "the default under lockdown is closed: {failure:?}");
}

#[test]
fn the_refusal_is_not_something_try_catch_can_turn_into_control_flow() {
    let port = spawn_server();
    let (code, _) = eval_sandboxed(
        format!(
            "try {{ out fetch(\"http://localhost:{port}/ok\"); }} catch (e) {{ out \"caught\"; }}\n\
             out \"after\";\n"
        ),
        &[],
    );
    assert_ne!(
        code, 0,
        "a security refusal must not be catchable, or the gate is advice"
    );
}

#[test]
fn a_redirect_from_an_allowed_host_to_a_forbidden_one_is_stopped() {
    // The reason this file exists. `ureq` follows redirects itself, so gating
    // only the URL the program wrote would reach `127.0.0.1` here with nothing
    // asking.
    let port = spawn_server();
    let (code, failure) = eval_sandboxed(
        format!(
            "let reached = \"no\";\n\
             try {{ reached = fetch(\"http://localhost:{port}/to-forbidden\"); }}\n\
             catch (e) {{ reached = \"blocked\"; }}\n\
             assert(reached == \"blocked\", reached);\n"
        ),
        &["localhost"],
    );
    assert_eq!(
        code, 0,
        "the redirect reached the forbidden host: {failure:?}"
    );
}

#[test]
fn a_redirect_that_reaches_the_forbidden_host_would_have_said_so() {
    // The positive control for the test above: with **both** names allowed, the
    // same redirect completes. Without this, a `fetch` that simply always failed
    // would pass.
    let port = spawn_server();
    let (code, failure) = eval_sandboxed(
        format!(
            "let body = fetch(\"http://localhost:{port}/to-forbidden\");\n\
             assert(body == \"ARRIVED\", body);\n"
        ),
        &["localhost", "127.0.0.1"],
    );
    assert_eq!(
        code, 0,
        "allowing both hosts must let the redirect through: {failure:?}"
    );
}

#[test]
fn a_redirect_that_stays_on_an_allowed_host_still_works() {
    // The other half of not over-blocking: a root-relative `Location` is the
    // ordinary case, and it must not be collateral damage.
    let port = spawn_server();
    let (code, failure) = eval_sandboxed(
        format!(
            "let body = fetch(\"http://localhost:{port}/to-same-host\");\n\
             assert(body == \"ARRIVED\", body);\n"
        ),
        &["localhost"],
    );
    assert_eq!(
        code, 0,
        "a same-host redirect must still follow: {failure:?}"
    );
}
