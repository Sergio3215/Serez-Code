use crate::ast;
use crate::region::{ObjectData, ObjectRef, RegionId};
use super::EvalResult;

impl super::Evaluator {
    // ── Constructor: new Tensor([rows, cols], fill) ───────────────────────────
    pub(super) fn eval_new_tensor(&mut self, new_expr: &ast::NewExpression) -> EvalResult {
        let args = match &new_expr.args {
            ast::NewArgs::Positional(a) => a.clone(),
            ast::NewArgs::Fields(_) => {
                eprintln!("❌ ERROR: Tensor constructor requires positional arguments: new Tensor([shape], fill)");
                return EvalResult::Error;
            }
        };
        if args.is_empty() {
            eprintln!("❌ ERROR: Tensor() requires at least a shape argument like [2, 3]");
            return EvalResult::Error;
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
                    _ => { eprintln!("❌ ERROR: Tensor fill value must be a number"); return EvalResult::Error; }
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
                    eprintln!("❌ ERROR: Tensor.zeros([shape]) requires 1 argument");
                    return EvalResult::Error;
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                let total: usize = shape.iter().product();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![0.0; total] , tid: 0}))
            }
            "ones" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.ones([shape]) requires 1 argument");
                    return EvalResult::Error;
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                let total: usize = shape.iter().product();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![1.0; total] , tid: 0}))
            }
            "eye" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.eye(n) requires 1 argument");
                    return EvalResult::Error;
                }
                let n = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) if *n > 0 => *n as usize,
                        _ => { eprintln!("❌ ERROR: Tensor.eye(n) requires a positive integer"); return EvalResult::Error; }
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
                    eprintln!("❌ ERROR: Tensor.from(array) requires 1 argument");
                    return EvalResult::Error;
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                self.tensor_from_array(arr_ref)
            }
            _ => {
                eprintln!("❌ ERROR: Unknown Tensor static method '{}'", dot_call.method);
                EvalResult::Error
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
                let refs: Vec<ObjectRef> = shape.iter()
                    .map(|&d| self.alloc(ObjectData::Integer(d as i64)))
                    .collect();
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: refs,
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
                    eprintln!("❌ ERROR: Tensor.set() requires index arg(s) + value, e.g. set(0, 1, val)");
                    return EvalResult::Error;
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
                        _ => { eprintln!("❌ ERROR: Tensor.set() value must be a number"); return EvalResult::Error; }
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
                    eprintln!("❌ ERROR: Tensor.reshape([shape]) requires 1 argument");
                    return EvalResult::Error;
                }
                let new_shape = match self.eval_shape_expr(&dot_call.arguments[0].clone()) {
                    Ok(s) => s, Err(e) => return e,
                };
                let new_total: usize = new_shape.iter().product();
                if new_total != data.len() {
                    eprintln!("❌ ERROR: Tensor.reshape() — shape has {} elements but tensor has {}", new_total, data.len());
                    return EvalResult::Error;
                }
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data , tid: 0}))
            }
            "transpose" => {
                if shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.transpose() only supported for 2D tensors");
                    return EvalResult::Error;
                }
                let (rows, cols) = (shape[0], shape[1]);
                let mut new_data = vec![0.0f64; data.len()];
                for r in 0..rows {
                    for c in 0..cols {
                        new_data[c * rows + r] = data[r * cols + c];
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![cols, rows], data: new_data, tid: 0 }))
            }
            "add" => self.tensor_elementwise(tensor_ref, shape, data, dot_call, "add"),
            "sub" => self.tensor_elementwise(tensor_ref, shape, data, dot_call, "sub"),
            "mul" => self.tensor_elementwise(tensor_ref, shape, data, dot_call, "mul"),
            "div" => self.tensor_elementwise(tensor_ref, shape, data, dot_call, "div"),
            "dot" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.dot() requires 1 argument");
                    return EvalResult::Error;
                }
                let arg_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.resolve(arg_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s2, data: d2 , ..}) => {
                        if shape.len() != 1 || s2.len() != 1 {
                            eprintln!("❌ ERROR: Tensor.dot() only works on 1D tensors (use matmul for 2D)");
                            return EvalResult::Error;
                        }
                        if shape[0] != s2[0] {
                            eprintln!("❌ ERROR: Tensor.dot() length mismatch: {} vs {}", shape[0], s2[0]);
                            return EvalResult::Error;
                        }
                        let result: f64 = data.iter().zip(d2.iter()).map(|(a, b)| a * b).sum();
                        EvalResult::Value(self.alloc(ObjectData::Decimal(result)))
                    }
                    _ => {
                        eprintln!("❌ ERROR: Tensor.dot() requires a 1D Tensor argument");
                        EvalResult::Error
                    }
                }
            }
            "matmul" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.matmul() requires 1 argument");
                    return EvalResult::Error;
                }
                let arg_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.resolve(arg_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s2, data: d2 , ..}) => {
                        if shape.len() != 2 || s2.len() != 2 {
                            eprintln!("❌ ERROR: Tensor.matmul() requires 2D tensors");
                            return EvalResult::Error;
                        }
                        let (m, k, k2, n) = (shape[0], shape[1], s2[0], s2[1]);
                        if k != k2 {
                            eprintln!("❌ ERROR: Tensor.matmul() inner dimensions must match: {} != {}", k, k2);
                            return EvalResult::Error;
                        }
                        let mut result = vec![0.0f64; m * n];
                        for i in 0..m {
                            for j in 0..n {
                                let mut acc = 0.0f64;
                                for l in 0..k { acc += data[i * k + l] * d2[l * n + j]; }
                                result[i * n + j] = acc;
                            }
                        }
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
                        eprintln!("❌ ERROR: Tensor.matmul() requires a 2D Tensor argument");
                        EvalResult::Error
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
                    eprintln!("❌ ERROR: Tensor.max() on empty tensor");
                    return EvalResult::Error;
                }
                let v = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                EvalResult::Value(self.alloc(ObjectData::Decimal(v)))
            }
            "min" => {
                if data.is_empty() {
                    eprintln!("❌ ERROR: Tensor.min() on empty tensor");
                    return EvalResult::Error;
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
                    eprintln!("❌ ERROR: Tensor.fill(val) requires 1 argument");
                    return EvalResult::Error;
                }
                let val = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { eprintln!("❌ ERROR: Tensor.fill() value must be a number"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![val; data.len()] , tid: 0}))
            }
            "toArray" => {
                if shape.len() == 1 {
                    let refs: Vec<ObjectRef> = data.iter()
                        .map(|&x| self.alloc(ObjectData::Decimal(x)))
                        .collect();
                    EvalResult::Value(self.alloc(ObjectData::Array {
                        element_type: Some("decimal".to_string()),
                        elements: refs,
                    }))
                } else if shape.len() == 2 {
                    let (rows, cols) = (shape[0], shape[1]);
                    let row_refs: Vec<ObjectRef> = (0..rows).map(|r| {
                        let col_refs: Vec<ObjectRef> = (0..cols)
                            .map(|c| self.alloc(ObjectData::Decimal(data[r * cols + c])))
                            .collect();
                        self.alloc(ObjectData::Array {
                            element_type: Some("decimal".to_string()),
                            elements: col_refs,
                        })
                    }).collect();
                    EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: row_refs }))
                } else {
                    // Higher dims: return flat
                    let refs: Vec<ObjectRef> = data.iter()
                        .map(|&x| self.alloc(ObjectData::Decimal(x)))
                        .collect();
                    EvalResult::Value(self.alloc(ObjectData::Array {
                        element_type: Some("decimal".to_string()),
                        elements: refs,
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
                if data.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data, tid: 0 }));
                }
                let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let exps: Vec<f64> = data.iter().map(|&x| (x - max).exp()).collect();
                let sum: f64 = exps.iter().sum();
                let new_data: Vec<f64> = exps.iter().map(|&e| e / sum).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }

            // ── Element-wise math ────────────────────────────────────────────
            "abs" => {
                let new_data: Vec<f64> = data.iter().map(|&x| x.abs()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }
            "sqrt" => {
                for &x in &data {
                    if x < 0.0 { eprintln!("❌ ERROR: Tensor.sqrt() — negative value {}", x); return EvalResult::Error; }
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
                    if x <= 0.0 { eprintln!("❌ ERROR: Tensor.log() — non-positive value {}", x); return EvalResult::Error; }
                }
                let new_data: Vec<f64> = data.iter().map(|&x| x.ln()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }
            "pow" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.pow(exponent) requires 1 argument");
                    return EvalResult::Error;
                }
                let exp = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { eprintln!("❌ ERROR: Tensor.pow() exponent must be a number"); return EvalResult::Error; }
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
                            _ => { eprintln!("❌ ERROR: Tensor.norm() order must be an integer"); return EvalResult::Error; }
                        },
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                };
                let result = match order {
                    1 => data.iter().map(|&x| x.abs()).sum::<f64>(),
                    2 => data.iter().map(|&x| x * x).sum::<f64>().sqrt(),
                    _ => { eprintln!("❌ ERROR: Tensor.norm() supports order 1 or 2, got {}", order); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(ObjectData::Decimal(result)))
            }

            // ── Clamp ────────────────────────────────────────────────────────
            "clamp" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.clamp(min, max) requires 2 arguments");
                    return EvalResult::Error;
                }
                let lo = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { eprintln!("❌ ERROR: Tensor.clamp() min must be a number"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let hi = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => match self.resolve(r) {
                        Some(ObjectData::Integer(n)) => *n as f64,
                        Some(ObjectData::Decimal(d)) => *d,
                        _ => { eprintln!("❌ ERROR: Tensor.clamp() max must be a number"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                if lo > hi { eprintln!("❌ ERROR: Tensor.clamp() min > max"); return EvalResult::Error; }
                let new_data: Vec<f64> = data.iter().map(|&x| x.clamp(lo, hi)).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }

            // ── Broadcast add: (m,n) + (n,) ─────────────────────────────────
            "broadcastAdd" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.broadcastAdd(bias) requires 1 argument");
                    return EvalResult::Error;
                }
                if shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.broadcastAdd() only supported for 2D tensors");
                    return EvalResult::Error;
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
                            eprintln!("❌ ERROR: Tensor.broadcastAdd() bias shape {:?} must match last dim {}", bs, cols);
                            return EvalResult::Error;
                        }
                        bd
                    }
                    _ => { eprintln!("❌ ERROR: Tensor.broadcastAdd() argument must be a 1D Tensor"); return EvalResult::Error; }
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
                    eprintln!("❌ ERROR: Tensor.{}() requires 1 argument", dot_call.method);
                    return EvalResult::Error;
                }
                if shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.{}() only supported for 2D tensors", dot_call.method);
                    return EvalResult::Error;
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
                            eprintln!("❌ ERROR: Tensor.{}() rhs shape {:?} must match last dim {}", dot_call.method, rs, cols);
                            return EvalResult::Error;
                        }
                        rd
                    }
                    _ => { eprintln!("❌ ERROR: Tensor.{}() argument must be a 1D Tensor", dot_call.method); return EvalResult::Error; }
                };
                if dot_call.method == "broadcastDiv" {
                    for v in &rhs_data {
                        if *v == 0.0 {
                            eprintln!("❌ ERROR: Tensor.broadcastDiv() division by zero in rhs");
                            return EvalResult::Error;
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
                    let mat_id = self.ad_tensor_id(tensor_ref);
                    let rhs_id = self.ad_tensor_id(rhs_ref);
                    self.ad_push(out_ref, crate::evaluator::namespaces_autodiff::TapeOp::BroadcastMul { mat_id, rhs_id, rows, cols });
                }
                EvalResult::Value(out_ref)
            }

            // ── sum(axis) / mean(axis) ────────────────────────────────────────
            "sumAxis" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.sumAxis(axis) requires 1 argument");
                    return EvalResult::Error;
                }
                if shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.sumAxis() only supported for 2D tensors");
                    return EvalResult::Error;
                }
                let (rows, cols) = (shape[0], shape[1]);
                let ax_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n == 0 || n == 1 => n as usize,
                    _ => { eprintln!("❌ ERROR: Tensor.sumAxis() axis must be 0 or 1"); return EvalResult::Error; }
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
                    eprintln!("❌ ERROR: Tensor.meanAxis(axis) requires 1 argument");
                    return EvalResult::Error;
                }
                if shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.meanAxis() only supported for 2D tensors");
                    return EvalResult::Error;
                }
                let (rows, cols) = (shape[0], shape[1]);
                let ax_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n == 0 || n == 1 => n as usize,
                    _ => { eprintln!("❌ ERROR: Tensor.meanAxis() axis must be 0 or 1"); return EvalResult::Error; }
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
                    eprintln!("❌ ERROR: Tensor.argmax() on empty tensor");
                    return EvalResult::Error;
                }
                let idx = data.iter().enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i).unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(idx as i64)))
            }

            "argmin" => {
                if data.is_empty() {
                    eprintln!("❌ ERROR: Tensor.argmin() on empty tensor");
                    return EvalResult::Error;
                }
                let idx = data.iter().enumerate()
                    .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i).unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(idx as i64)))
            }

            // ── slice(start, end) — flat index range ──────────────────────────
            "slice" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.slice(start, end) requires 2 arguments");
                    return EvalResult::Error;
                }
                let r0 = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let r1 = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let (start, end) = match (self.resolve(r0).cloned(), self.resolve(r1).cloned()) {
                    (Some(ObjectData::Integer(a)), Some(ObjectData::Integer(b))) => (a as usize, b as usize),
                    _ => { eprintln!("❌ ERROR: Tensor.slice() arguments must be integers"); return EvalResult::Error; }
                };
                if start > end || end > data.len() {
                    eprintln!("❌ ERROR: Tensor.slice() invalid range {}..{} for tensor of length {}", start, end, data.len());
                    return EvalResult::Error;
                }
                let sliced = data[start..end].to_vec();
                let len = end - start;
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![len], data: sliced, tid: 0 }))
            }

            // ── concat(other, axis) — 2D row/col concatenation ────────────────
            "concat" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.concat(other, axis) requires 2 arguments");
                    return EvalResult::Error;
                }
                if shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.concat() only supported for 2D tensors");
                    return EvalResult::Error;
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let ax_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let axis = match self.resolve(ax_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n == 0 || n == 1 => n as usize,
                    _ => { eprintln!("❌ ERROR: Tensor.concat() axis must be 0 or 1"); return EvalResult::Error; }
                };
                let (rows, cols) = (shape[0], shape[1]);
                match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { shape: os, data: od , ..}) => {
                        if axis == 0 {
                            if os.len() != 2 || os[1] != cols {
                                eprintln!("❌ ERROR: Tensor.concat(axis=0) column mismatch: {} vs {}", os.get(1).copied().unwrap_or(0), cols);
                                return EvalResult::Error;
                            }
                            let mut out = data.clone();
                            out.extend_from_slice(&od);
                            EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![rows + os[0], cols], data: out , tid: 0}))
                        } else {
                            if os.len() != 2 || os[0] != rows {
                                eprintln!("❌ ERROR: Tensor.concat(axis=1) row mismatch: {} vs {}", os.get(0).copied().unwrap_or(0), rows);
                                return EvalResult::Error;
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
                    _ => { eprintln!("❌ ERROR: Tensor.concat() argument must be a Tensor"); EvalResult::Error }
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
                    eprintln!("❌ ERROR: Tensor.scale(s) requires 1 argument");
                    return EvalResult::Error;
                }
                let s_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let s = match self.resolve(s_ref).cloned() {
                    Some(ObjectData::Integer(n)) => n as f64,
                    Some(ObjectData::Decimal(d)) => d,
                    _ => { eprintln!("❌ ERROR: Tensor.scale() argument must be a number"); return EvalResult::Error; }
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
                    eprintln!("❌ ERROR: Tensor.leaky_relu(alpha) requires 1 argument");
                    return EvalResult::Error;
                }
                let a_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let alpha = match self.resolve(a_ref).cloned() {
                    Some(ObjectData::Integer(n)) => n as f64,
                    Some(ObjectData::Decimal(d)) => d,
                    _ => { eprintln!("❌ ERROR: Tensor.leaky_relu() alpha must be a number"); return EvalResult::Error; }
                };
                let new_data: Vec<f64> = data.iter().map(|&x| if x >= 0.0 { x } else { alpha * x }).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }

            "gelu" => {
                // Gaussian Error Linear Unit: x * Φ(x) approximated via tanh
                let sqrt2_over_pi = (2.0f64 / std::f64::consts::PI).sqrt();
                let new_data: Vec<f64> = data.iter().map(|&x| {
                    0.5 * x * (1.0 + (sqrt2_over_pi * (x + 0.044715 * x * x * x)).tanh())
                }).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data , tid: 0}))
            }

            // ── conv2d(weights, bias, kernel, stride) ─────────────────────────
            "conv2d" => {
                if dot_call.arguments.len() != 4 {
                    eprintln!("❌ ERROR: Tensor.conv2d(weights, bias, kernel, stride) requires 4 arguments");
                    return EvalResult::Error;
                }
                if shape.len() != 4 {
                    eprintln!("❌ ERROR: Tensor.conv2d() input must be 4D [N, H, W, C_in], got {}D", shape.len());
                    return EvalResult::Error;
                }
                let w_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let b_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let k_ref = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, other => return other };
                let s_ref = match self.eval_expression(&dot_call.arguments[3]) { EvalResult::Value(r) => r, other => return other };
                let kernel = match self.resolve(k_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { eprintln!("❌ ERROR: Tensor.conv2d() kernel must be a positive integer"); return EvalResult::Error; }
                };
                let stride = match self.resolve(s_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { eprintln!("❌ ERROR: Tensor.conv2d() stride must be a positive integer"); return EvalResult::Error; }
                };
                let (w_shape, w_data) = match self.resolve(w_ref).cloned() {
                    Some(ObjectData::Tensor { shape: ws, data: wd, .. }) => (ws, wd),
                    _ => { eprintln!("❌ ERROR: Tensor.conv2d() weights must be a Tensor"); return EvalResult::Error; }
                };
                let b_data = match self.resolve(b_ref).cloned() {
                    Some(ObjectData::Tensor { data: bd, .. }) => bd,
                    _ => { eprintln!("❌ ERROR: Tensor.conv2d() bias must be a Tensor"); return EvalResult::Error; }
                };
                if w_shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.conv2d() weights must be 2D [kH*kW*C_in, C_out]");
                    return EvalResult::Error;
                }
                let (n, h, w_in, c_in) = (shape[0], shape[1], shape[2], shape[3]);
                let col_cols = w_shape[0];
                let c_out = w_shape[1];
                if col_cols != kernel * kernel * c_in {
                    eprintln!("❌ ERROR: Tensor.conv2d() weights dim0={} != kernel*kernel*C_in={}", col_cols, kernel*kernel*c_in);
                    return EvalResult::Error;
                }
                if h < kernel || w_in < kernel {
                    eprintln!("❌ ERROR: Tensor.conv2d() spatial {}x{} < kernel {}", h, w_in, kernel);
                    return EvalResult::Error;
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
                    eprintln!("❌ ERROR: Tensor.max_pool2d(kernel, stride) requires 2 arguments");
                    return EvalResult::Error;
                }
                if shape.len() != 4 {
                    eprintln!("❌ ERROR: Tensor.max_pool2d() input must be 4D [N, H, W, C]");
                    return EvalResult::Error;
                }
                let k_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let s_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let kernel = match self.resolve(k_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { eprintln!("❌ ERROR: Tensor.max_pool2d() kernel must be a positive integer"); return EvalResult::Error; }
                };
                let stride = match self.resolve(s_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { eprintln!("❌ ERROR: Tensor.max_pool2d() stride must be a positive integer"); return EvalResult::Error; }
                };
                let (n, h, w_in, c) = (shape[0], shape[1], shape[2], shape[3]);
                if h < kernel || w_in < kernel {
                    eprintln!("❌ ERROR: Tensor.max_pool2d() spatial {}x{} < kernel {}", h, w_in, kernel);
                    return EvalResult::Error;
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

            // ── one_hot(vocab_size) ───────────────────────────────────────────
            "one_hot" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.one_hot(vocab_size) requires 1 argument");
                    return EvalResult::Error;
                }
                if shape.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.one_hot() input must be 1D [seq_len] of indices");
                    return EvalResult::Error;
                }
                let v_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let vocab = match self.resolve(v_ref).cloned() {
                    Some(ObjectData::Integer(n)) if n > 0 => n as usize,
                    _ => { eprintln!("❌ ERROR: Tensor.one_hot() vocab_size must be a positive integer"); return EvalResult::Error; }
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
                    eprintln!("❌ ERROR: Tensor.lstm(Wx, Wh, b, h0, c0) requires 5 arguments");
                    return EvalResult::Error;
                }
                if shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.lstm() input must be 2D [seq_len, input_size]");
                    return EvalResult::Error;
                }
                let (seq_len, input_size) = (shape[0], shape[1]);
                let wx_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let wh_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let b_ref  = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, other => return other };
                let h0_ref = match self.eval_expression(&dot_call.arguments[3]) { EvalResult::Value(r) => r, other => return other };
                let c0_ref = match self.eval_expression(&dot_call.arguments[4]) { EvalResult::Value(r) => r, other => return other };
                let (wx_shape, wx_data) = match self.resolve(wx_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { eprintln!("❌ ERROR: Tensor.lstm() Wx must be a Tensor"); return EvalResult::Error; }
                };
                let (wh_shape, wh_data) = match self.resolve(wh_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { eprintln!("❌ ERROR: Tensor.lstm() Wh must be a Tensor"); return EvalResult::Error; }
                };
                let b_data = match self.resolve(b_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { eprintln!("❌ ERROR: Tensor.lstm() b must be a Tensor"); return EvalResult::Error; }
                };
                let h0_data = match self.resolve(h0_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { eprintln!("❌ ERROR: Tensor.lstm() h0 must be a Tensor"); return EvalResult::Error; }
                };
                let c0_data = match self.resolve(c0_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { eprintln!("❌ ERROR: Tensor.lstm() c0 must be a Tensor"); return EvalResult::Error; }
                };
                if wx_shape.len() != 2 || wx_shape[0] != input_size || wx_shape[1] % 4 != 0 {
                    eprintln!("❌ ERROR: Tensor.lstm() Wx must be [input_size, 4*hidden_size]"); return EvalResult::Error;
                }
                let four_h = wx_shape[1];
                let hidden_size = four_h / 4;
                if wh_shape.len() != 2 || wh_shape[0] != hidden_size || wh_shape[1] != four_h {
                    eprintln!("❌ ERROR: Tensor.lstm() Wh must be [hidden_size, 4*hidden_size]"); return EvalResult::Error;
                }
                if b_data.len() != four_h {
                    eprintln!("❌ ERROR: Tensor.lstm() b must have length 4*hidden_size"); return EvalResult::Error;
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
                    eprintln!("❌ ERROR: Tensor.gru(Wx, Wh, b, h0) requires 4 arguments");
                    return EvalResult::Error;
                }
                if shape.len() != 2 {
                    eprintln!("❌ ERROR: Tensor.gru() input must be 2D [seq_len, input_size]");
                    return EvalResult::Error;
                }
                let (seq_len, input_size) = (shape[0], shape[1]);
                let wx_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let wh_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let b_ref  = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, other => return other };
                let h0_ref = match self.eval_expression(&dot_call.arguments[3]) { EvalResult::Value(r) => r, other => return other };
                let (wx_shape, wx_data) = match self.resolve(wx_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { eprintln!("❌ ERROR: Tensor.gru() Wx must be a Tensor"); return EvalResult::Error; }
                };
                let (wh_shape, wh_data) = match self.resolve(wh_ref).cloned() {
                    Some(ObjectData::Tensor { shape: s, data: d, .. }) => (s, d),
                    _ => { eprintln!("❌ ERROR: Tensor.gru() Wh must be a Tensor"); return EvalResult::Error; }
                };
                let b_data = match self.resolve(b_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { eprintln!("❌ ERROR: Tensor.gru() b must be a Tensor"); return EvalResult::Error; }
                };
                let h0_data = match self.resolve(h0_ref).cloned() {
                    Some(ObjectData::Tensor { data: d, .. }) => d,
                    _ => { eprintln!("❌ ERROR: Tensor.gru() h0 must be a Tensor"); return EvalResult::Error; }
                };
                if wx_shape.len() != 2 || wx_shape[0] != input_size || wx_shape[1] % 3 != 0 {
                    eprintln!("❌ ERROR: Tensor.gru() Wx must be [input_size, 3*hidden_size]"); return EvalResult::Error;
                }
                let three_h = wx_shape[1];
                let hidden_size = three_h / 3;
                if wh_shape.len() != 2 || wh_shape[0] != hidden_size || wh_shape[1] != three_h {
                    eprintln!("❌ ERROR: Tensor.gru() Wh must be [hidden_size, 3*hidden_size]"); return EvalResult::Error;
                }
                if b_data.len() != three_h {
                    eprintln!("❌ ERROR: Tensor.gru() b must have length 3*hidden_size"); return EvalResult::Error;
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
                    eprintln!("❌ ERROR: Tensor.outer(other) requires 1 argument");
                    return EvalResult::Error;
                }
                if shape.len() != 1 {
                    eprintln!("❌ ERROR: Tensor.outer() requires 1D tensors");
                    return EvalResult::Error;
                }
                let other_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                match self.resolve(other_ref).cloned() {
                    Some(ObjectData::Tensor { shape: os, data: od , ..}) => {
                        if os.len() != 1 {
                            eprintln!("❌ ERROR: Tensor.outer() argument must be a 1D Tensor");
                            return EvalResult::Error;
                        }
                        let m = shape[0];
                        let n = os[0];
                        let mut out = Vec::with_capacity(m * n);
                        for i in 0..m { for j in 0..n { out.push(data[i] * od[j]); } }
                        EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![m, n], data: out, tid: 0 }))
                    }
                    _ => { eprintln!("❌ ERROR: Tensor.outer() argument must be a Tensor"); EvalResult::Error }
                }
            }

            _ => {
                eprintln!("❌ ERROR: Unknown Tensor method '{}'", dot_call.method);
                EvalResult::Error
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
                    match self.resolve(e).cloned() {
                        Some(ObjectData::Integer(n)) if n > 0 => shape.push(n as usize),
                        Some(ObjectData::Integer(n)) => {
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
                let first = self.resolve(elements[0]).cloned();
                if let Some(ObjectData::Array { .. }) = first {
                    let nrows = elements.len();
                    let mut data = Vec::new();
                    let mut ncols = 0usize;
                    for (row_i, row_ref) in elements.iter().enumerate() {
                        match self.resolve(*row_ref).cloned() {
                            Some(ObjectData::Array { elements: cols, .. }) => {
                                if row_i == 0 {
                                    ncols = cols.len();
                                } else if cols.len() != ncols {
                                    eprintln!("❌ ERROR: Tensor.from() — all rows must have the same length");
                                    return EvalResult::Error;
                                }
                                for c in cols {
                                    match self.resolve(c) {
                                        Some(ObjectData::Integer(n)) => data.push(*n as f64),
                                        Some(ObjectData::Decimal(d)) => data.push(*d),
                                        _ => {
                                            eprintln!("❌ ERROR: Tensor.from() — elements must be numbers");
                                            return EvalResult::Error;
                                        }
                                    }
                                }
                            }
                            _ => {
                                eprintln!("❌ ERROR: Tensor.from() — mixed nesting not allowed");
                                return EvalResult::Error;
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
                        match self.resolve(*e) {
                            Some(ObjectData::Integer(n)) => data.push(*n as f64),
                            Some(ObjectData::Decimal(d)) => data.push(*d),
                            _ => {
                                eprintln!("❌ ERROR: Tensor.from() — elements must be numbers");
                                return EvalResult::Error;
                            }
                        }
                    }
                    let len = data.len();
                    EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![len], data , tid: 0}))
                }
            }
            _ => {
                eprintln!("❌ ERROR: Tensor.from() requires an array argument");
                EvalResult::Error
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
            eprintln!("❌ ERROR: Tensor.{}() requires 1 argument", op);
            return EvalResult::Error;
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
                    eprintln!("❌ ERROR: Tensor.{}() shape mismatch: {:?} vs {:?}", op, shape, s2);
                    return EvalResult::Error;
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
                eprintln!("❌ ERROR: Tensor.{}() requires a Tensor or numeric argument", op);
                return EvalResult::Error;
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
}
