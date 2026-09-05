//! Dot access on structured data that came back from `fetch`.
//!
//! # Why this file contains no runtime code of its own
//!
//! The question this answers is whether `response.name` needs anything
//! fetch-specific. It does not, and the reason is worth stating because the
//! tempting shape — `if came_from_fetch { … }` — would have been wrong:
//!
//! `fetch` has no structured-data type. Measured against the release binary:
//! the default call returns the **body as a string**, and `{ full: true }`
//! returns a `Dict<string, any>` of `{ status, ok, statusText, headers, body }`.
//! A program that wants fields out of a JSON body calls `JSON.parse`, which
//! builds an ordinary `ObjectData::Dict` — the same one a dict literal builds.
//!
//! So there is exactly one runtime path, and the dictionary commit already
//! changed it. These tests exist to *prove* that claim on real HTTP responses
//! rather than to assert it in a comment, and they would fail if anyone later
//! special-cased fetch into a second kind of value.
//!
//! # Why a local server
//!
//! `tests/42_fetch_e2e.sz` and `tests/43_fetch_full_e2e.sz` reach the public
//! internet and are deliberately network-tolerant: they pass when the network
//! is down. That is right for an end-to-end smoke test and useless for pinning
//! semantics — a test that passes without running is not a gate. Everything
//! here answers from a `TcpListener` on a port the OS picks, in the same shape
//! `tests/lockdown_fetch.rs` uses, so a failure means the language changed and
//! never means the internet did.

use serez_code::run::{RunFailure, RunOpts, run_source_detailed};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

// ── A server that answers with the shapes a real API answers with ────────────

/// The JSON every test below reads. Nested object, array, string, int, bool and
/// null, so one response covers every value type dot access has to carry —
/// plus `keys` and `length`, which are the names of dict methods and are here
/// because a real API is under no obligation to avoid them.
const PAYLOAD: &str = r#"{"name":"Sergio","age":28,"active":true,"nickname":null,"address":{"city":"Rosario","zip":"S2000"},"tags":["owner","admin"],"keys":"a field called keys","length":"a field called length"}"#;

fn spawn_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = handle(&mut stream);
        }
    });
    port
}

fn handle(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 2048];
    let read = stream.read(&mut buf)?;
    let request = String::from_utf8_lossy(&buf[..read]).into_owned();
    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();

    let response = match path.as_str() {
        "/json" => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Probe: seen\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{PAYLOAD}",
            PAYLOAD.len()
        ),
        _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
    };
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

/// Run a Serez program with `PORT` replaced by the live port.
///
/// The program asserts for itself and a non-zero exit is the failure, which
/// keeps the JSON out of the Serez source: the payload arrives over the socket,
/// so no test here has to escape `{` for string interpolation.
fn run(program: &str, port: u16) -> (i32, Option<RunFailure>) {
    let source = program.replace("PORT", &port.to_string());
    let outcome = run_source_detailed(source, "<fetch-dot-access>", RunOpts::default());
    (outcome.exit_code, outcome.failure)
}

fn assert_passes(program: &str, port: u16) {
    let (code, failure) = run(program, port);
    assert_eq!(code, 0, "the program should have passed: {failure:?}");
}

// ── 0. The harness can fail ──────────────────────────────────────────────────

#[test]
fn the_harness_reports_a_failing_assertion_rather_than_passing_quietly() {
    // Every test below passes, which is only meaningful if passing is
    // something this harness can not do by accident — a server that never
    // answered, or a program that never ran, must not read as success.
    let port = spawn_server();
    let (code, _) = run(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(data.name == "nobody", "deliberately wrong");
        "#,
        port,
    );
    assert_ne!(code, 0, "a false assertion must fail the program");

    // And an unreachable port must fail too, so "the server answered" is
    // load-bearing rather than incidental.
    let (dead, _) = run(
        r#"
        let body = fetch("http://127.0.0.1:PORT/json");
        assert(body.includes("Sergio"), "unreachable");
        "#,
        1,
    );
    assert_ne!(dead, 0, "a request to a closed port must fail the program");
}

