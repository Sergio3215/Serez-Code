use crate::ast;
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use super::EvalResult;

// ── Matrix multiply kernel (dependency-free) ──────────────────────────────────
// Cache-friendly `ikj` loop order (the inner loop walks `b` and `out` contiguously)
// and, for large products, parallelized across output rows with std scoped threads.
// The per-output accumulation order is unchanged (l ascending), so results are
// bit-identical to the previous naive `ijk` loop. Small matrices (serez-ai's
// typical case) stay single-threaded to avoid thread-spawn overhead.
const MATMUL_PARALLEL_FLOPS: usize = 262_144; // ~64x64x64 before threading kicks in

pub(crate) fn matmul_kernel(a: &[f64], b: &[f64], m: usize, k: usize, n: usize, out: &mut [f64]) {
    let flops = m.saturating_mul(n).saturating_mul(k);
    let threads = if flops >= MATMUL_PARALLEL_FLOPS {
        std::thread::available_parallelism().map(|p| p.get()).unwrap_or(1).min(m).max(1)
    } else {
        1
    };
    if threads <= 1 {
        matmul_rows(a, b, k, n, 0, out);
        return;
    }
    let rows_per = m.div_ceil(threads);
    std::thread::scope(|s| {
        let mut start_row = 0usize;
        for chunk in out.chunks_mut(rows_per * n) {
            let sr = start_row;
            s.spawn(move || matmul_rows(a, b, k, n, sr, chunk));
            start_row += rows_per;
        }
    });
}

// Fill `out` (a contiguous block of output rows starting at global row `start_row`)
// with A·B for those rows. `out` must be zeroed on entry.
fn matmul_rows(a: &[f64], b: &[f64], k: usize, n: usize, start_row: usize, out: &mut [f64]) {
    let rows = out.len() / n;
    for li in 0..rows {
        let a_row = (start_row + li) * k;
        let o = &mut out[li * n..li * n + n];
        for l in 0..k {
            let av = a[a_row + l];
            let bb = &b[l * n..l * n + n];
            for j in 0..n {
                o[j] += av * bb[j];
            }
        }
    }
}

