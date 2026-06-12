// Socket namespace — raw TCP client/server over std::net
// Socket.connect(host, port) → int  (socket id)
// Socket.send(id, data)      → int  (bytes written)
// Socket.recv(id, max_bytes) → string
// Socket.close(id)           → null
// Socket.listen(port)        → int  (listener id)
// Socket.accept(listener_id) → int  (new socket id)

use crate::ast;
use crate::region::ObjectData;
use super::EvalResult;
use std::io::{Read, Write};

macro_rules! require_perm {
    ($self:expr, $ns:expr) => {
        if !$self.permissions.contains($ns) {
            eprintln!(
                "❌ ERROR: '{}' requires permission '{}' — declare it in serez.json \
                 (\"permissions\": [\"{}\", ...]) or with `use permissions {{ {} }}`",
                $ns, $ns, $ns, $ns
            );
            return EvalResult::Error;
        }
    };
}

impl super::Evaluator {
    pub(super) fn eval_socket_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        require_perm!(self, "Socket");
        match dot_call.method.as_str() {
            "connect" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Socket.connect(host, port) requires 2 arguments");
                    return EvalResult::Error;
                }
                let host = match self.eval_to_string(&dot_call.arguments[0], "Socket.connect") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let port_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let port: u16 = match self.resolve(port_ref) {
                    Some(ObjectData::Integer(n)) => *n as u16,
                    _ => {
                        eprintln!("❌ ERROR: Socket.connect: port must be an integer");
                        return EvalResult::Error;
                    }
                };
                let addr = format!("{}:{}", host, port);
                match std::net::TcpStream::connect(&addr) {
                    Ok(stream) => {
                        let id = self.socket_next_id;
                        self.socket_next_id += 1;
                        self.socket_registry.insert(id, stream);
                        EvalResult::Value(self.alloc(ObjectData::Integer(id)))
                    }
                    Err(e) => {
                        eprintln!("❌ ERROR: Socket.connect: {}", e);
                        EvalResult::Error
                    }
                }
            }

            "send" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Socket.send(id, data) requires 2 arguments");
                    return EvalResult::Error;
                }
                let id = match self.eval_socket_id(&dot_call.arguments[0], "Socket.send") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let data = match self.eval_to_string(&dot_call.arguments[1], "Socket.send") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let bytes = data.into_bytes();
                let result: Result<usize, std::io::Error> =
                    match self.socket_registry.get_mut(&id) {
                        Some(stream) => stream.write_all(&bytes).map(|_| bytes.len()),
                        None => {
                            eprintln!("❌ ERROR: Socket.send: no socket with id {}", id);
                            return EvalResult::Error;
                        }
                    };
                match result {
                    Ok(n) => EvalResult::Value(self.alloc(ObjectData::Integer(n as i64))),
                    Err(e) => {
                        eprintln!("❌ ERROR: Socket.send: {}", e);
                        EvalResult::Error
                    }
                }
            }

            "recv" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Socket.recv(id, max_bytes) requires 2 arguments");
                    return EvalResult::Error;
                }
                let id = match self.eval_socket_id(&dot_call.arguments[0], "Socket.recv") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let max_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let max_bytes: usize = match self.resolve(max_ref) {
                    Some(ObjectData::Integer(n)) => (*n).max(0) as usize,
                    _ => {
                        eprintln!("❌ ERROR: Socket.recv: max_bytes must be an integer");
                        return EvalResult::Error;
                    }
                };
                let result: Result<String, std::io::Error> =
                    match self.socket_registry.get_mut(&id) {
                        Some(stream) => {
                            let mut buf = vec![0u8; max_bytes];
                            stream.read(&mut buf).map(|n| {
                                String::from_utf8_lossy(&buf[..n]).into_owned()
                            })
                        }
                        None => {
                            eprintln!("❌ ERROR: Socket.recv: no socket with id {}", id);
                            return EvalResult::Error;
                        }
                    };
                match result {
                    Ok(s) => EvalResult::Value(self.alloc(ObjectData::Str(s))),
                    Err(e) => {
                        eprintln!("❌ ERROR: Socket.recv: {}", e);
                        EvalResult::Error
                    }
                }
            }

            "close" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Socket.close(id) requires 1 argument");
                    return EvalResult::Error;
                }
                let id = match self.eval_socket_id(&dot_call.arguments[0], "Socket.close") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                // Remove from both registries — IDs are shared across socket/listener space
                self.socket_registry.remove(&id);
                self.listener_registry.remove(&id);
                EvalResult::Value(self.null_ref)
            }

            "listen" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Socket.listen(port) requires 1 argument");
                    return EvalResult::Error;
                }
                let port_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let port: u16 = match self.resolve(port_ref) {
                    Some(ObjectData::Integer(n)) => *n as u16,
                    _ => {
                        eprintln!("❌ ERROR: Socket.listen: port must be an integer");
                        return EvalResult::Error;
                    }
                };
                let addr = format!("0.0.0.0:{}", port);
                match std::net::TcpListener::bind(&addr) {
                    Ok(listener) => {
                        let id = self.socket_next_id;
                        self.socket_next_id += 1;
                        self.listener_registry.insert(id, listener);
                        EvalResult::Value(self.alloc(ObjectData::Integer(id)))
                    }
                    Err(e) => {
                        eprintln!("❌ ERROR: Socket.listen: {}", e);
                        EvalResult::Error
                    }
                }
            }

            "accept" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Socket.accept(listener_id) requires 1 argument");
                    return EvalResult::Error;
                }
                let id = match self.eval_socket_id(&dot_call.arguments[0], "Socket.accept") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                // accept() takes &self so we can get an immutable borrow, complete the call,
                // then drop the borrow before mutating socket_registry.
                let accept_result: Result<(std::net::TcpStream, _), _> =
                    match self.listener_registry.get(&id) {
                        Some(listener) => listener.accept(),
                        None => {
                            eprintln!(
                                "❌ ERROR: Socket.accept: no listener with id {}",
                                id
                            );
                            return EvalResult::Error;
                        }
                    };
                match accept_result {
                    Ok((stream, _addr)) => {
                        let new_id = self.socket_next_id;
                        self.socket_next_id += 1;
                        self.socket_registry.insert(new_id, stream);
                        EvalResult::Value(self.alloc(ObjectData::Integer(new_id)))
                    }
                    Err(e) => {
                        eprintln!("❌ ERROR: Socket.accept: {}", e);
                        EvalResult::Error
                    }
                }
            }

            "recvWsFrame" => {
                // Socket.recvWsFrame(conn_id) → string | null
                // Reads one WebSocket frame from an established connection.
                // Returns the decoded text payload, or null on close frame / connection end.
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Socket.recvWsFrame(conn_id) requires 1 argument");
                    return EvalResult::Error;
                }
                let id = match self.eval_socket_id(&dot_call.arguments[0], "Socket.recvWsFrame") {
                    Ok(v) => v, Err(e) => return e,
                };
                match self.socket_registry.get_mut(&id) {
                    Some(stream) => match ws_recv_frame(stream) {
                        Ok(Some(msg)) => EvalResult::Value(self.alloc(ObjectData::Str(msg))),
                        Ok(None)      => EvalResult::Value(self.null_ref),
                        Err(e) => {
                            eprintln!("❌ ERROR: Socket.recvWsFrame: {}", e);
                            EvalResult::Value(self.null_ref)
                        }
                    },
                    None => {
                        eprintln!("❌ ERROR: Socket.recvWsFrame: no socket with id {}", id);
                        EvalResult::Error
                    }
                }
            }

            "sendWsFrame" => {
                // Socket.sendWsFrame(conn_id, data) → null
                // Encodes data as a WebSocket text frame and sends it.
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Socket.sendWsFrame(conn_id, data) requires 2 arguments");
                    return EvalResult::Error;
                }
                let id = match self.eval_socket_id(&dot_call.arguments[0], "Socket.sendWsFrame") {
                    Ok(v) => v, Err(e) => return e,
                };
                let data = match self.eval_to_string(&dot_call.arguments[1], "Socket.sendWsFrame") {
                    Ok(v) => v, Err(e) => return e,
                };
                match self.socket_registry.get_mut(&id) {
                    Some(stream) => match ws_send_frame(stream, &data) {
                        Ok(()) => EvalResult::Value(self.null_ref),
                        Err(e) => {
                            eprintln!("❌ ERROR: Socket.sendWsFrame: {}", e);
                            EvalResult::Error
                        }
                    },
                    None => {
                        eprintln!("❌ ERROR: Socket.sendWsFrame: no socket with id {}", id);
                        EvalResult::Error
                    }
                }
            }

            _ => {
                eprintln!("❌ ERROR: Unknown Socket method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    fn eval_socket_id(
        &mut self,
        expr: &ast::Expression,
        ctx: &str,
    ) -> Result<i64, EvalResult> {
        let r = match self.eval_expression(expr) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Integer(n)) => Ok(*n),
            _ => {
                eprintln!("❌ ERROR: {}: socket id must be an integer", ctx);
                Err(EvalResult::Error)
            }
        }
    }
}