// ── 1. The network contract is unchanged ─────────────────────────────────────

#[test]
fn a_plain_fetch_still_returns_the_body_as_a_string() {
    // The starting point, pinned so the rest of this file cannot be read as a
    // claim that fetch grew a structured return type. It did not.
    let port = spawn_server();
    assert_passes(
        r#"
        let body = fetch("http://127.0.0.1:PORT/json");
        assert(type_of(body) == "string", "fetch returns a string body");
        assert(body.includes("Sergio"), "and it is the payload");
        "#,
        port,
    );
}

// ── 2-3. Parsing it produces an ordinary dict, read the way dicts are read ───

#[test]
fn the_parsed_body_is_a_dict_and_brackets_read_it() {
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(type_of(data) == "dict", "JSON.parse builds a dict, not a fetch type");
        assert(data["name"] == "Sergio", "brackets read the key");
        assert(data["age"] == 28, "including an int");
        "#,
        port,
    );
}

// ── 4-5. Dot reads the same key, and the two forms agree ─────────────────────

#[test]
fn dot_access_reads_what_brackets_read_on_a_fetched_body() {
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(data.name == "Sergio", "dot reads the key");
        assert(data.name == data["name"], "and agrees with brackets");
        assert(data.age == data["age"], "int agrees");
        assert(data.active == data["active"], "bool agrees");
        assert(data.active == true, "and is the right bool");
        assert(data.nickname == null, "a JSON null reads as null");
        assert(data.nickname == data["nickname"], "null agrees");
        "#,
        port,
    );
}

// ── 6-7. Nested objects, and the two forms mixed in one expression ───────────

#[test]
fn a_nested_object_is_reachable_through_dots() {
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(type_of(data.address) == "dict", "a nested object is a dict too");
        assert(data.address.city == "Rosario", "a chain of dots");
        assert(data.address.city == data["address"]["city"], "agrees with brackets");
        "#,
        port,
    );
}

#[test]
fn the_two_forms_can_be_mixed_in_one_expression() {
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(data["address"].city == "Rosario", "brackets then dot");
        assert(data.address["city"] == "Rosario", "dot then brackets");
        assert(data["address"].city == data.address["city"], "both mixings agree");
        assert(data.address.zip == "S2000", "a sibling key of the nested object");
        "#,
        port,
    );
}

// ── 8. Arrays inside the response ────────────────────────────────────────────

#[test]
fn an_array_value_is_indexed_after_a_dot_access() {
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(type_of(data.tags) == "array", "a JSON array stays an array");
        assert(data.tags[0] == "owner", "dot access then index");
        assert(data.tags[1] == data["tags"][1], "agrees with brackets");
        assert(data.tags.length() == 2, "and the array methods still work");
        "#,
        port,
    );
}

// ── 9. A key the response did not carry ──────────────────────────────────────

#[test]
fn a_field_the_response_does_not_carry_follows_the_dict_policy() {
    // Not a fetch question at all: the dict answers `null` for a key it does not
    // hold, and a parsed response is a dict. Asserted against the bracket form
    // so the two cannot be given different answers later.
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(data.missing == null, "an absent key reads as null");
        assert(data.missing == data["missing"], "which is what brackets answer");
        assert(data.address.missing == null, "and nested, the same");
        "#,
        port,
    );
}

// ── 10. Full mode — the one place fetch itself hands back a dict ─────────────

#[test]
fn full_mode_metadata_is_reachable_through_dots() {
    // `{ full: true }` is the only fetch result that is structured, and it is a
    // plain `Dict<string, any>`. None of its keys — status, ok, statusText,
    // headers, body — collide with a dict method name, so all five read through
    // dot access.
    let port = spawn_server();
    assert_passes(
        r#"
        let opts <string, any> = ({"full", true});
        let r = fetch("http://127.0.0.1:PORT/json", opts);
        assert(type_of(r) == "dict", "full mode returns a dict");
        assert(r.status == 200, "status through dot");
        assert(r.ok == true, "ok through dot");
        assert(r.statusText != null, "statusText through dot");
        assert(r.status == r["status"], "status agrees with brackets");
        assert(r.ok == r["ok"], "ok agrees with brackets");
        assert(r.body == r["body"], "body agrees with brackets");
        assert(r.body.includes("Sergio"), "and the body is the payload");
        "#,
        port,
    );
}