impl super::Evaluator {
    // ── Constructor: new Tensor([rows, cols], fill) ───────────────────────────
    pub(super) fn eval_new_tensor(&mut self, new_expr: &ast::NewExpression) -> EvalResult {
        let args = match &new_expr.args {
            ast::NewArgs::Positional(a) => a.clone(),
            ast::NewArgs::Fields(_) => {
                return self.rt_err_kind("TypeError", "Tensor constructor requires positional arguments: new Tensor([shape], fill)");
            }
        };
        if args.is_empty() {
            return self.rt_err_kind("TypeError", "Tensor() requires at least a shape argument like [2, 3]");
        }
        let shape = match self.eval_shape_expr(&args[0]) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let total: usize = if shape.is_empty() { 0 } else { shape.iter().product() };
        if total > 10_000_000 {
            eprintln!("❌ ERROR: Tensor too large: {} elements (max 10M)", total);
            return EvalResult::Error;
        }
        let fill = if args.len() >= 2 {
            match self.eval_expression(&args[1]) {
                EvalResult::Value(r) => match self.resolve(r) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor fill value must be a number"); }
                },
                EvalResult::Throw(v) => return EvalResult::Throw(v),
                _ => return EvalResult::Error,
            }
        } else {
            0.0
        };
        EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![fill; total] , tid: 0}))
    }

    // ── Static namespace: Tensor.zeros / ones / eye / from ───────────────────
    pub(super) fn eval_tensor_static(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "zeros" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.zeros([shape]) requires 1 argument");
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                let total: usize = shape.iter().product();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![0.0; total] , tid: 0}))
            }
            "ones" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.ones([shape]) requires 1 argument");
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                let total: usize = shape.iter().product();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![1.0; total] , tid: 0}))
            }
            "eye" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.eye(n) requires 1 argument");
                }
                let n = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) if *n > 0 => *n as usize,
                        _ => { return self.rt_err_kind("TypeError", "Tensor.eye(n) requires a positive integer"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let mut data = vec![0.0f64; n * n];
                for i in 0..n { data[i * n + i] = 1.0; }
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![n, n], data , tid: 0}))
            }
            "from" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.from(array) requires 1 argument");
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                self.tensor_from_array(arr_ref)
            }
            _ => {
                self.rt_err_kind("TypeError", format!("Unknown Tensor static method '{}'", dot_call.method))
            }
        }
    }

    // ── Instance methods ──────────────────────────────────────────────────────
    pub(super) fn eval_tensor_method(
        &mut self,
        tensor_ref: ObjectRef,
        shape: Vec<usize>,
        data: Vec<f64>,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "shape" => {
                let owned: Vec<OwnedValue> = shape.iter()
                    .map(|&d| OwnedValue::Integer(d as i64))
                    .collect();
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: owned,
                }))
            }
            "ndim" => EvalResult::Value(self.alloc(ObjectData::Integer(shape.len() as i64))),
            "size" => {
                let total = shape.iter().product::<usize>() as i64;
                EvalResult::Value(self.alloc(ObjectData::Integer(total)))
            }
            "get" => {
                let idx = match self.tensor_flat_index(&shape, &dot_call.arguments.clone()) {
                    Ok(i) => i,
                    Err(e) => return e,
                };
                EvalResult::Value(self.alloc(ObjectData::Decimal(data[idx])))
            }
            "set" => {
                if dot_call.arguments.len() < 2 {
                    return self.rt_err_kind("TypeError", "Tensor.set() requires index arg(s) + value, e.g. set(0, 1, val)");
                }
                let n_args = dot_call.arguments.len();
                let index_args = dot_call.arguments[..n_args - 1].to_vec();
                let val_expr = dot_call.arguments[n_args - 1].clone();
                let idx = match self.tensor_flat_index(&shape, &index_args) {
                    Ok(i) => i, Err(e) => return e,
                };
                let val = match self.eval_expression(&val_expr) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { return self.rt_err_kind("TypeError", "Tensor.set() value must be a number"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let mut new_data = data;
                new_data[idx] = val;
                let new_obj = ObjectData::Tensor { shape, data: new_data , tid: 0};
                match tensor_ref.region {
                    RegionId::Global => self.global_arena.update(tensor_ref.index, new_obj),
                    RegionId::Scoped => self.scopes.arena.update(tensor_ref.index, new_obj),
                }
                EvalResult::Value(tensor_ref)
            }
            "reshape" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.reshape([shape]) requires 1 argument");
                }
                let new_shape = match self.eval_shape_expr(&dot_call.arguments[0].clone()) {
                    Ok(s) => s, Err(e) => return e,
                };
                let new_total: usize = new_shape.iter().product();
                if new_total != data.len() {
                    return self.rt_err_kind("TensorError", format!("Tensor.reshape() — shape has {} elements but tensor has {}", new_total, data.len()));
                }
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data , tid: 0}))
            }
            "transpose" => {
                if shape.len() != 2 {
                    return self.rt_err_kind("TensorError", "Tensor.transpose() only supported for 2D tensors");
                }
                let (rows, cols) = (shape[0], shape[1]);
                let mut new_data = vec![0.0f64; data.len()];
                for r in 0..rows {
                    for c in 0..cols {
                        new_data[c * rows + r] = data[r * cols + c];
                    }
                }
                let out_ref = self.alloc(ObjectData::Tensor { shape: vec![cols, rows], data: new_data, tid: 0 });
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Transpose { in_id, rows, cols });
                }
                EvalResult::Value(out_ref)
            }
            "add" => self.tensor_elementwise(tensor_ref, shape, data, dot_call, "add"),
            "sub" => self.tensor_elementwise(tensor_ref, shape, data, dot_call, "sub"),
            "mul" => self.tensor_elementwise(tensor_ref, shape, data, dot_call, "mul"),
            "div" => self.tensor_elementwise(tensor_ref, shape, data, dot_call, "div"),
            "dot" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.dot() requires 1 argument");
                }
                let arg_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.resolve(arg_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s2, data: d2 , ..}) => {
                        if shape.len() != 1 || s2.len() != 1 {
                            return self.rt_err_kind("TensorError", "Tensor.dot() only works on 1D tensors (use matmul for 2D)");
                        }
                        if shape[0] != s2[0] {
                            return self.rt_err_kind("TensorError", format!("Tensor.dot() length mismatch: {} vs {}", shape[0], s2[0]));
                        }
                        let result: f64 = data.iter().zip(d2.iter()).map(|(a, b)| a * b).sum();
                        EvalResult::Value(self.alloc(ObjectData::Decimal(result)))
                    }
                    _ => {
                        self.rt_err_kind("TypeError", "Tensor.dot() requires a 1D Tensor argument")
                    }
                }
            }
            "matmul" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.matmul() requires 1 argument");
                }
                let arg_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.resolve(arg_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s2, data: d2 , ..}) => {
                        if shape.len() != 2 || s2.len() != 2 {
                            return self.rt_err_kind("TypeError", "Tensor.matmul() requires 2D tensors");
                        }
                        let (m, k, k2, n) = (shape[0], shape[1], s2[0], s2[1]);
                        if k != k2 {
                            return self.rt_err_kind("TensorError", format!("Tensor.matmul() inner dimensions must match: {} != {}", k, k2));
                        }
                        let mut result = vec![0.0f64; m * n];
                        matmul_kernel(&data, &d2, m, k, n, &mut result);
                        let out_ref = self.alloc(ObjectData::Tensor { shape: vec![m, n], data: result, tid: 0 });
                        if self.ad_recording {
                            let a_id = self.ad_tensor_id(tensor_ref);
                            let b_id = self.ad_tensor_id(arg_ref);
                            self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::MatMul {
                                a_id, b_id, a_rows: m, a_cols: k, b_cols: n,
                                a_data: data.clone(), b_data: d2.clone(),
                            });
                        }
                        EvalResult::Value(out_ref)
                    }
                    _ => {
                        self.rt_err_kind("TypeError", "Tensor.matmul() requires a 2D Tensor argument")
                    }
                }
            }
            "sum" => {
                if data.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Decimal(0.0)));
                }
                let in_len = data.len();
                let s: f64 = data.iter().sum();
                if self.ad_recording {
                    // Return a 1-element Tensor so backward() can track it
                    let out_ref = self.alloc(ObjectData::Tensor { shape: vec![1], data: vec![s], tid: 0 });
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Sum { in_id, in_len });
                    EvalResult::Value(out_ref)
                } else {
                    EvalResult::Value(self.alloc(ObjectData::Decimal(s)))
                }
            }
            "mean" => {
                if data.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Decimal(0.0)));
                }
                let in_len = data.len();
                let m = data.iter().sum::<f64>() / in_len as f64;
                if self.ad_recording {
                    let out_ref = self.alloc(ObjectData::Tensor { shape: vec![1], data: vec![m], tid: 0 });
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Mean { in_id, in_len });
                    EvalResult::Value(out_ref)
                } else {
                    EvalResult::Value(self.alloc(ObjectData::Decimal(m)))
                }
            }
            "max" => {
                if data.is_empty() {
                    return self.rt_err_kind("TensorError", "Tensor.max() on empty tensor");
                }
                let v = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                EvalResult::Value(self.alloc(ObjectData::Decimal(v)))
            }
            "min" => {
                if data.is_empty() {
                    return self.rt_err_kind("TensorError", "Tensor.min() on empty tensor");
                }
                let v = data.iter().cloned().fold(f64::INFINITY, f64::min);
                EvalResult::Value(self.alloc(ObjectData::Decimal(v)))
            }
            "flatten" => {
                let len = data.len();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![len], data , tid: 0}))
            }
            "fill" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.fill(val) requires 1 argument");
                }
                let val = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { return self.rt_err_kind("TypeError", "Tensor.fill() value must be a number"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![val; data.len()] , tid: 0}))
            }
            "toArray" => {
                if shape.len() == 1 {
                    let owned: Vec<OwnedValue> = data.iter()
                        .map(|&x| OwnedValue::Decimal(x))
                        .collect();
                    EvalResult::Value(self.alloc(ObjectData::Array {
                        element_type: Some("decimal".to_string()),
                        elements: owned,
                    }))
                } else if shape.len() == 2 {
                    let (rows, cols) = (shape[0], shape[1]);
                    let row_owned: Vec<OwnedValue> = (0..rows).map(|r| {
                        let col_owned: Vec<OwnedValue> = (0..cols)
                            .map(|c| OwnedValue::Decimal(data[r * cols + c]))
                            .collect();
                        OwnedValue::Array {
                            element_type: Some("decimal".to_string()),
                            elements: col_owned,
                        }
                    }).collect();
                    EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: row_owned }))
                } else {
                    // Higher dims: return flat
                    let owned: Vec<OwnedValue> = data.iter()
                        .map(|&x| OwnedValue::Decimal(x))
                        .collect();
                    EvalResult::Value(self.alloc(ObjectData::Array {
                        element_type: Some("decimal".to_string()),
                        elements: owned,
                    }))
                }
            }
            "toString" => {
                let s = self.display(tensor_ref);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }

            // ── Activation functions ─────────────────────────────────────────
            "relu" => {
                let cached = data.clone();
                let new_data: Vec<f64> = data.iter().map(|&x| if x > 0.0 { x } else { 0.0 }).collect();
                let out_ref = self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0});
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Relu { in_id, cached_input: cached });
                }
                EvalResult::Value(out_ref)
            }
            "sigmoid" => {
                let new_data: Vec<f64> = data.iter().map(|&x| 1.0 / (1.0 + (-x).exp())).collect();
                let out_ref = self.alloc(ObjectData::Tensor { shape, data: new_data.clone() , tid: 0});
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Sigmoid { in_id, cached_output: new_data });
                }
                EvalResult::Value(out_ref)
            }
            "tanh" => {
                let new_data: Vec<f64> = data.iter().map(|&x| x.tanh()).collect();
                let out_ref = self.alloc(ObjectData::Tensor { shape, data: new_data.clone() , tid: 0});
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Tanh { in_id, cached_output: new_data });
                }
                EvalResult::Value(out_ref)
            }
            "softmax" => {
                // Row-wise softmax for 2D, global for 1D — both tracked by autodiff
                if data.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data, tid: 0 }));
                }
                let (rows, cols) = if shape.len() >= 2 { (shape[0], shape[1..].iter().product()) } else { (1, data.len()) };
                let mut new_data = vec![0.0f64; data.len()];
                for r in 0..rows {
                    let row = &data[r*cols..(r+1)*cols];
                    let mx = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let exps: Vec<f64> = row.iter().map(|&x| (x - mx).exp()).collect();
                    let s: f64 = exps.iter().sum();
                    for c in 0..cols { new_data[r*cols+c] = exps[c] / s; }
                }
                let out_ref = self.alloc(ObjectData::Tensor { shape: shape.clone(), data: new_data.clone(), tid: 0 });
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Softmax {
                        in_id, rows, cols, cached_out: new_data,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── Element-wise math ────────────────────────────────────────────
            "abs" => {
                let new_data: Vec<f64> = data.iter().map(|&x| x.abs()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }
            "sqrt" => {
                for &x in &data {
                    if x < 0.0 { return self.rt_err_kind("TensorError", format!("Tensor.sqrt() — negative value {}", x)); }
                }
                let new_data: Vec<f64> = data.iter().map(|&x| x.sqrt()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }
            "exp" => {
                let new_data: Vec<f64> = data.iter().map(|&x| x.exp()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }
            "log" => {
                for &x in &data {
                    if x <= 0.0 { return self.rt_err_kind("TensorError", format!("Tensor.log() — non-positive value {}", x)); }
                }
                let new_data: Vec<f64> = data.iter().map(|&x| x.ln()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }
            "pow" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.pow(exponent) requires 1 argument");
                }
                let exp = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { return self.rt_err_kind("TypeError", "Tensor.pow() exponent must be a number"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let new_data: Vec<f64> = data.iter().map(|&x| x.powf(exp)).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }

            // ── Norms ────────────────────────────────────────────────────────
            "norm" => {
                // norm(order=2) — L1 (order=1) or L2 (order=2, default)
                let order = if dot_call.arguments.is_empty() {
                    2i64
                } else {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => match self.resolve(r) {
                            Some(ObjectData::Integer(n)) => *n,
                            _ => { return self.rt_err_kind("TypeError", "Tensor.norm() order must be an integer"); }
                        },
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                };
                let result = match order {
                    1 => data.iter().map(|&x| x.abs()).sum::<f64>(),
                    2 => data.iter().map(|&x| x * x).sum::<f64>().sqrt(),
                    _ => { return self.rt_err_kind("TensorError", format!("Tensor.norm() supports order 1 or 2, got {}", order)); }
                };
                EvalResult::Value(self.alloc(ObjectData::Decimal(result)))
            }

            // ── Clamp ────────────────────────────────────────────────────────
            "clamp" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.clamp(min, max) requires 2 arguments");
                }
                let lo = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { return self.rt_err_kind("TypeError", "Tensor.clamp() min must be a number"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let hi = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { return self.rt_err_kind("TypeError", "Tensor.clamp() max must be a number"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                if lo > hi { return self.rt_err_kind("TensorError", "Tensor.clamp() min > max"); }
                let new_data: Vec<f64> = data.iter().map(|&x| x.clamp(lo, hi)).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }

            // ── Broadcast add: (m,n) + (n,) ─────────────────────────────────
            "broadcastAdd" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.broadcastAdd(bias) requires 1 argument");
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TensorError", "Tensor.broadcastAdd() only supported for 2D tensors");
                }
                let (rows, cols) = (shape[0], shape[1]);
                let bias_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let bias_data = match self.resolve(bias_ref).cloned() {
                    Some(ObjectData::Tensor { shape: bs, data: bd , ..}) => {
                        if bs != vec![cols] {
                            return self.rt_err_kind("TensorError", format!("Tensor.broadcastAdd() bias shape {:?} must match last dim {}", bs, cols));
                        }
                        bd
                    }
                    _ => { return self.rt_err_kind("TypeError", "Tensor.broadcastAdd() argument must be a 1D Tensor"); }
                };
                let mut new_data = data.clone();
                for r in 0..rows {
                    for c in 0..cols {
                        new_data[r * cols + c] += bias_data[c];
                    }
                }
                let out_ref = self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0});
                if self.ad_recording {
                    let mat_id = self.ad_tensor_id(tensor_ref);
                    let bias_id = self.ad_tensor_id(bias_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::BroadcastAdd { mat_id, bias_id, rows, cols });
                }
                EvalResult::Value(out_ref)
            }

            // ── Broadcast mul/sub/div: (m,n) op (n,) ────────────────────────
            "broadcastMul" | "broadcastSub" | "broadcastDiv" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", format!("Tensor.{}() requires 1 argument", dot_call.method));
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TensorError", format!("Tensor.{}() only supported for 2D tensors", dot_call.method));
                }
                let (rows, cols) = (shape[0], shape[1]);
                let rhs_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let rhs_data = match self.resolve(rhs_ref).cloned() {
                    Some(ObjectData::Tensor { shape: rs, data: rd , ..}) => {
                        if rs != vec![cols] {
                            return self.rt_err_kind("TensorError", format!("Tensor.{}() rhs shape {:?} must match last dim {}", dot_call.method, rs, cols));
                        }
                        rd
                    }
                    _ => { return self.rt_err_kind("TypeError", format!("Tensor.{}() argument must be a 1D Tensor", dot_call.method)); }
                };
                if dot_call.method == "broadcastDiv" {
                    for v in &rhs_data {
                        if *v == 0.0 {
                            return self.rt_err_kind("TensorError", "Tensor.broadcastDiv() division by zero in rhs");
                        }
                    }
                }
                let mut new_data = data.clone();
                for r in 0..rows {
                    for c in 0..cols {
                        let idx = r * cols + c;
                        match dot_call.method.as_str() {
                            "broadcastMul" => new_data[idx] *= rhs_data[c],
                            "broadcastSub" => new_data[idx] -= rhs_data[c],
                            "broadcastDiv" => new_data[idx] /= rhs_data[c],
                            _ => unreachable!(),
                        }
                    }
                }
                let out_ref = self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0});
                if self.ad_recording && dot_call.method == "broadcastMul" {
                    let mat_id  = self.ad_tensor_id(tensor_ref);
                    let rhs_id  = self.ad_tensor_id(rhs_ref);
                    let mat_saved = data.clone();
                    let rhs_saved = rhs_data.clone();
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::BroadcastMul {
                        mat_id, rhs_id, rows, cols, mat_data: mat_saved, rhs_data: rhs_saved,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── sum(axis) / mean(axis) ────────────────────────────────────────
            "sumAxis" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.sumAxis(axis) requires 1 argument");
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TensorError", "Tensor.sumAxis() only supported for 2D tensors");
                }
                let (rows, cols) = (shape[0], shape[1]);
                let ax_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n == 0 || n == 1 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.sumAxis() axis must be 0 or 1"); }
                };
                if axis == 0 {
                    let mut out = vec![0.0f64; cols];
                    for r in 0..rows { for c in 0..cols { out[c] += data[r * cols + c]; } }
                    EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![cols], data: out, tid: 0 }))
                } else {
                    let mut out = vec![0.0f64; rows];
                    for r in 0..rows { for c in 0..cols { out[r] += data[r * cols + c]; } }
                    EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![rows], data: out, tid: 0 }))
                }
            }

            "meanAxis" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.meanAxis(axis) requires 1 argument");
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TensorError", "Tensor.meanAxis() only supported for 2D tensors");
                }
                let (rows, cols) = (shape[0], shape[1]);
                let ax_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n == 0 || n == 1 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.meanAxis() axis must be 0 or 1"); }
                };
                if axis == 0 {
                    let mut out = vec![0.0f64; cols];
                    for r in 0..rows { for c in 0..cols { out[c] += data[r * cols + c]; } }
                    for v in &mut out { *v /= rows as f64; }
                    EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![cols], data: out, tid: 0 }))
                } else {
                    let mut out = vec![0.0f64; rows];
                    for r in 0..rows { for c in 0..cols { out[r] += data[r * cols + c]; } }
                    for v in &mut out { *v /= cols as f64; }
                    EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![rows], data: out, tid: 0 }))
                }
            }

            // ── argmax / argmin ───────────────────────────────────────────────
            "argmax" => {
                if data.is_empty() {
                    return self.rt_err_kind("TensorError", "Tensor.argmax() on empty tensor");
                }
                let idx = data.iter().enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i).unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(idx as i64)))
            }

            "argmin" => {
                if data.is_empty() {
                    return self.rt_err_kind("TensorError", "Tensor.argmin() on empty tensor");
                }
                let idx = data.iter().enumerate()
                    .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i).unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(idx as i64)))
            }

            // ── slice(start, end) — flat index range ──────────────────────────
            "slice" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.slice(start, end) requires 2 arguments");
                }
                let r0 = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let r1 = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let (start, end) = match (self.resolve(r0).cloned(), self.resolve(r1).cloned()) {
                    (Some(ObjectData::Integer(a)), Some(ObjectData::Integer(b))) => (a as usize, b as usize),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.slice() arguments must be integers"); }
                };
                if start > end || end > data.len() {
                    return self.rt_err_kind("TensorError", format!("Tensor.slice() invalid range {}..{} for tensor of length {}", start, end, data.len()));
                }
                let sliced = data[start..end].to_vec();
                let len = end - start;
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![len], data: sliced, tid: 0 }))
            }

            // ── concat(other, axis) — 2D row/col concatenation ────────────────
            "concat" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.concat(other, axis) requires 2 arguments");
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TensorError", "Tensor.concat() only supported for 2D tensors");
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let ax_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n == 0 || n == 1 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.concat() axis must be 0 or 1"); }
                };
                let (rows, cols) = (shape[0], shape[1]);
                match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { shape: os, data: od , ..}) => {
                        if axis == 0 {
                            if os.len() != 2 || os[1] != cols {
                                return self.rt_err_kind("TensorError", format!("Tensor.concat(axis=0) column mismatch: {} vs {}", os.get(1).copied().unwrap_or(0), cols));
                            }
                            let mut out = data.clone();
                            out.extend_from_slice(&od);
                            EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![rows + os[0], cols], data: out , tid: 0}))
                        } else {
                            if os.len() != 2 || os[0] != rows {
                                return self.rt_err_kind("TensorError", format!("Tensor.concat(axis=1) row mismatch: {} vs {}", os.get(0).copied().unwrap_or(0), rows));
                            }
                            let other_cols = os[1];
                            let new_cols = cols + other_cols;
                            let mut out = Vec::with_capacity(rows * new_cols);
                            for r in 0..rows {
                                out.extend_from_slice(&data[r * cols..(r + 1) * cols]);
                                out.extend_from_slice(&od[r * other_cols..(r + 1) * other_cols]);
                            }
                            EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![rows, new_cols], data: out, tid: 0 }))
                        }
                    }
                    _ => { self.rt_err_kind("TypeError", "Tensor.concat() argument must be a Tensor") }
                }
            }

            // ── neg / scale ───────────────────────────────────────────────────
            "neg" => {
                let new_data: Vec<f64> = data.iter().map(|x| -x).collect();
                let out_ref = self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0});
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Neg { in_id });
                }
                EvalResult::Value(out_ref)
            }

            "scale" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.scale(s) requires 1 argument");
                }
                let s_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let s = match self.resolve(s_ref).cloned() {
                    Some(ObjectData::Integer(n)) => n as f64,
                    Some(ObjectData::Decimal(d)) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.scale() argument must be a number"); }
                };
                let new_data: Vec<f64> = data.iter().map(|x| x * s).collect();
                let out_ref = self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0});
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Scale { in_id, scalar: s });
                }
                EvalResult::Value(out_ref)
            }

            // ── leaky_relu / gelu ─────────────────────────────────────────────
            "leaky_relu" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.leaky_relu(alpha) requires 1 argument");
                }
                let a_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let alpha = match self.resolve(a_ref).cloned() {
                    Some(ObjectData::Integer(n)) => n as f64,
                    Some(ObjectData::Decimal(d)) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.leaky_relu() alpha must be a number"); }
                };
                let cached_input = data.clone();
                let new_data: Vec<f64> = data.iter().map(|&x| if x >= 0.0 { x } else { alpha * x }).collect();
                let out_ref = self.alloc_tensor(shape, new_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::LeakyRelu { in_id, cached_input, alpha });
                }
                EvalResult::Value(out_ref)
            }

            "gelu" => {
                // Gaussian Error Linear Unit: x * Φ(x) approximated via tanh
                let c = (2.0f64 / std::f64::consts::PI).sqrt();
                let cached_input = data.clone();
                let new_data: Vec<f64> = data.iter().map(|&x| {
                    0.5 * x * (1.0 + (c * (x + 0.044715 * x * x * x)).tanh())
                }).collect();
                let out_ref = self.alloc_tensor(shape, new_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Gelu { in_id, cached_input });
                }
                EvalResult::Value(out_ref)
            }

            // ── Phase 1-2: New activations (tracked) ─────────────────────────
            "elu" => {
                let alpha = if dot_call.arguments.is_empty() { 1.0 } else {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => match self.resolve(r) {
                            Some(ObjectData::Decimal(d)) => *d,
                            Some(ObjectData::Integer(n)) => *n as f64,
                            _ => 1.0,
                        },
                        _ => 1.0,
                    }
                };
                let cached_input = data.clone();
                let new_data: Vec<f64> = data.iter().map(|&x| {
                    if x > 0.0 { x } else { alpha * (x.exp() - 1.0) }
                }).collect();
                let out_ref = self.alloc_tensor(shape, new_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Elu { in_id, cached_input, alpha });
                }
                EvalResult::Value(out_ref)
            }

            "swish" | "silu" => {
                // swish(x) = x * sigmoid(x)  — also known as SiLU
                let cached_input = data.clone();
                let cached_sigmoid: Vec<f64> = data.iter()
                    .map(|&x| 1.0 / (1.0 + (-x).exp())).collect();
                let new_data: Vec<f64> = data.iter().zip(cached_sigmoid.iter())
                    .map(|(&x, &s)| x * s).collect();
                let out_ref = self.alloc_tensor(shape, new_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Swish {
                        in_id, cached_input, cached_sigmoid,
                    });
                }
                EvalResult::Value(out_ref)
            }

            "mish" => {
                // mish(x) = x * tanh(softplus(x))  where softplus(x) = ln(1 + e^x)
                let cached_input = data.clone();
                let new_data: Vec<f64> = data.iter().map(|&x| {
                    let sp = (1.0 + x.exp()).ln();
                    x * sp.tanh()
                }).collect();
                let out_ref = self.alloc_tensor(shape, new_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Mish { in_id, cached_input });
                }
                EvalResult::Value(out_ref)
            }

            // leaky_relu with autodiff tracking
            "leaky_relu_tracked" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.leaky_relu_tracked(alpha) requires 1 argument");
                }
                let a_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let alpha = match self.resolve(a_ref).cloned() {
                    Some(ObjectData::Integer(n)) => n as f64,
                    Some(ObjectData::Decimal(d)) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.leaky_relu_tracked() alpha must be a number"); }
                };
                let cached_input = data.clone();
                let new_data: Vec<f64> = data.iter().map(|&x| if x >= 0.0 { x } else { alpha * x }).collect();
                let out_ref = self.alloc_tensor(shape, new_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::LeakyRelu { in_id, cached_input, alpha });
                }
                EvalResult::Value(out_ref)
            }

            // ── Phase 2: avg_pool2d ───────────────────────────────────────────
            "avg_pool2d" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.avg_pool2d(kernel, stride) requires 2 arguments");
                }
                if shape.len() != 4 {
                    return self.rt_err_kind("TypeError", "Tensor.avg_pool2d() input must be 4D [N, H, W, C]");
                }
                let k_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let s_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let kernel = match self.resolve(k_ref) {
                    Some(ObjectData::Integer(n)) if *n > 0 => *n as usize,
                    _ => { return self.rt_err_kind("TypeError", "avg_pool2d kernel must be a positive integer"); }
                };
                let stride = match self.resolve(s_ref) {
                    Some(ObjectData::Integer(n)) if *n > 0 => *n as usize,
                    _ => { return self.rt_err_kind("TypeError", "avg_pool2d stride must be a positive integer"); }
                };
                let (n_batch, in_h, in_w, ch) = (shape[0], shape[1], shape[2], shape[3]);
                let out_h = (in_h - kernel) / stride + 1;
                let out_w = (in_w - kernel) / stride + 1;
                let pool_area = (kernel * kernel) as f64;
                let mut out_data = vec![0.0_f64; n_batch * out_h * out_w * ch];
                for nb in 0..n_batch {
                    for oh in 0..out_h {
                        for ow in 0..out_w {
                            for c in 0..ch {
                                let mut s = 0.0_f64;
                                for kh in 0..kernel {
                                    for kw in 0..kernel {
                                        let ih = oh * stride + kh;
                                        let iw = ow * stride + kw;
                                        s += data[nb*in_h*in_w*ch + ih*in_w*ch + iw*ch + c];
                                    }
                                }
                                out_data[nb*out_h*out_w*ch + oh*out_w*ch + ow*ch + c] = s / pool_area;
                            }
                        }
                    }
                }
                let out_shape = vec![n_batch, out_h, out_w, ch];
                let out_ref = self.alloc_tensor(out_shape.clone(), out_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::AvgPool2d {
                        in_id, kernel, stride,
                        in_shape: shape.clone(),
                        out_shape,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── Phase 3: Additional tensor utilities ──────────────────────────
            "variance" => {
                // .variance() → scalar tensor
                let n = data.len() as f64;
                let mean = data.iter().sum::<f64>() / n;
                let var = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![1], data: vec![var], tid: 0 }))
            }

            "std" => {
                // .std() → scalar tensor
                let n = data.len() as f64;
                let mean = data.iter().sum::<f64>() / n;
                let var = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![1], data: vec![var.sqrt()], tid: 0 }))
            }

            "cumsum" => {
                // .cumsum() → flat cumulative sum tensor
                let mut out = Vec::with_capacity(data.len());
                let mut acc = 0.0_f64;
                for &x in &data { acc += x; out.push(acc); }
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![data.len()], data: out, tid: 0 }))
            }

            "softplus" => {
                // softplus(x) = log(1 + exp(x))
                let new_data: Vec<f64> = data.iter()
                    .map(|&x| (1.0 + x.exp()).ln()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "hardsigmoid" => {
                // hard sigmoid: clamp((x+3)/6, 0, 1)
                let new_data: Vec<f64> = data.iter()
                    .map(|&x| ((x + 3.0) / 6.0).clamp(0.0, 1.0)).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "hardswish" => {
                // hard swish: x * hardsigmoid(x)
                let new_data: Vec<f64> = data.iter()
                    .map(|&x| x * ((x + 3.0) / 6.0).clamp(0.0, 1.0)).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            // ── Phase 3: N-D shape manipulation ──────────────────────────────
            "unsqueeze" => {
                // .unsqueeze(dim) — insert dim of size 1 at position dim
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.unsqueeze(dim) requires 1 argument");
                }
                let d_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let dim = match self.resolve(d_ref) {
                    Some(ObjectData::Integer(n)) => {
                        let ndim = shape.len() as i64 + 1;
                        let d = if *n < 0 { ndim + n } else { *n };
                        if d < 0 || d > ndim { return self.rt_err_kind("TensorError", format!("unsqueeze dim {} out of range for {}D tensor", n, shape.len())); }
                        d as usize
                    }
                    _ => { return self.rt_err_kind("TypeError", "unsqueeze dim must be an integer"); }
                };
                let mut new_shape = shape.clone();
                new_shape.insert(dim, 1);
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data, tid: 0 }))
            }

            "squeeze" => {
                // .squeeze() — remove all dims of size 1
                // .squeeze(dim) — remove specific dim if size is 1
                let new_shape: Vec<usize> = if dot_call.arguments.is_empty() {
                    shape.iter().cloned().filter(|&d| d != 1).collect()
                } else {
                    let d_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                    let dim = match self.resolve(d_ref) {
                        Some(ObjectData::Integer(n)) => {
                            let nd = shape.len() as i64;
                            let d = if *n < 0 { nd + n } else { *n };
                            d as usize
                        }
                        _ => { return self.rt_err_kind("TypeError", "squeeze dim must be an integer"); }
                    };
                    if dim < shape.len() && shape[dim] == 1 {
                        let mut s = shape.clone(); s.remove(dim); s
                    } else { shape.clone() }
                };
                let new_shape = if new_shape.is_empty() { vec![1] } else { new_shape };
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data, tid: 0 }))
            }

            "permute" => {
                // .permute([ax0, ax1, ...]) — N-D generalized transpose
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.permute([axes]) requires 1 argument");
                }
                let axes_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let axes: Vec<usize> = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s,
                    Err(_) => match self.resolve(axes_ref).cloned() {
                        Some(ObjectData::Array { elements, .. }) => elements.iter().filter_map(|e| {
                            match e { OwnedValue::Integer(n) => Some(*n as usize), _ => None }
                        }).collect(),
                        _ => { return self.rt_err_kind("TypeError", "Tensor.permute() axes must be an array"); }
                    }
                };
                if axes.len() != shape.len() {
                    return self.rt_err_kind("TensorError", format!("Tensor.permute() axes length {} must match tensor ndim {}", axes.len(), shape.len()));
                }
                let new_shape: Vec<usize> = axes.iter().map(|&a| shape[a]).collect();
                let total: usize = new_shape.iter().product();
                let mut new_data = vec![0.0_f64; total];
                // Compute strides for old shape
                let ndim = shape.len();
                let mut old_strides = vec![1usize; ndim];
                for i in (0..ndim - 1).rev() { old_strides[i] = old_strides[i+1] * shape[i+1]; }
                let mut new_strides = vec![1usize; ndim];
                for i in (0..ndim - 1).rev() { new_strides[i] = new_strides[i+1] * new_shape[i+1]; }
                // Iterate over new index space
                for new_flat in 0..total {
                    // Decode new_flat into new multi-index
                    let mut rem = new_flat;
                    let mut new_idx = vec![0usize; ndim];
                    for i in 0..ndim { new_idx[i] = rem / new_strides[i]; rem %= new_strides[i]; }
                    // Map new_idx back to old_idx via axes permutation
                    let mut old_flat = 0usize;
                    for i in 0..ndim { old_flat += new_idx[i] * old_strides[axes[i]]; }
                    new_data[new_flat] = data[old_flat];
                }
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data: new_data, tid: 0 }))
            }

            // ── Phase 3: N-D broadcasting ─────────────────────────────────────
            "broadcastTo" => {
                // .broadcastTo([shape]) — expand to target shape (numpy semantics)
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.broadcastTo([shape]) requires 1 argument");
                }
                let target_shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                match Self::broadcast_data(&data, &shape, &target_shape) {
                    Some(new_data) => EvalResult::Value(self.alloc(ObjectData::Tensor { shape: target_shape, data: new_data, tid: 0 })),
                    None => { self.rt_err_kind("TensorError", format!("Tensor.broadcastTo() incompatible shapes {:?} → {:?}", shape, target_shape)) }
                }
            }

            "broadcastAddNd" => {
                // N-D broadcast add (full numpy semantics)
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.broadcastAddNd(other) requires 1 argument");
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let (other_data, other_shape) = match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, shape: s, .. }) => (d, s),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.broadcastAddNd() argument must be a Tensor"); }
                };
                let out_shape = match Self::broadcast_shape(&shape, &other_shape) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TensorError", format!("Tensor.broadcastAddNd() shapes {:?} and {:?} are not broadcastable", shape, other_shape)); }
                };
                let a_data = Self::broadcast_data(&data,       &shape,       &out_shape).unwrap();
                let b_data = Self::broadcast_data(&other_data, &other_shape, &out_shape).unwrap();
                let result: Vec<f64> = a_data.iter().zip(b_data.iter()).map(|(a,b)| a+b).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: out_shape, data: result, tid: 0 }))
            }

            "broadcastMulNd" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.broadcastMulNd(other) requires 1 argument");
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let (other_data, other_shape) = match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, shape: s, .. }) => (d, s),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.broadcastMulNd() argument must be a Tensor"); }
                };
                let out_shape = match Self::broadcast_shape(&shape, &other_shape) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TensorError", "Tensor.broadcastMulNd() shapes not broadcastable"); }
                };
                let a_data = Self::broadcast_data(&data,       &shape,       &out_shape).unwrap();
                let b_data = Self::broadcast_data(&other_data, &other_shape, &out_shape).unwrap();
                let result: Vec<f64> = a_data.iter().zip(b_data.iter()).map(|(a,b)| a*b).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: out_shape, data: result, tid: 0 }))
            }

            // ── Phase 3: Batch matmul ─────────────────────────────────────────
            "bmm" => {
                // .bmm(other) — batch matmul: [B,N,M] @ [B,M,K] → [B,N,K]
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.bmm(other) requires 1 argument");
                }
                if shape.len() != 3 {
                    return self.rt_err_kind("TypeError", format!("Tensor.bmm() requires 3D tensors [B,N,M], got {}D", shape.len()));
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let (other_data, other_shape) = match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, shape: s, .. }) => (d, s),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.bmm() argument must be a Tensor"); }
                };
                if other_shape.len() != 3 || other_shape[0] != shape[0] || other_shape[1] != shape[2] {
                    return self.rt_err_kind("TensorError", format!("Tensor.bmm() shape mismatch: {:?} @ {:?}", shape, other_shape));
                }
                let (b, n, m, k) = (shape[0], shape[1], shape[2], other_shape[2]);
                let mut out_data = vec![0.0_f64; b * n * k];
                for bi in 0..b {
                    for i in 0..n {
                        for l in 0..m {
                            for j in 0..k {
                                out_data[bi*n*k + i*k + j] += data[bi*n*m + i*m + l] * other_data[bi*m*k + l*k + j];
                            }
                        }
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![b,n,k], data: out_data, tid: 0 }))
            }

            // ── Phase 3: N-D reduce along axis ────────────────────────────────
            "reduceSum" => {
                // .reduceSum(axis, keepdim=false) — sum along axis for any N-D tensor
                if dot_call.arguments.is_empty() {
                    return self.rt_err_kind("TypeError", "Tensor.reduceSum(axis) requires at least 1 argument");
                }
                let ax_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref) {
                    Some(ObjectData::Integer(n)) => {
                        let nd = shape.len() as i64;
                        let a = if *n < 0 { nd + n } else { *n };
                        if a < 0 || a >= nd { return self.rt_err_kind("TensorError", format!("reduceSum axis {} out of range", n)); }
                        a as usize
                    }
                    _ => { return self.rt_err_kind("TypeError", "reduceSum axis must be integer"); }
                };
                let keepdim = dot_call.arguments.len() > 1 && matches!(
                    self.eval_expression(&dot_call.arguments[1]),
                    EvalResult::Value(r) if matches!(self.resolve(r), Some(ObjectData::Boolean(true)))
                );
                let (new_shape, out_data) = Self::reduce_along_axis(&data, &shape, axis, keepdim, |a, b| a + b, 0.0);
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data: out_data, tid: 0 }))
            }

            "reduceMean" => {
                if dot_call.arguments.is_empty() {
                    return self.rt_err_kind("TypeError", "Tensor.reduceMean(axis) requires at least 1 argument");
                }
                let ax_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref) {
                    Some(ObjectData::Integer(n)) => {
                        let nd = shape.len() as i64;
                        let a = if *n < 0 { nd + n } else { *n };
                        if a < 0 || a >= nd { return self.rt_err_kind("TensorError", format!("reduceMean axis {} out of range", n)); }
                        a as usize
                    }
                    _ => { return self.rt_err_kind("TypeError", "reduceMean axis must be integer"); }
                };
                let keepdim = dot_call.arguments.len() > 1 && matches!(
                    self.eval_expression(&dot_call.arguments[1]),
                    EvalResult::Value(r) if matches!(self.resolve(r), Some(ObjectData::Boolean(true)))
                );
                let n_reduce = shape[axis] as f64;
                let (new_shape, sum_data) = Self::reduce_along_axis(&data, &shape, axis, keepdim, |a, b| a + b, 0.0);
                let out_data: Vec<f64> = sum_data.iter().map(|x| x / n_reduce).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data: out_data, tid: 0 }))
            }

            "reduceMax" => {
                if dot_call.arguments.is_empty() {
                    return self.rt_err_kind("TypeError", "Tensor.reduceMax(axis) requires at least 1 argument");
                }
                let ax_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref) {
                    Some(ObjectData::Integer(n)) => {
                        let nd = shape.len() as i64;
                        let a = if *n < 0 { nd + n } else { *n };
                        a as usize
                    }
                    _ => { return self.rt_err_kind("TypeError", "reduceMax axis must be integer"); }
                };
                let keepdim = dot_call.arguments.len() > 1 && matches!(
                    self.eval_expression(&dot_call.arguments[1]),
                    EvalResult::Value(r) if matches!(self.resolve(r), Some(ObjectData::Boolean(true)))
                );
                let (new_shape, out_data) = Self::reduce_along_axis(&data, &shape, axis, keepdim, f64::max, f64::NEG_INFINITY);
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data: out_data, tid: 0 }))
            }

            // ── Phase 3: stopGrad (detach from tape) ──────────────────────────
            "stopGrad" | "detach" => {
                // Returns a copy of the tensor not connected to the tape
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data, tid: 0 }))
            }

            // ── Phase 3: Element-wise operations ─────────────────────────────
            "sign" => {
                let new_data: Vec<f64> = data.iter()
                    .map(|&x| if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "round" => {
                let new_data: Vec<f64> = data.iter().map(|x| x.round()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "floor" => {
                let new_data: Vec<f64> = data.iter().map(|x| x.floor()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "ceil" => {
                let new_data: Vec<f64> = data.iter().map(|x| x.ceil()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "reciprocal" => {
                let new_data: Vec<f64> = data.iter().map(|x| 1.0 / x).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "sin" => {
                let new_data: Vec<f64> = data.iter().map(|x| x.sin()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "cos" => {
                let new_data: Vec<f64> = data.iter().map(|x| x.cos()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "maximum" => {
                // .maximum(other) — element-wise max with another tensor
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.maximum(other) requires 1 argument");
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let other_data = match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    Some(ObjectData::Decimal(v)) => vec![v; data.len()],
                    Some(ObjectData::Integer(v)) => vec![v as f64; data.len()],
                    _ => { return self.rt_err_kind("TypeError", "maximum requires Tensor or scalar"); }
                };
                let new_data: Vec<f64> = data.iter().zip(other_data.iter()).map(|(&a,&b)| a.max(b)).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            "minimum" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.minimum(other) requires 1 argument");
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let other_data = match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    Some(ObjectData::Decimal(v)) => vec![v; data.len()],
                    Some(ObjectData::Integer(v)) => vec![v as f64; data.len()],
                    _ => { return self.rt_err_kind("TypeError", "minimum requires Tensor or scalar"); }
                };
                let new_data: Vec<f64> = data.iter().zip(other_data.iter()).map(|(&a,&b)| a.min(b)).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data, tid: 0 }))
            }

            // ── conv2d(weights, bias, kernel, stride) ─────────────────────────
            "conv2d" => {
                if dot_call.arguments.len() != 4 {
                    return self.rt_err_kind("TypeError", "Tensor.conv2d(weights, bias, kernel, stride) requires 4 arguments");
                }
                if shape.len() != 4 {
                    return self.rt_err_kind("TypeError", format!("Tensor.conv2d() input must be 4D [N, H, W, C_in], got {}D", shape.len()));
                }
                let w_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let b_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let k_ref = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, other => return other };
                let s_ref = match self.eval_expression(&dot_call.arguments[3]) { EvalResult::Value(r) => r, other => return other };
                let kernel = match self.resolve(k_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.conv2d() kernel must be a positive integer"); }
                };
                let stride = match self.resolve(s_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.conv2d() stride must be a positive integer"); }
                };
                let (w_shape, w_data) = match self.resolve(w_ref).cloned() {
                    Some(ObjectData::Tensor { shape: ws, data: wd, .. }) => (ws, wd),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.conv2d() weights must be a Tensor"); }
                };
                let b_data = match self.resolve(b_ref).cloned() {
                    Some(ObjectData::Tensor { data: bd, .. }) => bd,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.conv2d() bias must be a Tensor"); }
                };
                if w_shape.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.conv2d() weights must be 2D [kH*kW*C_in, C_out]");
                }
                let (n, h, w_in, c_in) = (shape[0], shape[1], shape[2], shape[3]);
                let col_cols = w_shape[0];
                let c_out = w_shape[1];
                if col_cols != kernel * kernel * c_in {
                    return self.rt_err_kind("TensorError", format!("Tensor.conv2d() weights dim0={} != kernel*kernel*C_in={}", col_cols, kernel*kernel*c_in));
                }
                if h < kernel || w_in < kernel {
                    return self.rt_err_kind("TensorError", format!("Tensor.conv2d() spatial {}x{} < kernel {}", h, w_in, kernel));
                }
                let out_h = (h - kernel) / stride + 1;
                let out_w = (w_in - kernel) / stride + 1;
                let col_rows = n * out_h * out_w;
                // im2col
                let mut col_mat = vec![0.0f64; col_rows * col_cols];
                for nb in 0..n {
                    for oh in 0..out_h {
                        for ow in 0..out_w {
                            let row = nb * out_h * out_w + oh * out_w + ow;
                            for kh in 0..kernel {
                                for kw in 0..kernel {
                                    for ci in 0..c_in {
                                        let ih = oh * stride + kh;
                                        let iw = ow * stride + kw;
                                        let in_idx = nb*h*w_in*c_in + ih*w_in*c_in + iw*c_in + ci;
                                        let col_idx = row*col_cols + kh*kernel*c_in + kw*c_in + ci;
                                        col_mat[col_idx] = data[in_idx];
                                    }
                                }
                            }
                        }
                    }
                }
                // matmul [col_rows, col_cols] @ [col_cols, c_out] → [col_rows, c_out]
                let mut out_2d = vec![0.0f64; col_rows * c_out];
                for i in 0..col_rows {
                    for j in 0..c_out {
                        for l in 0..col_cols {
                            out_2d[i * c_out + j] += col_mat[i * col_cols + l] * w_data[l * c_out + j];
                        }
                    }
                }
                // broadcast add bias
                for i in 0..col_rows {
                    for j in 0..c_out { out_2d[i * c_out + j] += b_data[j]; }
                }
                let out_shape = vec![n, out_h, out_w, c_out];
                let out_ref = self.alloc(ObjectData::Tensor { shape: out_shape.clone(), data: out_2d, tid: 0 });
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    let w_id = self.ad_tensor_id(w_ref);
                    let b_id = self.ad_tensor_id(b_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Conv2d {
                        in_id, w_id, b_id, kernel, stride,
                        in_shape: shape, w_data, col_mat, col_rows, col_cols, c_out,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── max_pool2d(kernel, stride) ────────────────────────────────────
            "max_pool2d" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.max_pool2d(kernel, stride) requires 2 arguments");
                }
                if shape.len() != 4 {
                    return self.rt_err_kind("TypeError", "Tensor.max_pool2d() input must be 4D [N, H, W, C]");
                }
                let k_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let s_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let kernel = match self.resolve(k_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.max_pool2d() kernel must be a positive integer"); }
                };
                let stride = match self.resolve(s_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.max_pool2d() stride must be a positive integer"); }
                };
                let (n, h, w_in, c) = (shape[0], shape[1], shape[2], shape[3]);
                if h < kernel || w_in < kernel {
                    return self.rt_err_kind("TensorError", format!("Tensor.max_pool2d() spatial {}x{} < kernel {}", h, w_in, kernel));
                }
                let out_h = (h - kernel) / stride + 1;
                let out_w = (w_in - kernel) / stride + 1;
                let out_total = n * out_h * out_w * c;
                let mut out_data = vec![f64::NEG_INFINITY; out_total];
                let mut max_indices = vec![0usize; out_total];
                for nb in 0..n {
                    for oh in 0..out_h {
                        for ow in 0..out_w {
                            for ci in 0..c {
                                let out_idx = nb*out_h*out_w*c + oh*out_w*c + ow*c + ci;
                                let mut max_val = f64::NEG_INFINITY;
                                let mut max_idx = 0;
                                for kh in 0..kernel {
                                    for kw in 0..kernel {
                                        let ih = oh * stride + kh;
                                        let iw = ow * stride + kw;
                                        let in_idx = nb*h*w_in*c + ih*w_in*c + iw*c + ci;
                                        if data[in_idx] > max_val {
                                            max_val = data[in_idx];
                                            max_idx = in_idx;
                                        }
                                    }
                                }
                                out_data[out_idx] = max_val;
                                max_indices[out_idx] = max_idx;
                            }
                        }
                    }
                }
                let out_shape = vec![n, out_h, out_w, c];
                let out_ref = self.alloc(ObjectData::Tensor { shape: out_shape.clone(), data: out_data, tid: 0 });
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::MaxPool2d {
                        in_id, kernel, stride, in_shape: shape, out_shape, max_indices,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── layer_norm(gamma, beta, eps) ─────────────────────────────────
            "layer_norm" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Tensor.layer_norm(gamma, beta, eps) requires 3 arguments");
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.layer_norm() input must be 2D [rows, cols]");
                }
                let g_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let b_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let e_ref = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, other => return other };
                let gamma_data = match self.resolve(g_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.layer_norm() gamma must be a Tensor"); }
                };
                let beta_data = match self.resolve(b_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.layer_norm() beta must be a Tensor"); }
                };
                let eps = match self.resolve(e_ref).cloned() {
                    Some(ObjectData::Decimal(d)) => d,
                    Some(ObjectData::Integer(n)) => n as f64,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.layer_norm() eps must be a number"); }
                };
                let (rows, cols) = (shape[0], shape[1]);
                if gamma_data.len() != cols || beta_data.len() != cols {
                    let msg = self.alloc(ObjectData::Str(format!(
                        "❌ Tensor.layer_norm(): gamma and beta must each have {} element(s) (one per column); got gamma={}, beta={}",
                        cols, gamma_data.len(), beta_data.len()
                    )));
                    return EvalResult::Throw(msg);
                }
                let mut out_data = vec![0.0f64; rows * cols];
                let mut x_norm = vec![0.0f64; rows * cols];
                let mut stds   = vec![0.0f64; rows];
                let mut x_mu   = vec![0.0f64; rows * cols];
                for r in 0..rows {
                    let row = &data[r*cols..(r+1)*cols];
                    let mu = row.iter().sum::<f64>() / cols as f64;
                    let var = row.iter().map(|&x| (x-mu)*(x-mu)).sum::<f64>() / cols as f64;
                    let std_v = (var + eps).sqrt();
                    stds[r] = std_v;
                    for c in 0..cols {
                        let xm = row[c] - mu;
                        x_mu[r*cols+c]   = xm;
                        let xn = xm / std_v;
                        x_norm[r*cols+c] = xn;
                        out_data[r*cols+c] = gamma_data[c] * xn + beta_data[c];
                    }
                }
                let out_ref = self.alloc(ObjectData::Tensor { shape: shape.clone(), data: out_data, tid: 0 });
                if self.ad_recording {
                    let in_id = self.ad_tensor_id(tensor_ref);
                    let g_id  = self.ad_tensor_id(g_ref);
                    let b_id  = self.ad_tensor_id(b_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::LayerNorm {
                        in_id, g_id, b_id, eps, x_norm, stds, x_mu, gamma_data, rows, cols,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── mha(Wq, Wk, Wv, Wo, n_heads) — multi-head self-attention ─────
            "mha" => {
                if dot_call.arguments.len() != 5 {
                    return self.rt_err_kind("TypeError", "Tensor.mha(Wq, Wk, Wv, Wo, n_heads) requires 5 arguments");
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.mha() input must be 2D [seq_len, d_model]");
                }
                let (seq_len, d_model) = (shape[0], shape[1]);
                let wq_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let wk_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let wv_ref = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, other => return other };
                let wo_ref = match self.eval_expression(&dot_call.arguments[3]) { EvalResult::Value(r) => r, other => return other };
                let nh_ref = match self.eval_expression(&dot_call.arguments[4]) { EvalResult::Value(r) => r, other => return other };
                let n_heads = match self.resolve(nh_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.mha() n_heads must be a positive integer"); }
                };
                let (wq_shape, wq_data) = match self.resolve(wq_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.mha() Wq must be a Tensor"); }
                };
                let wk_data = match self.resolve(wk_ref).cloned() { Some(ObjectData::Tensor { data: d, .. }) => d, _ => return EvalResult::Error };
                let wv_data = match self.resolve(wv_ref).cloned() { Some(ObjectData::Tensor { data: d, .. }) => d, _ => return EvalResult::Error };
                let wo_data = match self.resolve(wo_ref).cloned() { Some(ObjectData::Tensor { data: d, .. }) => d, _ => return EvalResult::Error };
                if wq_shape.len() != 2 || wq_shape[0] != d_model || wq_shape[1] != d_model || d_model % n_heads != 0 {
                    return self.rt_err_kind("TypeError", "Tensor.mha() Wq/Wk/Wv/Wo must be [d_model, d_model], d_model divisible by n_heads");
                }
                let dh = d_model / n_heads;
                let sl = seq_len;
                let dm = d_model;
                // mm helper: [m,k] @ [k,n] → [m,n]
                let mm = |a: &[f64], m: usize, k: usize, b: &[f64], n: usize| -> Vec<f64> {
                    let mut c = vec![0.0f64; m * n];
                    for i in 0..m { for l in 0..k { if a[i*k+l] == 0.0 { continue; } for j in 0..n { c[i*n+j] += a[i*k+l] * b[l*n+j]; } } }
                    c
                };
                // Projections
                let q_proj = mm(&data, sl, dm, &wq_data, dm);
                let k_proj = mm(&data, sl, dm, &wk_data, dm);
                let v_proj = mm(&data, sl, dm, &wv_data, dm);
                let scale = 1.0 / (dh as f64).sqrt();
                let mut attn_weights = vec![0.0f64; n_heads * sl * sl];
                let mut concat_heads = vec![0.0f64; sl * dm];
                for h in 0..n_heads {
                    // Extract Qh, Kh, Vh [sl, dh]
                    let mut qh = vec![0.0f64; sl * dh];
                    let mut kh = vec![0.0f64; sl * dh];
                    let mut vh = vec![0.0f64; sl * dh];
                    for row in 0..sl {
                        qh[row*dh..row*dh+dh].copy_from_slice(&q_proj[row*dm+h*dh..row*dm+h*dh+dh]);
                        kh[row*dh..row*dh+dh].copy_from_slice(&k_proj[row*dm+h*dh..row*dm+h*dh+dh]);
                        vh[row*dh..row*dh+dh].copy_from_slice(&v_proj[row*dm+h*dh..row*dm+h*dh+dh]);
                    }
                    // scores = Qh @ Kh^T * scale  [sl, sl]
                    let mut scores = vec![0.0f64; sl * sl];
                    for i in 0..sl { for j in 0..sl { for k in 0..dh { scores[i*sl+j] += qh[i*dh+k] * kh[j*dh+k]; } scores[i*sl+j] *= scale; } }
                    // softmax rows
                    let mut attn_h = scores.clone();
                    for row in 0..sl {
                        let mx = attn_h[row*sl..(row+1)*sl].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                        let mut s = 0.0f64;
                        for c in 0..sl { attn_h[row*sl+c] = (attn_h[row*sl+c] - mx).exp(); s += attn_h[row*sl+c]; }
                        for c in 0..sl { attn_h[row*sl+c] /= s; }
                    }
                    attn_weights[h*sl*sl..(h+1)*sl*sl].copy_from_slice(&attn_h);
                    // head = attn_h @ Vh  [sl, dh]
                    let head = mm(&attn_h, sl, sl, &vh, dh);
                    for row in 0..sl { concat_heads[row*dm+h*dh..row*dm+h*dh+dh].copy_from_slice(&head[row*dh..row*dh+dh]); }
                }
                // output = concat @ Wo  [sl, dm]
                let out_data = mm(&concat_heads, sl, dm, &wo_data, dm);
                let out_ref = self.alloc(ObjectData::Tensor { shape: shape.clone(), data: out_data, tid: 0 });
                if self.ad_recording {
                    let x_id  = self.ad_tensor_id(tensor_ref);
                    let wq_id = self.ad_tensor_id(wq_ref);
                    let wk_id = self.ad_tensor_id(wk_ref);
                    let wv_id = self.ad_tensor_id(wv_ref);
                    let wo_id = self.ad_tensor_id(wo_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Mha {
                        x_id, wq_id, wk_id, wv_id, wo_id,
                        x_data: data, wq_data, wk_data, wv_data, wo_data,
                        q_proj, k_proj, v_proj, attn_weights, concat_heads,
                        seq_len, d_model, n_heads, dh,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── one_hot(vocab_size) ───────────────────────────────────────────
            "one_hot" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.one_hot(vocab_size) requires 1 argument");
                }
                if shape.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.one_hot() input must be 1D [seq_len] of indices");
                }
                let v_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let vocab = match self.resolve(v_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.one_hot() vocab_size must be a positive integer"); }
                };
                let seq_len = shape[0];
                let mut out = vec![0.0f64; seq_len * vocab];
                for (i, &val) in data.iter().enumerate() {
                    let idx = val as usize;
                    if idx < vocab { out[i * vocab + idx] = 1.0; }
                }
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![seq_len, vocab], data: out, tid: 0 }))
            }

            // ── lstm(Wx, Wh, b, h0, c0) → [1, hidden_size] last hidden state ─
            "lstm" => {
                if dot_call.arguments.len() != 5 {
                    return self.rt_err_kind("TypeError", "Tensor.lstm(Wx, Wh, b, h0, c0) requires 5 arguments");
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.lstm() input must be 2D [seq_len, input_size]");
                }
                let (seq_len, input_size) = (shape[0], shape[1]);
                let wx_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let wh_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let b_ref  = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, other => return other };
                let h0_ref = match self.eval_expression(&dot_call.arguments[3]) { EvalResult::Value(r) => r, other => return other };
                let c0_ref = match self.eval_expression(&dot_call.arguments[4]) { EvalResult::Value(r) => r, other => return other };
                let (wx_shape, wx_data) = match self.resolve(wx_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.lstm() Wx must be a Tensor"); }
                };
                let (wh_shape, wh_data) = match self.resolve(wh_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.lstm() Wh must be a Tensor"); }
                };
                let b_data = match self.resolve(b_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.lstm() b must be a Tensor"); }
                };
                let h0_data = match self.resolve(h0_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.lstm() h0 must be a Tensor"); }
                };
                let c0_data = match self.resolve(c0_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.lstm() c0 must be a Tensor"); }
                };
                if wx_shape.len() != 2 || wx_shape[0] != input_size || wx_shape[1] % 4 != 0 {
                    return self.rt_err_kind("TypeError", "Tensor.lstm() Wx must be [input_size, 4*hidden_size]");
                }
                let four_h = wx_shape[1];
                let hidden_size = four_h / 4;
                if wh_shape.len() != 2 || wh_shape[0] != hidden_size || wh_shape[1] != four_h {
                    return self.rt_err_kind("TypeError", "Tensor.lstm() Wh must be [hidden_size, 4*hidden_size]");
                }
                if b_data.len() != four_h {
                    return self.rt_err_kind("TensorError", "Tensor.lstm() b must have length 4*hidden_size");
                }
                // Forward pass
                let mut h_all = vec![0.0f64; (seq_len + 1) * hidden_size];
                let mut c_all = vec![0.0f64; (seq_len + 1) * hidden_size];
                let mut gates_i = vec![0.0f64; seq_len * hidden_size];
                let mut gates_f = vec![0.0f64; seq_len * hidden_size];
                let mut gates_g = vec![0.0f64; seq_len * hidden_size];
                let mut gates_o = vec![0.0f64; seq_len * hidden_size];
                for j in 0..hidden_size.min(h0_data.len()) { h_all[j] = h0_data[j]; }
                for j in 0..hidden_size.min(c0_data.len()) { c_all[j] = c0_data[j]; }
                for t in 0..seq_len {
                    let x_t   = &data[t*input_size..(t+1)*input_size];
                    let h_prev: Vec<f64> = h_all[t*hidden_size..(t+1)*hidden_size].to_vec();
                    let c_prev: Vec<f64> = c_all[t*hidden_size..(t+1)*hidden_size].to_vec();
                    let mut z = b_data.clone();
                    for j in 0..four_h {
                        for k in 0..input_size  { z[j] += x_t[k]    * wx_data[k*four_h+j]; }
                        for k in 0..hidden_size { z[j] += h_prev[k] * wh_data[k*four_h+j]; }
                    }
                    let sig = |x: f64| 1.0f64 / (1.0 + (-x).exp());
                    let i_t: Vec<f64> = z[..hidden_size].iter().map(|&x| sig(x)).collect();
                    let f_t: Vec<f64> = z[hidden_size..2*hidden_size].iter().map(|&x| sig(x)).collect();
                    let g_t: Vec<f64> = z[2*hidden_size..3*hidden_size].iter().map(|&x| x.tanh()).collect();
                    let o_t: Vec<f64> = z[3*hidden_size..].iter().map(|&x| sig(x)).collect();
                    let c_t: Vec<f64> = (0..hidden_size).map(|j| f_t[j]*c_prev[j] + i_t[j]*g_t[j]).collect();
                    let h_t: Vec<f64> = (0..hidden_size).map(|j| o_t[j]*c_t[j].tanh()).collect();
                    gates_i[t*hidden_size..(t+1)*hidden_size].copy_from_slice(&i_t);
                    gates_f[t*hidden_size..(t+1)*hidden_size].copy_from_slice(&f_t);
                    gates_g[t*hidden_size..(t+1)*hidden_size].copy_from_slice(&g_t);
                    gates_o[t*hidden_size..(t+1)*hidden_size].copy_from_slice(&o_t);
                    h_all[(t+1)*hidden_size..(t+2)*hidden_size].copy_from_slice(&h_t);
                    c_all[(t+1)*hidden_size..(t+2)*hidden_size].copy_from_slice(&c_t);
                }
                let last_h = h_all[seq_len*hidden_size..].to_vec();
                let out_ref = self.alloc(ObjectData::Tensor { shape: vec![1, hidden_size], data: last_h, tid: 0 });
                if self.ad_recording {
                    let x_id  = self.ad_tensor_id(tensor_ref);
                    let wx_id = self.ad_tensor_id(wx_ref);
                    let wh_id = self.ad_tensor_id(wh_ref);
                    let b_id  = self.ad_tensor_id(b_ref);
                    let h0_id = self.ad_tensor_id(h0_ref);
                    let c0_id = self.ad_tensor_id(c0_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Lstm {
                        x_id, wx_id, wh_id, b_id, h0_id, c0_id,
                        x_data: data, wx_data, wh_data,
                        h_all, c_all, gates_i, gates_f, gates_g, gates_o,
                        seq_len, input_size, hidden_size,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── gru(Wx, Wh, b, h0) → [1, hidden_size] last hidden state ──────
            "gru" => {
                if dot_call.arguments.len() != 4 {
                    return self.rt_err_kind("TypeError", "Tensor.gru(Wx, Wh, b, h0) requires 4 arguments");
                }
                if shape.len() != 2 {
                    return self.rt_err_kind("TypeError", "Tensor.gru() input must be 2D [seq_len, input_size]");
                }
                let (seq_len, input_size) = (shape[0], shape[1]);
                let wx_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let wh_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let b_ref  = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, other => return other };
                let h0_ref = match self.eval_expression(&dot_call.arguments[3]) { EvalResult::Value(r) => r, other => return other };
                let (wx_shape, wx_data) = match self.resolve(wx_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.gru() Wx must be a Tensor"); }
                };
                let (wh_shape, wh_data) = match self.resolve(wh_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { return self.rt_err_kind("TypeError", "Tensor.gru() Wh must be a Tensor"); }
                };
                let b_data = match self.resolve(b_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.gru() b must be a Tensor"); }
                };
                let h0_data = match self.resolve(h0_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { return self.rt_err_kind("TypeError", "Tensor.gru() h0 must be a Tensor"); }
                };
                if wx_shape.len() != 2 || wx_shape[0] != input_size || wx_shape[1] % 3 != 0 {
                    return self.rt_err_kind("TypeError", "Tensor.gru() Wx must be [input_size, 3*hidden_size]");
                }
                let three_h = wx_shape[1];
                let hidden_size = three_h / 3;
                if wh_shape.len() != 2 || wh_shape[0] != hidden_size || wh_shape[1] != three_h {
                    return self.rt_err_kind("TypeError", "Tensor.gru() Wh must be [hidden_size, 3*hidden_size]");
                }
                if b_data.len() != three_h {
                    return self.rt_err_kind("TensorError", "Tensor.gru() b must have length 3*hidden_size");
                }
                let mut h_all = vec![0.0f64; (seq_len + 1) * hidden_size];
                let mut gates_r = vec![0.0f64; seq_len * hidden_size];
                let mut gates_z = vec![0.0f64; seq_len * hidden_size];
                let mut gates_n = vec![0.0f64; seq_len * hidden_size];
                for j in 0..hidden_size.min(h0_data.len()) { h_all[j] = h0_data[j]; }
                let sig = |x: f64| 1.0f64 / (1.0 + (-x).exp());
                for t in 0..seq_len {
                    let x_t   = &data[t*input_size..(t+1)*input_size];
                    let h_prev: Vec<f64> = h_all[t*hidden_size..(t+1)*hidden_size].to_vec();
                    // r gate and z gate: x@Wx[r,z] + h_prev@Wh[r,z] + b[r,z]
                    let mut rz_pre = b_data[..2*hidden_size].to_vec();
                    for j in 0..2*hidden_size {
                        for k in 0..input_size  { rz_pre[j] += x_t[k]    * wx_data[k*three_h+j]; }
                        for k in 0..hidden_size { rz_pre[j] += h_prev[k] * wh_data[k*three_h+j]; }
                    }
                    let r_t: Vec<f64> = rz_pre[..hidden_size].iter().map(|&x| sig(x)).collect();
                    let z_t: Vec<f64> = rz_pre[hidden_size..].iter().map(|&x| sig(x)).collect();
                    // n gate: x@Wx_n + (r*h_prev)@Wh_n + b_n
                    let mut n_pre: Vec<f64> = b_data[2*hidden_size..].to_vec();
                    for j in 0..hidden_size {
                        for k in 0..input_size  { n_pre[j] += x_t[k]              * wx_data[k*three_h+2*hidden_size+j]; }
                        for k in 0..hidden_size { n_pre[j] += r_t[k] * h_prev[k] * wh_data[k*three_h+2*hidden_size+j]; }
                    }
                    let n_t: Vec<f64> = n_pre.iter().map(|&x| x.tanh()).collect();
                    let h_t: Vec<f64> = (0..hidden_size).map(|j| (1.0 - z_t[j]) * h_prev[j] + z_t[j] * n_t[j]).collect();
                    gates_r[t*hidden_size..(t+1)*hidden_size].copy_from_slice(&r_t);
                    gates_z[t*hidden_size..(t+1)*hidden_size].copy_from_slice(&z_t);
                    gates_n[t*hidden_size..(t+1)*hidden_size].copy_from_slice(&n_t);
                    h_all[(t+1)*hidden_size..(t+2)*hidden_size].copy_from_slice(&h_t);
                }
                let last_h = h_all[seq_len*hidden_size..].to_vec();
                let out_ref = self.alloc(ObjectData::Tensor { shape: vec![1, hidden_size], data: last_h, tid: 0 });
                if self.ad_recording {
                    let x_id  = self.ad_tensor_id(tensor_ref);
                    let wx_id = self.ad_tensor_id(wx_ref);
                    let wh_id = self.ad_tensor_id(wh_ref);
                    let b_id  = self.ad_tensor_id(b_ref);
                    let h0_id = self.ad_tensor_id(h0_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::Gru {
                        x_id, wx_id, wh_id, b_id, h0_id,
                        x_data: data, wx_data, wh_data,
                        h_all, gates_r, gates_z, gates_n,
                        seq_len, input_size, hidden_size,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── outer product ─────────────────────────────────────────────────
            "outer" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.outer(other) requires 1 argument");
                }
                if shape.len() != 1 {
                    return self.rt_err_kind("TypeError", "Tensor.outer() requires 1D tensors");
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { shape: os, data: od , ..}) => {
                        if os.len() != 1 {
                            return self.rt_err_kind("TypeError", "Tensor.outer() argument must be a 1D Tensor");
                        }
                        let m = shape[0];
                        let n = os[0];
                        let mut out = Vec::with_capacity(m * n);
                        for i in 0..m { for j in 0..n { out.push(data[i] * od[j]); } }
                        EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![m, n], data: out, tid: 0 }))
                    }
                    _ => { self.rt_err_kind("TypeError", "Tensor.outer() argument must be a Tensor") }
                }
            }

            _ => {
                self.rt_err_kind("TypeError", format!("Unknown Tensor method '{}'", dot_call.method))
            }
        }
    }

    // ── Shared helpers ────────────────────────────────────────────────────────

    pub(super) fn eval_shape_expr(&mut self, expr: &ast::Expression) -> Result<Vec<usize>, EvalResult> {
        let r = match self.eval_expression(expr) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            other => return Err(other),
        };
        match self.resolve(r).cloned() {
            Some(ObjectData::Array { elements, .. }) => {
                let mut shape = Vec::new();
                for e in elements {
                    match e {
                        OwnedValue::Integer(n) if n > 0 => shape.push(n as usize),
                        OwnedValue::Integer(n) => {
                            eprintln!("❌ ERROR: Tensor shape dimensions must be positive, got {}", n);
                            return Err(EvalResult::Error);
                        }
                        _ => {
                            eprintln!("❌ ERROR: Tensor shape must be an array of positive integers");
                            return Err(EvalResult::Error);
                        }
                    }
                }
                if shape.is_empty() {
                    eprintln!("❌ ERROR: Tensor shape cannot be empty");
                    return Err(EvalResult::Error);
                }
                let total: usize = shape.iter().product();
                if total > 10_000_000 {
                    eprintln!("❌ ERROR: Tensor too large: {} elements (max 10M)", total);
                    return Err(EvalResult::Error);
                }
                Ok(shape)
            }
            _ => {
                eprintln!("❌ ERROR: Tensor shape must be an array like [2, 3]");
                Err(EvalResult::Error)
            }
        }
    }

    fn tensor_flat_index(
        &mut self,
        shape: &[usize],
        index_args: &[ast::Expression],
    ) -> Result<usize, EvalResult> {
        if index_args.len() != shape.len() {
            eprintln!(
                "❌ ERROR: Tensor has {} dimension(s) but {} index(es) given",
                shape.len(),
                index_args.len()
            );
            return Err(EvalResult::Error);
        }
        let mut flat = 0usize;
        let mut stride: usize = shape.iter().product();
        for (dim_idx, (dim_size, arg)) in shape.iter().zip(index_args.iter()).enumerate() {
            stride /= dim_size;
            let idx = match self.eval_expression(arg) {
                EvalResult::Value(r) => match self.resolve(r) {
                    Some(ObjectData::Integer(n)) => {
                        let n = *n;
                        if n < 0 || (n as usize) >= *dim_size {
                            eprintln!(
                                "❌ ERROR: Tensor index {} out of bounds for dimension {} (size {})",
                                n, dim_idx, dim_size
                            );
                            return Err(EvalResult::Error);
                        }
                        n as usize
                    }
                    _ => {
                        eprintln!("❌ ERROR: Tensor indices must be integers");
                        return Err(EvalResult::Error);
                    }
                },
                EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
                other => return Err(other),
            };
            flat += idx * stride;
        }
        Ok(flat)
    }

    fn tensor_from_array(&mut self, arr_ref: ObjectRef) -> EvalResult {
        match self.resolve(arr_ref).cloned() {
            Some(ObjectData::Array { elements, .. }) => {
                if elements.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Tensor {
                        shape: vec![0],
                        data: vec![],
                        tid: 0,
                    }));
                }
                // Detect 2D: first element is also an array
                if let OwnedValue::Array { .. } = &elements[0] {
                    let nrows = elements.len();
                    let mut data = Vec::new();
                    let mut ncols = 0usize;
                    for (row_i, row) in elements.iter().enumerate() {
                        match row {
                            OwnedValue::Array { elements: cols, .. } => {
                                if row_i == 0 {
                                    ncols = cols.len();
                                } else if cols.len() != ncols {
                                    return self.rt_err_kind("TensorError", "Tensor.from() — all rows must have the same length");
                                }
                                for c in cols {
                                    match c {
                                        OwnedValue::Integer(n) => data.push(*n as f64),
                                        OwnedValue::Decimal(d) => data.push(*d),
                                        _ => {
                                            return self.rt_err_kind("TypeError", "Tensor.from() — elements must be numbers");
                                        }
                                    }
                                }
                            }
                            _ => {
                                return self.rt_err_kind("TensorError", "Tensor.from() — mixed nesting not allowed");
                            }
                        }
                    }
                    EvalResult::Value(self.alloc(ObjectData::Tensor {
                        shape: vec![nrows, ncols],
                        data,
                        tid: 0,
                    }))
                } else {
                    // 1D
                    let mut data = Vec::new();
                    for e in &elements {
                        match e {
                            OwnedValue::Integer(n) => data.push(*n as f64),
                            OwnedValue::Decimal(d) => data.push(*d),
                            _ => {
                                return self.rt_err_kind("TypeError", "Tensor.from() — elements must be numbers");
                            }
                        }
                    }
                    let len = data.len();
                    EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![len], data , tid: 0}))
                }
            }
            _ => {
                self.rt_err_kind("TypeError", "Tensor.from() requires an array argument")
            }
        }
    }

    fn tensor_elementwise(
        &mut self,
        tensor_ref: ObjectRef,
        shape: Vec<usize>,
        data: Vec<f64>,
        dot_call: &ast::DotCallExpression,
        op: &str,
    ) -> EvalResult {
        if dot_call.arguments.len() != 1 {
            return self.rt_err_kind("TypeError", format!("Tensor.{}() requires 1 argument", op));
        }
        let arg_ref = match self.eval_expression(&dot_call.arguments[0]) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        let is_tensor_arg = matches!(self.resolve(arg_ref), Some(ObjectData::Tensor { .. }));
        // For mul tape recording, save both input data before consuming them
        let saved_a = if self.ad_recording && is_tensor_arg && op == "mul" { Some(data.clone()) } else { None };
        let saved_b = if self.ad_recording && is_tensor_arg && op == "mul" {
            match self.resolve(arg_ref) {
                Some(ObjectData::Tensor { data: d2, .. }) => Some(d2.clone()),
                _ => None,
            }
        } else { None };

        let result_data = match self.resolve(arg_ref).cloned() {
            Some(ObjectData::Tensor { shape: s2, data: d2 , ..}) => {
                if s2 != shape {
                    return self.rt_err_kind("TensorError", format!("Tensor.{}() shape mismatch: {:?} vs {:?}", op, shape, s2));
                }
                data.iter().zip(d2.iter()).map(|(a, b)| match op {
                    "add" => a + b,
                    "sub" => a - b,
                    "mul" => a * b,
                    "div" => if *b == 0.0 { f64::NAN } else { a / b },
                    _ => unreachable!(),
                }).collect()
            }
            Some(ObjectData::Integer(n)) => {
                let s = n as f64;
                data.iter().map(|a| match op {
                    "add" => a + s,
                    "sub" => a - s,
                    "mul" => a * s,
                    "div" => if s == 0.0 { f64::NAN } else { a / s },
                    _ => unreachable!(),
                }).collect()
            }
            Some(ObjectData::Decimal(d)) => {
                data.iter().map(|a| match op {
                    "add" => a + d,
                    "sub" => a - d,
                    "mul" => a * d,
                    "div" => if d == 0.0 { f64::NAN } else { a / d },
                    _ => unreachable!(),
                }).collect()
            }
            _ => {
                return self.rt_err_kind("TypeError", format!("Tensor.{}() requires a Tensor or numeric argument", op));
            }
        };
        let out_ref = self.alloc(ObjectData::Tensor { shape, data: result_data , tid: 0});
        if self.ad_recording && is_tensor_arg {
            let a_id = self.ad_tensor_id(tensor_ref);
            let b_id = self.ad_tensor_id(arg_ref);
            let tape_op = match op {
                "add" => crate::evaluator::namespaces_autodiff::TapeOp::Add { a_id, b_id },
                "sub" => crate::evaluator::namespaces_autodiff::TapeOp::Sub { a_id, b_id },
                "mul" => crate::evaluator::namespaces_autodiff::TapeOp::Mul {
                    a_id, b_id,
                    a_data: saved_a.unwrap_or_default(),
                    b_data: saved_b.unwrap_or_default(),
                },
                _ => { return EvalResult::Value(out_ref); }
            };
            self.ad_push(out_ref, tape_op);
        }
        EvalResult::Value(out_ref)
    }

    // ── Phase 3: Static helpers for N-D broadcasting & reduction ─────────────

    /// Compute broadcast output shape (numpy semantics).
    /// Returns None if shapes are incompatible.
    pub(super) fn broadcast_shape(a: &[usize], b: &[usize]) -> Option<Vec<usize>> {
        let ndim = a.len().max(b.len());
        let mut out = vec![0usize; ndim];
        for i in 0..ndim {
            let ai = if i < ndim - a.len() { 1 } else { a[i - (ndim - a.len())] };
            let bi = if i < ndim - b.len() { 1 } else { b[i - (ndim - b.len())] };
            if ai == bi { out[i] = ai; }
            else if ai == 1 { out[i] = bi; }
            else if bi == 1 { out[i] = ai; }
            else { return None; }
        }
        Some(out)
    }

    /// Broadcast `data` from `from_shape` to `to_shape`.
    /// Returns None if incompatible.
    pub(super) fn broadcast_data(data: &[f64], from_shape: &[usize], to_shape: &[usize]) -> Option<Vec<f64>> {
        let ndim = to_shape.len();
        if from_shape.len() > ndim { return None; }
        // Pad from_shape on the left with 1s
        let pad = ndim - from_shape.len();
        let padded: Vec<usize> = (0..pad).map(|_| 1).chain(from_shape.iter().cloned()).collect();
        // Verify compatible
        for i in 0..ndim {
            if padded[i] != to_shape[i] && padded[i] != 1 { return None; }
        }
        let out_len: usize = to_shape.iter().product();
        let mut out = vec![0.0_f64; out_len];
        // Compute strides for padded_shape
        let mut strides = vec![0usize; ndim];
        let mut s = 1usize;
        for i in (0..ndim).rev() {
            strides[i] = if padded[i] == 1 { 0 } else { s };
            if padded[i] != 1 { s *= padded[i]; }
        }
        // Compute strides for to_shape (for iterating output)
        let mut out_strides = vec![1usize; ndim];
        for i in (0..ndim - 1).rev() { out_strides[i] = out_strides[i+1] * to_shape[i+1]; }
        // Fill output
        for flat_out in 0..out_len {
            let mut rem = flat_out;
            let mut src_flat = 0usize;
            for i in 0..ndim {
                let idx = rem / out_strides[i];
                rem %= out_strides[i];
                src_flat += idx * strides[i];
            }
            out[flat_out] = data[src_flat];
        }
        Some(out)
    }

    /// Generic reduce along a single axis. `keepdim=true` inserts a size-1 dim.
    pub(super) fn reduce_along_axis(
        data: &[f64],
        shape: &[usize],
        axis: usize,
        keepdim: bool,
        op: impl Fn(f64, f64) -> f64,
        init: f64,
    ) -> (Vec<usize>, Vec<f64>) {
        let ndim = shape.len();
        // Output shape: remove or keep-dim the axis
        let mut out_shape: Vec<usize> = shape.iter().cloned().enumerate()
            .filter_map(|(i, d)| if i == axis { if keepdim { Some(1) } else { None } } else { Some(d) })
            .collect();
        if out_shape.is_empty() { out_shape = vec![1]; }
        let out_len: usize = out_shape.iter().product();
        let mut out_data = vec![init; out_len];
        // Strides for input
        let mut in_strides = vec![1usize; ndim];
        for i in (0..ndim - 1).rev() { in_strides[i] = in_strides[i+1] * shape[i+1]; }
        // Strides for output (using out_shape without the axis dim)
        let out_shape_no_ax: Vec<usize> = shape.iter().cloned().enumerate()
            .filter_map(|(i, d)| if i == axis { None } else { Some(d) }).collect();
        let mut out_strides_no_ax = vec![1usize; out_shape_no_ax.len()];
        if !out_shape_no_ax.is_empty() {
            for i in (0..out_shape_no_ax.len() - 1).rev() {
                out_strides_no_ax[i] = out_strides_no_ax[i+1] * out_shape_no_ax[i+1];
            }
        }
        // Iterate all input elements
        for flat_in in 0..data.len() {
            // Decode multi-index
            let mut rem = flat_in;
            let mut out_flat = 0usize;
            let mut out_ax = 0usize;
            for i in 0..ndim {
                let idx = rem / in_strides[i];
                rem %= in_strides[i];
                if i == axis { out_ax = idx; }
                else {
                    let ax_no = if i < axis { i } else { i - 1 };
                    if ax_no < out_strides_no_ax.len() { out_flat += idx * out_strides_no_ax[ax_no]; }
                }
            }
            let _ = out_ax;
            out_data[out_flat] = op(out_data[out_flat], data[flat_in]);
        }
        (out_shape, out_data)
    }
}
