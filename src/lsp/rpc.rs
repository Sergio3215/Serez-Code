// JSON-RPC framing over stdio, as the LSP spec defines it:
// `Content-Length: N\r\n` (+ optional other headers) `\r\n` + N bytes of JSON.
use std::io::{BufRead, Write};

/// Read one framed message. Returns `None` on EOF or malformed headers
/// (the server should exit in both cases).
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
