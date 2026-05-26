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

impl super::Evaluator {
    pub(super) fn eval_socket_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
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
                let addr = format!("127.0.0.1:{}", port);
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