// ── WebSocket frame helpers (RFC 6455) ────────────────────────────────────────

// Maximum accepted payload per frame. Rejects DoS attempts that claim huge sizes.
const WS_MAX_PAYLOAD: usize = 16 * 1024 * 1024; // 16 MiB

fn ws_recv_frame(stream: &mut std::net::TcpStream) -> Result<Option<String>, std::io::Error> {
    use std::io::{Read, Write};

    loop {
        let mut header = [0u8; 2];
        stream.read_exact(&mut header)?;

        // BUG-FIX: validate RSV bits — must be zero unless an extension is negotiated.
        // A non-zero RSV without a negotiated extension is a protocol error (RFC 6455 §5.2).
        let rsv = header[0] & 0x70;
        if rsv != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WS: non-zero RSV bits without negotiated extension",
            ));
        }

        let opcode          = header[0] & 0x0F;
        let masked          = (header[1] & 0x80) != 0;
        let payload_len_raw = (header[1] & 0x7F) as usize;

        // Parse extended payload length
        let payload_len = if payload_len_raw == 126 {
            let mut buf = [0u8; 2];
            stream.read_exact(&mut buf)?;
            u16::from_be_bytes(buf) as usize
        } else if payload_len_raw == 127 {
            let mut buf = [0u8; 8];
            stream.read_exact(&mut buf)?;
            let n = u64::from_be_bytes(buf);
            // BUG-FIX: cap before casting to prevent usize overflow and DoS allocation.
            if n > WS_MAX_PAYLOAD as u64 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WS: payload exceeds maximum allowed size (16 MiB)",
                ));
            }
            n as usize
        } else {
            payload_len_raw
        };

        // BUG-FIX: guard against DoS via 1-byte extended-length path claiming huge size.
        if payload_len > WS_MAX_PAYLOAD {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WS: payload exceeds maximum allowed size (16 MiB)",
            ));
        }

        // BUG-FIX: control frames (close/ping/pong) MUST have payload ≤ 125 bytes (RFC 6455 §5.5).
        if opcode >= 8 && payload_len > 125 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WS: control frame payload exceeds 125 bytes",
            ));
        }

        // Read masking key if present
        let mask = if masked {
            let mut buf = [0u8; 4];
            stream.read_exact(&mut buf)?;
            buf
        } else {
            [0u8; 4]
        };

        // Read payload
        let mut payload = vec![0u8; payload_len];
        stream.read_exact(&mut payload)?;

        if masked {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[i % 4];
            }
        }

        match opcode {
            // BUG-FIX: close frame — payload is now fully consumed before returning None,
            // preventing stream desync when the peer includes a close code + reason.
            8 => return Ok(None),

            // BUG-FIX: ping — must reply with pong carrying the same payload (RFC 6455 §5.5.2).
            // Use a loop instead of recursion to avoid stack overflow on repeated pings.
            9 => {
                let mut pong = Vec::with_capacity(2 + payload.len());
                pong.push(0x8Au8);              // FIN=1, opcode=10 (pong)
                pong.push(payload.len() as u8); // length ≤ 125 — safe after the check above
                pong.extend_from_slice(&payload);
                stream.write_all(&pong)?;
                stream.flush()?;
                continue; // read the next actual data frame
            }

            // Pong — unsolicited, RFC 6455 §5.5.3 allows ignoring it.
            10 => continue,

            // Text frame (1)
            1 => {
                // BUG-FIX: RFC 6455 §5.7 — text frames MUST be valid UTF-8.
                // Return an error instead of silently replacing invalid bytes with U+FFFD.
                match String::from_utf8(payload) {
                    Ok(s)  => return Ok(Some(s)),
                    Err(_) => return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WS: text frame contains invalid UTF-8",
                    )),
                }
            }

            // Binary (2), continuation (0), or unknown — return as lossy string
            _ => return Ok(Some(String::from_utf8_lossy(&payload).into_owned())),
        }
    }
}

