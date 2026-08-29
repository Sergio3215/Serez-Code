// JSON-RPC framing over stdio, as the LSP spec defines it:
// `Content-Length: N\r\n` (+ optional other headers) `\r\n` + N bytes of JSON.
use std::io::{BufRead, Write};

/// The largest message body this server will allocate for.
///
/// `Content-Length` arrives from whatever is writing to stdin, and the body
/// used to be allocated at exactly that size with no ceiling: a header reading
/// `Content-Length: 9999999999999` made the process abort with
/// `memory allocation of 9999999999999 bytes failed` — an allocator message,
/// not a diagnostic, and the editor's language server simply disappears.
///
/// Every other input-sized allocation in this project is bounded and the bound
/// is written down in `spec/limits.md`: `File.read` at 256 MiB, a package
/// archive at 64 MiB, a task worker's source at 16 MiB, a WebSocket frame at
/// 16 MiB. This one was the exception.
///
/// 64 MiB is deliberately generous. The largest legitimate message is a
/// `didOpen`/`didChange` carrying a whole document, and a source file anywhere
/// near this size is far past the `MAX_PARSE_DEPTH` ceiling already.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

/// Read one framed message. Returns `None` on EOF, malformed headers, or a
/// `Content-Length` above [`MAX_MESSAGE_BYTES`] (the server should exit in all
/// three cases).
pub fn read_message(input: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None; // EOF
        }
        let line = line.trim_end();
        if line.is_empty() {
            break; // end of headers
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
        // Content-Type is ignored (always utf-8 in practice).
    }
    let len = content_length?;
    if len > MAX_MESSAGE_BYTES {
        // Say why. A silent exit here looks identical to the editor closing the
        // pipe, which is the wrong thing to go looking for.
        eprintln!(
            "sz-lsp: refusing a {len}-byte message; the ceiling is {MAX_MESSAGE_BYTES} bytes"
        );
        return None;
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body).ok()?;
    Some(body)
}

/// Write one framed message and flush.
pub fn write_message(output: &mut impl Write, message: &serde_json::Value) {
    let body = message.to_string();
    let _ = write!(output, "Content-Length: {}\r\n\r\n", body.len());
    let _ = output.write_all(body.as_bytes());
    let _ = output.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn framed(body: &str) -> Vec<u8> {
        format!(
            "Content-Length: {}

{body}",
            body.len()
        )
        .into_bytes()
    }

    #[test]
    fn a_well_formed_message_round_trips() {
        let mut input = Cursor::new(framed(r#"{"jsonrpc":"2.0"}"#));
        let body = read_message(&mut input).expect("a framed message must read");
        assert_eq!(String::from_utf8(body).unwrap(), r#"{"jsonrpc":"2.0"}"#);
    }

    #[test]
    fn a_content_length_above_the_ceiling_is_refused_not_allocated() {
        // This used to abort the process: `vec![0u8; len]` with an unbounded
        // `len` from the header produced
        // `memory allocation of 9999999999999 bytes failed`, an allocator
        // message rather than a diagnostic, and the editor's language server
        // vanished. Reaching this assertion at all is the test.
        let header = format!(
            "Content-Length: {}

",
            u64::MAX
        );
        let mut input = Cursor::new(header.into_bytes());
        assert!(read_message(&mut input).is_none());

        let just_over = format!(
            "Content-Length: {}

",
            MAX_MESSAGE_BYTES + 1
        );
        let mut input = Cursor::new(just_over.into_bytes());
        assert!(read_message(&mut input).is_none());
    }

    #[test]
    fn eof_and_malformed_headers_are_none_not_a_panic() {
        assert!(read_message(&mut Cursor::new(Vec::new())).is_none());
        assert!(
            read_message(&mut Cursor::new(
                b"
"
                .to_vec()
            ))
            .is_none()
        );
        assert!(
            read_message(&mut Cursor::new(
                b"Content-Length: abc

"
                .to_vec()
            ))
            .is_none()
        );
        assert!(
            read_message(&mut Cursor::new(
                b"Content-Length: -1

"
                .to_vec()
            ))
            .is_none()
        );
        // A body shorter than the header promises is a truncated stream.
        assert!(
            read_message(&mut Cursor::new(
                b"Content-Length: 100

short"
                    .to_vec()
            ))
            .is_none()
        );
    }
}
