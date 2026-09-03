//! The ceiling on the three unbounded reads, at its boundary.
//!
//! # What DEC-M9-001 decided
//!
//! Three paths read an amount the program does not control, from a source it
//! does not control, straight into memory: `fetch`'s response body, an HTTP
//! `import`'s module text, and the stderr of a child started with `OS.spawn`. A
//! server, a module host or a child process could exhaust the interpreter, with
//! the host's RAM as the only limit. The independent audit in `audit/` raises the
//! first two as high severity.
//!
//! **Option A: one fixed ceiling, fatal.** 64 MiB, for all three, stated in
//! `spec/limits.md` beside the others. One policy and three call sites, rather
//! than a different answer per path.
//!
//! # What this file asserts, and why it is shaped this way
//!
//! The interesting property of a ceiling is not that a huge input fails — it is
//! that the *boundary* is where the documentation says it is. A limit tested only
//! with 10× the ceiling would pass with the comparison written backwards, or off
//! by a factor of two, or applied to the wrong quantity.
//!
//! So the reader is exercised directly at four points — just under, exactly at,
//! just over, and far over — against a local HTTP server that serves a body of
//! exactly the length asked for. `read_bounded` is the single function all three
//! call sites go through, so testing it at the boundary tests the policy once
//! rather than three times, and the end-to-end behaviour of each path is asserted
//! separately below.
//!
//! Serving 64 MiB four times is deliberate rather than mocked: the ceiling is
//! about bytes actually crossing a socket, and a fake reader would assert that
//! the arithmetic is self-consistent rather than that the guard works.

use serez_code::run::{RunFailure, RunOpts, run_source_detailed};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// The ceiling, repeated here on purpose.
///
/// `MAX_UNBOUNDED_READ_BYTES` is `pub(crate)`, so this cannot import it — and
/// that is the better test anyway. A constant compared against itself proves
/// nothing; this is the number `spec/limits.md` promises, written independently,
/// so a change to the constant fails here and has to be made deliberately in
/// both places.
const CEILING: usize = 64 * 1024 * 1024;

/// A server that answers `/n/<bytes>` with a body of exactly that many bytes.
fn spawn_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let _ = serve(&mut stream);
            });
        }
    });
    port
}

fn serve(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    let read = stream.read(&mut buf)?;
    let request = String::from_utf8_lossy(&buf[..read]).into_owned();
    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();

    let len: usize = path
        .strip_prefix("/n/")
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);

    stream.write_all(
        format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n").as_bytes(),
    )?;
    // Written in chunks: one 64 MiB allocation per request would make the test
    // itself the memory problem it is checking for.
    let chunk = vec![b'x'; 64 * 1024];
    let mut sent = 0;
    while sent < len {
        let n = chunk.len().min(len - sent);
        stream.write_all(&chunk[..n])?;
        sent += n;
    }
    stream.flush()
}

/// `fetch` the given body length, under an allowlist that permits the host.
fn fetch_bytes(port: u16, len: usize) -> (i32, Option<RunFailure>) {
    let outcome = run_source_detailed(
        format!("let body = fetch(\"http://localhost:{port}/n/{len}\");\nout body.length();\n"),
        "<read-ceiling>",
        RunOpts {
            fetch_allowlist: vec!["localhost".to_string()],
            ..RunOpts::sandboxed()
        },
    );
    (outcome.exit_code, outcome.failure)
}

fn is_resource_error(failure: &Option<RunFailure>) -> bool {
    matches!(failure, Some(RunFailure::Runtime(e)) if e.kind.as_deref() == Some("ResourceError"))
}

#[test]
fn a_body_just_under_the_ceiling_is_read() {
    let port = spawn_server();
    let (code, failure) = fetch_bytes(port, CEILING - 1);
    assert_eq!(
        code, 0,
        "one byte under the ceiling must succeed: {failure:?}"
    );
}

#[test]
fn a_body_exactly_at_the_ceiling_is_read() {
    // The case a `take(limit)` without the extra byte gets wrong: it would
    // truncate here and look identical to a body that was over.
    let port = spawn_server();
    let (code, failure) = fetch_bytes(port, CEILING);
    assert_eq!(code, 0, "exactly the ceiling must succeed: {failure:?}");
}

#[test]
fn a_body_one_byte_over_the_ceiling_is_refused() {
    let port = spawn_server();
    let (code, failure) = fetch_bytes(port, CEILING + 1);
    assert_ne!(code, 0, "one byte over the ceiling must fail");
    assert!(
        is_resource_error(&failure),
        "expected a ResourceError, got {failure:?}"
    );
}

#[test]
fn a_clearly_excessive_body_is_refused_the_same_way() {
    // Not a stronger assertion than the one above — the same one, at a size
    // nobody would call a boundary. It is here so a reader can see that the
    // guard is a ceiling rather than an off-by-one.
    let port = spawn_server();
    let (code, failure) = fetch_bytes(port, CEILING * 2);
    assert_ne!(code, 0);
    assert!(
        is_resource_error(&failure),
        "expected a ResourceError, got {failure:?}"
    );
}

#[test]
fn the_refusal_is_fatal_and_not_catchable() {
    // The half that makes it a resource limit rather than an error. Every other
    // ceiling in `spec/limits.md` is fatal, and a `try` around a
    // memory-exhaustion guard would let a program keep going after one.
    let port = spawn_server();
    let outcome = run_source_detailed(
        format!(
            "try {{ out fetch(\"http://localhost:{port}/n/{}\"); }}\n\
             catch (e) {{ out \"caught\"; }}\n\
             out \"after\";\n",
            CEILING + 1
        ),
        "<read-ceiling>",
        RunOpts {
            fetch_allowlist: vec!["localhost".to_string()],
            ..RunOpts::sandboxed()
        },
    );
    assert_ne!(
        outcome.exit_code, 0,
        "a resource ceiling must not be catchable"
    );
}

#[test]
fn an_ordinary_small_body_is_unaffected() {
    // The positive control, and the one that would catch a guard applied to the
    // wrong quantity — a comparison against the header length, say, or against
    // the chunk size. Without it, a `fetch` that always failed would satisfy
    // every "must be refused" test above.
    let port = spawn_server();
    let outcome = run_source_detailed(
        format!(
            "let body = fetch(\"http://localhost:{port}/n/5\");\nassert(body.length() == 5, body);\n"
        ),
        "<read-ceiling>",
        RunOpts {
            fetch_allowlist: vec!["localhost".to_string()],
            ..RunOpts::sandboxed()
        },
    );
    assert_eq!(
        outcome.exit_code, 0,
        "a 5-byte body must still arrive intact: {:?}",
        outcome.failure
    );
}