fn ws_send_frame(stream: &mut std::net::TcpStream, data: &str) -> Result<(), std::io::Error> {
    use std::io::Write;

    let payload = data.as_bytes();
    let len     = payload.len();
    let mut frame = Vec::with_capacity(len + 10);

    // FIN=1, opcode=1 (text frame).
    // Server → client frames are NOT masked per RFC 6455 §5.1.
    frame.push(0x81u8);

    if len < 126 {
        frame.push(len as u8);
    } else if len < 65536 {
        frame.push(126u8);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127u8);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod ws_frame_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    fn loopback(port: u16) -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        let client   = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let (server, _) = listener.accept().unwrap();
        (client, server)
    }

    // Craft a raw unmasked WS frame: [0x81, len, payload...]
    fn make_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut f = vec![0x80 | opcode];
        f.push(payload.len() as u8);
        f.extend_from_slice(payload);
        f
    }

    #[test]
    fn text_frame_roundtrip() {
        let (mut cli, mut srv) = loopback(29900);
        cli.write_all(&make_frame(1, b"hello")).unwrap();
        assert_eq!(ws_recv_frame(&mut srv).unwrap(), Some("hello".to_string()));
    }

    #[test]
    fn ping_triggers_pong_and_returns_next_data() {
        let (mut cli, mut srv) = loopback(29901);
        // send ping then text
        cli.write_all(&make_frame(9, b"keepalive")).unwrap();
        cli.write_all(&make_frame(1, b"data")).unwrap();
        // server should send pong and return "data"
        let msg = ws_recv_frame(&mut srv).unwrap();
        assert_eq!(msg, Some("data".to_string()));
        // verify pong was sent back to client
        let mut pong_buf = [0u8; 11]; // 2 header + 9 payload
        cli.read_exact(&mut pong_buf).unwrap();
        assert_eq!(pong_buf[0], 0x8A, "pong opcode");
        assert_eq!(&pong_buf[2..], b"keepalive");
    }

    #[test]
    fn close_frame_returns_none_and_consumes_payload() {
        let (mut cli, mut srv) = loopback(29902);
        // close frame with 2-byte close code
        cli.write_all(&make_frame(8, &[0x03, 0xE8])).unwrap();
        // send another frame after close
        cli.write_all(&make_frame(1, b"after")).unwrap();
        assert_eq!(ws_recv_frame(&mut srv).unwrap(), None);
        // second recvWsFrame should read "after" cleanly (stream not desynchronized)
        assert_eq!(ws_recv_frame(&mut srv).unwrap(), Some("after".to_string()));
    }

    #[test]
    fn oversized_payload_claim_rejected() {
        let (mut cli, mut srv) = loopback(29903);
        // frame claiming 2^32 bytes via 127 extended length
        let mut evil = vec![0x81u8, 127u8];
        evil.extend_from_slice(&(0x0000000100000000u64).to_be_bytes());
        cli.write_all(&evil).unwrap();
        assert!(ws_recv_frame(&mut srv).is_err());
    }

    #[test]
    fn nonzero_rsv_rejected() {
        let (mut cli, mut srv) = loopback(29904);
        // FIN=1, RSV1=1, opcode=1 → first byte = 0xC1
        let frame = vec![0xC1u8, 0x05, b'h', b'e', b'l', b'l', b'o'];
        cli.write_all(&frame).unwrap();
        assert!(ws_recv_frame(&mut srv).is_err());
    }

    #[test]
    fn invalid_utf8_text_frame_rejected() {
        let (mut cli, mut srv) = loopback(29905);
        // 0xFF is not valid UTF-8
        cli.write_all(&make_frame(1, &[0xFF, 0xFE])).unwrap();
        assert!(ws_recv_frame(&mut srv).is_err());
    }

    #[test]
    fn control_frame_exceeding_125_bytes_rejected() {
        let (mut cli, mut srv) = loopback(29906);
        // ping with 126-byte payload (must be rejected — control frames ≤ 125)
        let payload = vec![b'x'; 126];
        let mut frame = vec![0x89u8, 126u8]; // ping, extended-2-byte length marker
        frame.extend_from_slice(&(126u16).to_be_bytes());
        frame.extend_from_slice(&payload);
        cli.write_all(&frame).unwrap();
        assert!(ws_recv_frame(&mut srv).is_err());
    }

    #[test]
    fn multiple_pings_before_data() {
        let (mut cli, mut srv) = loopback(29907);
        for _ in 0..5 {
            cli.write_all(&make_frame(9, b"ping")).unwrap();
        }
        cli.write_all(&make_frame(1, b"final")).unwrap();
        let msg = ws_recv_frame(&mut srv).unwrap();
        assert_eq!(msg, Some("final".to_string()));
    }
}
