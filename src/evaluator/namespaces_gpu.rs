// GPU namespace — CPU-backed compute buffers with a GPU-shaped API
//
// Buffers are flat f64 arrays stored in the evaluator. The API mirrors
// GPU compute patterns (create, upload, dispatch, readback, free) so that
// a future backend can swap the CPU implementation for real GPU calls.
//
// GPU.createBuffer(size)                    → int   (buffer id, zero-filled)
// GPU.createBufferFromArray(arr)            → int   (buffer id, initialized from Serez array)
// GPU.readBuffer(id)                        → [decimal]  (copy buffer to Serez array)
// GPU.freeBuffer(id)                        → null
// GPU.fill(id, value)                       → null
// GPU.size(id)                              → int
// GPU.map(id, fn)                           → int   (new buffer, element-wise fn)
// GPU.reduce(id, fn, initial)               → decimal
// GPU.dot(id_a, id_b)                       → decimal
// GPU.axpy(alpha, id_x, id_y)              → int   (new buffer: alpha*x + y)
// GPU.matmul(id_a, rows_a, cols_a, id_b, rows_b, cols_b) → int (new buffer)

use crate::ast;
use crate::region::{ObjectData, ObjectRef, OwnedValue};
use super::EvalResult;

impl super::Evaluator {
    pub(super) fn eval_gpu_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "createBuffer" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "GPU.createBuffer(size) requires 1 argument");
                }
                let size = match self.eval_to_usize(&dot_call.arguments[0], "GPU.createBuffer") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if size > 256 * 1024 * 1024 {
                    eprintln!("❌ ERROR: GPU.createBuffer: size {} exceeds limit", size);
                    return EvalResult::Error;
                }
                let id = self.alloc_gpu_buffer(vec![0.0f64; size]);
                EvalResult::Value(self.alloc(ObjectData::Integer(id)))
            }

            "createBufferFromArray" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "GPU.createBufferFromArray(arr) requires 1 argument");
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let data = match self.array_to_f64_vec(arr_ref, "GPU.createBufferFromArray") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let id = self.alloc_gpu_buffer(data);
                EvalResult::Value(self.alloc(ObjectData::Integer(id)))
            }

            "readBuffer" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "GPU.readBuffer(id) requires 1 argument");
                }
                let id = match self.eval_gpu_id(&dot_call.arguments[0], "GPU.readBuffer") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let data = match self.gpu_buffers.get(&id) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.readBuffer: no buffer with id {}", id));
                    }
                };
                let owned: Vec<OwnedValue> = data
                    .iter()
                    .map(|&f| OwnedValue::Decimal(f))
                    .collect();
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("decimal".to_string()),
                    elements: owned,
                }))
            }

            "freeBuffer" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "GPU.freeBuffer(id) requires 1 argument");
                }
                let id = match self.eval_gpu_id(&dot_call.arguments[0], "GPU.freeBuffer") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                self.gpu_buffers.remove(&id);
                EvalResult::Value(self.null_ref)
            }

            "fill" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "GPU.fill(id, value) requires 2 arguments");
                }
                let id = match self.eval_gpu_id(&dot_call.arguments[0], "GPU.fill") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let val_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let val = match self.to_f64(val_ref) {
                    Some(f) => f,
                    None => {
                        return self.rt_err_kind("TypeError", "GPU.fill: value must be numeric");
                    }
                };
                match self.gpu_buffers.get_mut(&id) {
                    Some(buf) => buf.fill(val),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.fill: no buffer with id {}", id));
                    }
                }
                EvalResult::Value(self.null_ref)
            }

            "size" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "GPU.size(id) requires 1 argument");
                }
                let id = match self.eval_gpu_id(&dot_call.arguments[0], "GPU.size") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let n = match self.gpu_buffers.get(&id) {
                    Some(buf) => buf.len() as i64,
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.size: no buffer with id {}", id));
                    }
                };
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            "map" => {
                // GPU.map(id, fn) → new buffer id
                // Applies fn(element) element-wise; fn receives and returns a decimal.
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "GPU.map(id, fn) requires 2 arguments");
                }
                let id = match self.eval_gpu_id(&dot_call.arguments[0], "GPU.map") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let fn_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let data = match self.gpu_buffers.get(&id) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.map: no buffer with id {}", id));
                    }
                };
                let mut out = Vec::with_capacity(data.len());
                for val in data {
                    let arg = OwnedValue::Decimal(val);
                    match self.call_function(fn_ref, vec![arg]) {
                        EvalResult::Value(r) => match self.to_f64(r) {
                            Some(f) => out.push(f),
                            None => {
                                return self.rt_err_kind("GpuError", "GPU.map: callback must return a number");
                            }
                        },
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                }
                let new_id = self.alloc_gpu_buffer(out);
                EvalResult::Value(self.alloc(ObjectData::Integer(new_id)))
            }

            "reduce" => {
                // GPU.reduce(id, fn, initial) → decimal
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "GPU.reduce(id, fn, initial) requires 3 arguments");
                }
                let id = match self.eval_gpu_id(&dot_call.arguments[0], "GPU.reduce") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let fn_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let init_ref = match self.eval_expression(&dot_call.arguments[2]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let mut acc = match self.to_f64(init_ref) {
                    Some(f) => f,
                    None => {
                        return self.rt_err_kind("TypeError", "GPU.reduce: initial value must be numeric");
                    }
                };
                let data = match self.gpu_buffers.get(&id) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.reduce: no buffer with id {}", id));
                    }
                };
                for val in data {
                    let args = vec![OwnedValue::Decimal(acc), OwnedValue::Decimal(val)];
                    match self.call_function(fn_ref, args) {
                        EvalResult::Value(r) => match self.to_f64(r) {
                            Some(f) => acc = f,
                            None => {
                                return self.rt_err_kind("GpuError", "GPU.reduce: callback must return a number");
                            }
                        },
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Decimal(acc)))
            }

            "dot" => {
                // GPU.dot(id_a, id_b) → decimal
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "GPU.dot(id_a, id_b) requires 2 arguments");
                }
                let id_a = match self.eval_gpu_id(&dot_call.arguments[0], "GPU.dot") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let id_b = match self.eval_gpu_id(&dot_call.arguments[1], "GPU.dot") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let a = match self.gpu_buffers.get(&id_a) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.dot: no buffer with id {}", id_a));
                    }
                };
                let b = match self.gpu_buffers.get(&id_b) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.dot: no buffer with id {}", id_b));
                    }
                };
                if a.len() != b.len() {
                    return self.rt_err_kind("GpuError", format!("GPU.dot: buffer lengths differ ({} vs {})", a.len(),
                        b.len()));
                }
                let result: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
                EvalResult::Value(self.alloc(ObjectData::Decimal(result)))
            }

            "axpy" => {
                // GPU.axpy(alpha, id_x, id_y) → int (new buffer: alpha*x + y)
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "GPU.axpy(alpha, id_x, id_y) requires 3 arguments");
                }
                let alpha_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let alpha = match self.to_f64(alpha_ref) {
                    Some(f) => f,
                    None => {
                        return self.rt_err_kind("TypeError", "GPU.axpy: alpha must be numeric");
                    }
                };
                let id_x = match self.eval_gpu_id(&dot_call.arguments[1], "GPU.axpy") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let id_y = match self.eval_gpu_id(&dot_call.arguments[2], "GPU.axpy") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let x = match self.gpu_buffers.get(&id_x) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.axpy: no buffer with id {}", id_x));
                    }
                };
                let y = match self.gpu_buffers.get(&id_y) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.axpy: no buffer with id {}", id_y));
                    }
                };
                if x.len() != y.len() {
                    return self.rt_err_kind("GpuError", format!("GPU.axpy: buffer lengths differ ({} vs {})", x.len(),
                        y.len()));
                }
                let out: Vec<f64> = x.iter().zip(y.iter()).map(|(xi, yi)| alpha * xi + yi).collect();
                let new_id = self.alloc_gpu_buffer(out);
                EvalResult::Value(self.alloc(ObjectData::Integer(new_id)))
            }

            "matmul" => {
                // GPU.matmul(id_a, rows_a, cols_a, id_b, rows_b, cols_b) → int (new buffer)
                // rows_a × cols_a  ·  rows_b × cols_b  →  rows_a × cols_b
                // requires cols_a == rows_b
                if dot_call.arguments.len() != 6 {
                    return self.rt_err_kind("TypeError", "GPU.matmul(id_a, rows_a, cols_a, id_b, rows_b, cols_b) requires 6 arguments");
                }
                let id_a = match self.eval_gpu_id(&dot_call.arguments[0], "GPU.matmul") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let rows_a = match self.eval_to_usize(&dot_call.arguments[1], "GPU.matmul rows_a") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let cols_a = match self.eval_to_usize(&dot_call.arguments[2], "GPU.matmul cols_a") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let id_b = match self.eval_gpu_id(&dot_call.arguments[3], "GPU.matmul") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let rows_b = match self.eval_to_usize(&dot_call.arguments[4], "GPU.matmul rows_b") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let cols_b = match self.eval_to_usize(&dot_call.arguments[5], "GPU.matmul cols_b") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if cols_a != rows_b {
                    return self.rt_err_kind("GpuError", format!("GPU.matmul: cols_a ({}) must equal rows_b ({})", cols_a, rows_b));
                }
                let a = match self.gpu_buffers.get(&id_a) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.matmul: no buffer with id {}", id_a));
                    }
                };
                let b = match self.gpu_buffers.get(&id_b) {
                    Some(v) => v.clone(),
                    None => {
                        return self.rt_err_kind("GpuError", format!("GPU.matmul: no buffer with id {}", id_b));
                    }
                };
                if a.len() != rows_a * cols_a {
                    return self.rt_err_kind("TypeError", format!("GPU.matmul: buffer A has {} elements, expected {}×{}={}", a.len(), rows_a, cols_a, rows_a * cols_a));
                }
                if b.len() != rows_b * cols_b {
                    return self.rt_err_kind("TypeError", format!("GPU.matmul: buffer B has {} elements, expected {}×{}={}", b.len(), rows_b, cols_b, rows_b * cols_b));
                }
                // Standard O(n³) matmul
                let mut c = vec![0.0f64; rows_a * cols_b];
                for i in 0..rows_a {
                    for j in 0..cols_b {
                        let mut s = 0.0f64;
                        for k in 0..cols_a {
                            s += a[i * cols_a + k] * b[k * cols_b + j];
                        }
                        c[i * cols_b + j] = s;
                    }
                }
                let new_id = self.alloc_gpu_buffer(c);
                EvalResult::Value(self.alloc(ObjectData::Integer(new_id)))
            }

            _ => {
                self.rt_err_kind("TypeError", format!("Unknown GPU method '{}'", dot_call.method))
            }
        }
    }

    // ── GPU helpers ───────────────────────────────────────────────────────────

    fn alloc_gpu_buffer(&mut self, data: Vec<f64>) -> i64 {
        let id = self.gpu_next_id;
        self.gpu_next_id += 1;
        self.gpu_buffers.insert(id, data);
        id
    }

    fn eval_gpu_id(
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
                eprintln!("❌ ERROR: {}: buffer id must be an integer", ctx);
                Err(EvalResult::Error)
            }
        }
    }

    fn eval_to_usize(
        &mut self,
        expr: &ast::Expression,
        ctx: &str,
    ) -> Result<usize, EvalResult> {
        let r = match self.eval_expression(expr) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Integer(n)) if *n >= 0 => Ok(*n as usize),
            _ => {
                eprintln!("❌ ERROR: {}: argument must be a non-negative integer", ctx);
                Err(EvalResult::Error)
            }
        }
    }

    fn to_f64(&self, obj_ref: ObjectRef) -> Option<f64> {
        match self.resolve(obj_ref) {
            Some(ObjectData::Decimal(f)) => Some(*f),
            Some(ObjectData::Integer(i)) => Some(*i as f64),
            _ => None,
        }
    }

    fn array_to_f64_vec(
        &mut self,
        arr_ref: ObjectRef,
        ctx: &str,
    ) -> Result<Vec<f64>, EvalResult> {
        let elems = match self.resolve(arr_ref) {
            Some(ObjectData::Array { elements, .. }) => elements.clone(),
            _ => {
                eprintln!("❌ ERROR: {}: argument must be an array", ctx);
                return Err(EvalResult::Error);
            }
        };
        let mut out = Vec::with_capacity(elems.len());
        for r in elems {
            match r {
                OwnedValue::Integer(i) => out.push(i as f64),
                OwnedValue::Decimal(d) => out.push(d),
                _ => {
                    eprintln!("❌ ERROR: {}: all array elements must be numeric", ctx);
                    return Err(EvalResult::Error);
                }
            }
        }
        Ok(out)
    }
}
