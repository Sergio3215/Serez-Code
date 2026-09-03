use super::ExecutionFlow;
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

use super::EvalResult;
use super::binary_ops;
use crate::ast;
use crate::region::{ObjectData, OwnedValue};

impl super::Evaluator {
    /// `Binary.*` — argument evaluation, then the operation.
    ///
    /// **DEC-M6-001.** What is left here is what only the evaluator can do: read
    /// the call, run its argument expressions, and check arity. Everything after
    /// that is in [`super::binary_ops`], behind [`ValueSink`], where it depends
    /// on four operations instead of on this struct's thirty-eight fields.
    ///
    /// The messages, kinds and arity rules are unchanged, including the ones that
    /// read oddly — `require_one_int` says `{ctx}(n) requires 1 argument, got N`
    /// while the inline checks say `Binary.toHex(bytes) requires 1 argument`.
    /// Making those consistent is a diagnostic change and this is a refactor.
    pub(super) fn eval_binary_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        let method = dot_call.method.as_str();
        match method {
            "fromHex" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Binary.fromHex(hex) requires 1 argument");
                }
                let hex = match self.eval_to_string(&dot_call.arguments[0], "Binary.fromHex") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                binary_ops::from_hex(self, &hex)
            }

            "toHex" | "toUtf8" => {
                let label = if method == "toHex" {
                    "Binary.toHex(bytes) requires 1 argument"
                } else {
                    "Binary.toUtf8(bytes) requires 1 argument"
                };
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", label);
                }
                let elements = match self.eval_to_elements(&dot_call.arguments[0], method) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if method == "toHex" {
                    binary_ops::to_hex(self, &elements)
                } else {
                    binary_ops::to_utf8(self, &elements)
                }
            }

            "fromUtf8" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Binary.fromUtf8(s) requires 1 argument");
                }
                let s = match self.eval_to_string(&dot_call.arguments[0], "Binary.fromUtf8") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                binary_ops::from_utf8(self, &s)
            }

            "packInt32Le" | "packInt32Be" | "packInt64Le" => {
                let ctx = format!("Binary.{method}");
                let n = match self.require_one_int(&dot_call.arguments, &ctx) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                match method {
                    "packInt32Le" => binary_ops::pack_i32_le(self, n),
                    "packInt32Be" => binary_ops::pack_i32_be(self, n),
                    _ => binary_ops::pack_i64_le(self, n),
                }
            }

            "unpackInt32Le" | "unpackInt32Be" | "unpackInt64Le" => {
                let ctx = format!("Binary.{method}");
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", format!("{ctx}(bytes) requires 1 argument"));
                }
                let bytes = match self.eval_to_bytes(&dot_call.arguments[0], &ctx) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                match method {
                    "unpackInt32Le" => binary_ops::unpack_i32_le(self, &bytes),
                    "unpackInt32Be" => binary_ops::unpack_i32_be(self, &bytes),
                    _ => binary_ops::unpack_i64_le(self, &bytes),
                }
            }

            "concat" => {
                if dot_call.arguments.len() != 2 {
                    return self
                        .rt_err_kind("TypeError", "Binary.concat(a, b) requires 2 arguments");
                }
                let first = match self.eval_to_elements(&dot_call.arguments[0], "concat-first") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let second = match self.eval_to_elements(&dot_call.arguments[1], "concat-second") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                binary_ops::concat(self, &first, &second)
            }

            _ => self.rt_err_kind(
                "ReferenceError",
                format!("Unknown Binary method '{}'", dot_call.method),
            ),
        }
    }

    // ── Argument evaluation ───────────────────────────────────────────────────
    //
    // These stay on the evaluator: each one runs an expression, which is the one
    // capability `ValueSink` deliberately does not offer.

    /// Evaluate an argument that must be an array, and hand back its elements.
    ///
    /// `context` selects the message, because the four call sites word it
    /// differently and this is a refactor rather than a rewording.
    fn eval_to_elements(
        &mut self,
        expr: &ast::Expression,
        context: &str,
    ) -> Result<Vec<OwnedValue>, EvalResult> {
        let r = match self.eval_expression(expr) {
            Ok(ExecutionFlow::Value(r)) => r,
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Array { elements, .. }) => Ok(elements.clone()),
            _ => {
                let message = match context {
                    "toHex" => "Binary.toHex: argument must be an array".to_string(),
                    "toUtf8" => "Binary.toUtf8: argument must be an array".to_string(),
                    "concat-first" => "Binary.concat: first argument must be an array".to_string(),
                    _ => "Binary.concat: second argument must be an array".to_string(),
                };
                Err(self.rt_err_kind("TypeError", message))
            }
        }
    }

    fn require_one_int(&mut self, args: &[ast::Expression], ctx: &str) -> Result<i64, EvalResult> {
        if args.len() != 1 {
            let given = args.len();
            return Err(self.rt_err_kind(
                "TypeError",
                format!("{ctx}(n) requires 1 argument, got {given}"),
            ));
        }
        let r = match self.eval_expression(&args[0]) {
            Ok(ExecutionFlow::Value(r)) => r,
            Ok(ExecutionFlow::Throw(v)) => return Err(Ok(ExecutionFlow::Throw(v))),
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Integer(n)) => Ok(*n),
            _ => Err(self.rt_err_kind("TypeError", format!("{ctx}: argument must be an integer"))),
        }
    }

    fn eval_to_bytes(&mut self, expr: &ast::Expression, ctx: &str) -> Result<Vec<u8>, EvalResult> {
        let r = match self.eval_expression(expr) {
            Ok(ExecutionFlow::Value(r)) => r,
            Ok(ExecutionFlow::Throw(v)) => return Err(Ok(ExecutionFlow::Throw(v))),
            other => return Err(other),
        };
        let elems = match self.resolve(r) {
            Some(ObjectData::Array { elements, .. }) => elements.clone(),
            _ => {
                return Err(
                    self.rt_err_kind("TypeError", format!("{ctx}: argument must be a byte array"))
                );
            }
        };
        let mut bytes = Vec::with_capacity(elems.len());
        for elem in elems {
            match elem {
                OwnedValue::Integer(b) => bytes.push(b as u8),
                _ => {
                    return Err(self.rt_err_kind(
                        "TypeError",
                        format!("{ctx}: all elements must be integers"),
                    ));
                }
            }
        }
        Ok(bytes)
    }
}
