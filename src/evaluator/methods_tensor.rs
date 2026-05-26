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
        EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![fill; total] }))
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![0.0; total] }))
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![1.0; total] }))
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![n, n], data }))
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
                let new_obj = ObjectData::Tensor { shape, data: new_data };
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: new_shape, data }))
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![cols, rows], data: new_data }))
            }
            "add" => self.tensor_elementwise(shape, data, dot_call, "add"),
            "sub" => self.tensor_elementwise(shape, data, dot_call, "sub"),
            "mul" => self.tensor_elementwise(shape, data, dot_call, "mul"),
            "div" => self.tensor_elementwise(shape, data, dot_call, "div"),
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
                    Some(ObjectData::Tensor { shape: s2, data: d2 }) => {
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
                    Some(ObjectData::Tensor { shape: s2, data: d2 }) => {
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
                        EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![m, n], data: result }))
                    }
                    _ => {
                        eprintln!("❌ ERROR: Tensor.matmul() requires a 2D Tensor argument");
                        EvalResult::Error
                    }
                }
            }
            "sum" => {
                let s: f64 = data.iter().sum();
                EvalResult::Value(self.alloc(ObjectData::Decimal(s)))
            }
            "mean" => {
                if data.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Decimal(0.0)));
                }
                let m = data.iter().sum::<f64>() / data.len() as f64;
                EvalResult::Value(self.alloc(ObjectData::Decimal(m)))
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![len], data }))
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: vec![val; data.len()] }))
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
                let new_data: Vec<f64> = data.iter().map(|&x| if x > 0.0 { x } else { 0.0 }).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
            }
            "sigmoid" => {
                let new_data: Vec<f64> = data.iter().map(|&x| 1.0 / (1.0 + (-x).exp())).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
            }
            "tanh" => {
                let new_data: Vec<f64> = data.iter().map(|&x| x.tanh()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
            }
            "softmax" => {
                if data.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data }));
                }
                let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let exps: Vec<f64> = data.iter().map(|&x| (x - max).exp()).collect();
                let sum: f64 = exps.iter().sum();
                let new_data: Vec<f64> = exps.iter().map(|&e| e / sum).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
            }

            // ── Element-wise math ────────────────────────────────────────────
            "abs" => {
                let new_data: Vec<f64> = data.iter().map(|&x| x.abs()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
            }
            "sqrt" => {
                for &x in &data {
                    if x < 0.0 { eprintln!("❌ ERROR: Tensor.sqrt() — negative value {}", x); return EvalResult::Error; }
                }
                let new_data: Vec<f64> = data.iter().map(|&x| x.sqrt()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
            }
            "exp" => {
                let new_data: Vec<f64> = data.iter().map(|&x| x.exp()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
            }
            "log" => {
                for &x in &data {
                    if x <= 0.0 { eprintln!("❌ ERROR: Tensor.log() — non-positive value {}", x); return EvalResult::Error; }
                }
                let new_data: Vec<f64> = data.iter().map(|&x| x.ln()).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
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
                    Some(ObjectData::Tensor { shape: bs, data: bd }) => {
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
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: new_data }))
            }

            _ => {
                eprintln!("❌ ERROR: Unknown Tensor method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── Shared helpers ────────────────────────────────────────────────────────

    fn eval_shape_expr(&mut self, expr: &ast::Expression) -> Result<Vec<usize>, EvalResult> {
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
                    EvalResult::Value(self.alloc(ObjectData::Tensor { shape: vec![len], data }))
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
        let result_data = match self.resolve(arg_ref).cloned() {
            Some(ObjectData::Tensor { shape: s2, data: d2 }) => {
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
        EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data: result_data }))
    }
}
