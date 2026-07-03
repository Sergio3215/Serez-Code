// Binary namespace — byte-array utilities for binary data manipulation
//
// All operations work on Serez integer arrays (values 0-255 = bytes).
//
// Binary.fromHex(hex)          → [int]   decode hex string to byte array
// Binary.toHex(bytes)          → string  encode byte array to lowercase hex
// Binary.fromUtf8(s)           → [int]   UTF-8 bytes of string
// Binary.toUtf8(bytes)         → string  decode UTF-8 byte array to string
// Binary.packInt32Le(n)        → [int]   4-byte LE encoding
// Binary.packInt32Be(n)        → [int]   4-byte BE encoding
// Binary.unpackInt32Le(bytes)  → int
// Binary.unpackInt32Be(bytes)  → int
// Binary.packInt64Le(n)        → [int]   8-byte LE encoding
// Binary.unpackInt64Le(bytes)  → int
// Binary.concat(a, b)          → [int]   concatenate two byte arrays

use crate::ast;
use crate::region::{ObjectData, ObjectRef, OwnedValue};
use super::EvalResult;

impl super::Evaluator {
    pub(super) fn eval_binary_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "fromHex" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Binary.fromHex(hex) requires 1 argument");
                }
                let hex = match self.eval_to_string(&dot_call.arguments[0], "Binary.fromHex") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if hex.len() % 2 != 0 {
                    return self.rt_err_kind("BinaryError", "Binary.fromHex: hex string must have even length");
                }
                let mut bytes: Vec<OwnedValue> = Vec::with_capacity(hex.len() / 2);
                for i in (0..hex.len()).step_by(2) {
                    match u8::from_str_radix(&hex[i..i + 2], 16) {
                        Ok(b) => {
                            bytes.push(OwnedValue::Integer(b as i64));
                        }
                        Err(_) => {
                            return self.rt_err_kind("BinaryError", format!("Binary.fromHex: invalid hex pair '{}'", &hex[i..i + 2]));
                        }
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: bytes,
                }))
            }

            "toHex" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Binary.toHex(bytes) requires 1 argument");
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let elems = match self.resolve(arr_ref) {
                    Some(ObjectData::Array { elements, .. }) => elements.clone(),
                    _ => {
                        return self.rt_err_kind("TypeError", "Binary.toHex: argument must be an array");
                    }
                };
                let mut hex = String::with_capacity(elems.len() * 2);
                for r in elems {
                    match r {
                        OwnedValue::Integer(b) => {
                            hex.push_str(&format!("{:02x}", (b as u8)));
                        }
                        _ => {
                            return self.rt_err_kind("TypeError", "Binary.toHex: all elements must be integers");
                        }
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Str(hex)))
            }

            "fromUtf8" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Binary.fromUtf8(s) requires 1 argument");
                }
                let s = match self.eval_to_string(&dot_call.arguments[0], "Binary.fromUtf8") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let owned: Vec<OwnedValue> = s
                    .as_bytes()
                    .iter()
                    .map(|&b| OwnedValue::Integer(b as i64))
                    .collect();
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: owned,
                }))
            }

            "toUtf8" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Binary.toUtf8(bytes) requires 1 argument");
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let elems = match self.resolve(arr_ref) {
                    Some(ObjectData::Array { elements, .. }) => elements.clone(),
                    _ => {
                        return self.rt_err_kind("TypeError", "Binary.toUtf8: argument must be an array");
                    }
                };
                let bytes: Result<Vec<u8>, _> = elems
                    .iter()
                    .map(|r| match r {
                        OwnedValue::Integer(b) => Ok(*b as u8),
                        _ => Err(()),
                    })
                    .collect();
                match bytes {
                    Ok(bs) => {
                        let s = String::from_utf8_lossy(&bs).into_owned();
                        EvalResult::Value(self.alloc(ObjectData::Str(s)))
                    }
                    Err(_) => {
                        self.rt_err_kind("TypeError", "Binary.toUtf8: all elements must be integers")
                    }
                }
            }

            "packInt32Le" => {
                let n = match self.require_one_int(&dot_call.arguments, "Binary.packInt32Le") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let b = (n as u32).to_le_bytes();
                self.alloc_byte_array(&b)
            }

            "packInt32Be" => {
                let n = match self.require_one_int(&dot_call.arguments, "Binary.packInt32Be") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let b = (n as u32).to_be_bytes();
                self.alloc_byte_array(&b)
            }

            "unpackInt32Le" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Binary.unpackInt32Le(bytes) requires 1 argument");
                }
                let bytes = match self.eval_to_bytes(&dot_call.arguments[0], "Binary.unpackInt32Le") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if bytes.len() < 4 {
                    return self.rt_err_kind("BinaryError", "Binary.unpackInt32Le: need at least 4 bytes");
                }
                let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i64;
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            "unpackInt32Be" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Binary.unpackInt32Be(bytes) requires 1 argument");
                }
                let bytes = match self.eval_to_bytes(&dot_call.arguments[0], "Binary.unpackInt32Be") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if bytes.len() < 4 {
                    return self.rt_err_kind("BinaryError", "Binary.unpackInt32Be: need at least 4 bytes");
                }
                let n = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as i64;
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            "packInt64Le" => {
                let n = match self.require_one_int(&dot_call.arguments, "Binary.packInt64Le") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let b = n.to_le_bytes();
                self.alloc_byte_array(&b)
            }

            "unpackInt64Le" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Binary.unpackInt64Le(bytes) requires 1 argument");
                }
                let bytes = match self.eval_to_bytes(&dot_call.arguments[0], "Binary.unpackInt64Le") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if bytes.len() < 8 {
                    return self.rt_err_kind("BinaryError", "Binary.unpackInt64Le: need at least 8 bytes");
                }
                let arr: [u8; 8] = bytes[..8].try_into().unwrap();
                let n = i64::from_le_bytes(arr);
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            "concat" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Binary.concat(a, b) requires 2 arguments");
                }
                let a_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let b_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let a_elems = match self.resolve(a_ref) {
                    Some(ObjectData::Array { elements, .. }) => elements.clone(),
                    _ => {
                        return self.rt_err_kind("TypeError", "Binary.concat: first argument must be an array");
                    }
                };
                let b_elems = match self.resolve(b_ref) {
                    Some(ObjectData::Array { elements, .. }) => elements.clone(),
                    _ => {
                        return self.rt_err_kind("TypeError", "Binary.concat: second argument must be an array");
                    }
                };
                let mut combined = a_elems;
                combined.extend(b_elems);
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: combined,
                }))
            }

            _ => {
                self.rt_err_kind("TypeError", format!("Unknown Binary method '{}'", dot_call.method))
            }
        }
    }

    // ── Binary helpers ────────────────────────────────────────────────────────

    fn alloc_byte_array(&mut self, bytes: &[u8]) -> EvalResult {
        let owned: Vec<OwnedValue> = bytes
            .iter()
            .map(|&b| OwnedValue::Integer(b as i64))
            .collect();
        EvalResult::Value(self.alloc(ObjectData::Array {
            element_type: Some("int".to_string()),
            elements: owned,
        }))
    }

    fn require_one_int(
        &mut self,
        args: &[ast::Expression],
        ctx: &str,
    ) -> Result<i64, EvalResult> {
        if args.len() != 1 {
            eprintln!("❌ ERROR: {}(n) requires 1 argument", ctx);
            return Err(EvalResult::Error);
        }
        let r = match self.eval_expression(&args[0]) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Integer(n)) => Ok(*n),
            _ => {
                eprintln!("❌ ERROR: {}: argument must be an integer", ctx);
                Err(EvalResult::Error)
            }
        }
    }

    fn eval_to_bytes(
        &mut self,
        expr: &ast::Expression,
        ctx: &str,
    ) -> Result<Vec<u8>, EvalResult> {
        let r = match self.eval_expression(expr) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            other => return Err(other),
        };
        let elems = match self.resolve(r) {
            Some(ObjectData::Array { elements, .. }) => elements.clone(),
            _ => {
                eprintln!("❌ ERROR: {}: argument must be a byte array", ctx);
                return Err(EvalResult::Error);
            }
        };
        let mut bytes = Vec::with_capacity(elems.len());
        for elem in elems {
            match elem {
                OwnedValue::Integer(b) => bytes.push(b as u8),
                _ => {
                    eprintln!("❌ ERROR: {}: all elements must be integers", ctx);
                    return Err(EvalResult::Error);
                }
            }
        }
        Ok(bytes)
    }
}