#[test]
fn the_headers_dict_inside_a_full_response_reads_the_same_way() {
    let port = spawn_server();
    assert_passes(
        r#"
        let opts <string, any> = ({"full", true});
        let r = fetch("http://127.0.0.1:PORT/json", opts);
        assert(type_of(r.headers) == "dict", "headers is a nested dict");
        assert(r["headers"]["x-probe"] == "seen", "the server's header, via brackets");
        assert(r.headers["x-probe"] == "seen", "dot then brackets");
        "#,
        port,
    );
}

// ── DEC-M12-001 on real response data ────────────────────────────────────────

#[test]
fn a_response_field_named_after_a_method_is_read_as_a_field() {
    // The case the decision exists for. A server is free to return a field
    // called "keys", and a client that reads `response.keys` means the field.
    // Under the previous precedence it got the method's key list instead —
    // silently, and only for the field names that happened to collide.
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(data.keys == "a field called keys", "the field, not the method");
        assert(data.keys == data["keys"], "the two forms agree on it");
        assert(data.length == "a field called length", "and the same for length");
        assert(data.length == data["length"], "which brackets also read");
        "#,
        port,
    );
}

#[test]
fn the_call_form_still_reaches_the_method_on_that_response() {
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        assert(data.length() == 8, "length() counts the fields");
        assert(data.keys().length() == 8, "keys() lists them");
        assert(data.keys().includes("keys"), "including the colliding one");
        assert(data.keys == "a field called keys", "and the field is still readable");
        "#,
        port,
    );
}

// ── DEC-M12-002 on real response data ────────────────────────────────────────

#[test]
fn a_parsed_response_is_written_the_way_any_dict_is() {
    // No provenance rule: the value is a dict held by a `let`, so the ordinary
    // dict write applies through both spellings.
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        data.name = "Jonathan";
        assert(data.name == "Jonathan", "the dot write took");
        assert(data["name"] == "Jonathan", "and brackets see it");

        data["age"] = 29;
        assert(data.age == 29, "and the other way round");

        data.newField = "added";
        assert(data["newField"] == "added", "a new key is created as brackets create it");

        data.address.city = "Cordoba";
        assert(data["address"]["city"] == "Cordoba", "nested, through the dotted path");
        "#,
        port,
    );
}

#[test]
fn writing_a_response_field_named_after_a_method_leaves_the_method_alone() {
    let port = spawn_server();
    assert_passes(
        r#"
        let data = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        data.keys = "overwritten";
        assert(data.keys == "overwritten", "the field was written");
        assert(data.keys().length() == 8, "and keys() still works");
        assert(data.length() == 8, "as does length()");
        "#,
        port,
    );
}

// ── The control: one path, not two ───────────────────────────────────────────

#[test]
fn a_fetched_dict_and_a_literal_dict_behave_identically() {
    // This is the test that would fail if anyone made fetch's data special. The
    // same key is read out of a hand-written dict and out of the parsed
    // response, through both forms, and every answer has to match — including
    // the missing-key answer, which is where a bespoke fetch policy would most
    // plausibly diverge.
    let port = spawn_server();
    assert_passes(
        r#"
        let fetched = JSON.parse(fetch("http://127.0.0.1:PORT/json"));
        let literal <string, any> = ({"name", "Sergio"}, {"age", 28});

        assert(fetched.name == literal.name, "dot access agrees across origins");
        assert(fetched["name"] == literal["name"], "so does bracket access");
        assert(fetched.age == literal.age, "and for a non-string value");
        assert(fetched.missing == literal.missing, "and for a key neither holds");
        assert(type_of(fetched) == type_of(literal), "they are the same type");

        // The dict methods answer on a fetched dict exactly as on any other.
        assert(fetched.keys().length() == 8, "keys() over the response");
        assert(fetched.length() == 8, "length() over the response");
        "#,
        port,
    );
}
