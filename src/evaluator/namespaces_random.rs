use super::ExecutionFlow;
// Random namespace
//
// Random.seed(n)                          → null      (set LCG seed)
// Random.decimal()                        → decimal   [0, 1)
// Random.int(min, max)                    → int       [min, max]
// Random.uniform(lo, hi)                  → decimal   [lo, hi)
// Random.normal(mean, std)                → decimal   N(mean, std)
// Random.normalTensor([shape], mean, std) → Tensor    each element ~ N(mean, std)
// Random.uniformTensor([shape], lo, hi)   → Tensor    each element ~ U[lo, hi)
// Random.shuffle(array)                   → [any]     Fisher-Yates copy
// Random.choice(array)                    → any       random element
// Random.bernoulli(p)                     → bool      true with probability p

use super::EvalResult;
use crate::ast;
use crate::region::ObjectData;

impl super::Evaluator {
    pub(super) fn eval_random_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "seed" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Random.seed(n) requires 1 argument");
                }
                let r = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                match self.resolve(r).cloned() {
                    Some(ObjectData::Integer(n)) => {
                        self.lcg_state = n as u64;
                        Ok(ExecutionFlow::Value(self.null_ref))
                    }
                    _ => self.rt_err_kind("TypeError", "Random.seed requires an integer"),
                }
            }

            "decimal" => {
                if !dot_call.arguments.is_empty() {
                    return self.rt_err_kind("TypeError", "Random.decimal() takes no arguments");
                }
                let v = self.lcg_next_f64();
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(v))))
            }

            "int" => {
                if dot_call.arguments.len() != 2 {
                    return self
                        .rt_err_kind("TypeError", "Random.int(min, max) requires 2 arguments");
                }
                let r0 = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let r1 = match self.eval_expression(&dot_call.arguments[1]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                match (self.resolve(r0).cloned(), self.resolve(r1).cloned()) {
                    (Some(ObjectData::Integer(lo)), Some(ObjectData::Integer(hi))) => {
                        if lo > hi {
                            return self.rt_err_kind(
                                "RangeError",
                                format!("Random.int: min ({lo}) must be <= max ({hi})"),
                            );
                        }
                        // Compute the inclusive width outside i64 so the complete
                        // [i64::MIN, i64::MAX] domain is representable.
                        let width = (hi as i128 - lo as i128 + 1) as u128;
                        let offset = self.lcg_bounded_offset(width);
                        let v = (lo as i128 + offset as i128) as i64;
                        Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(v))))
                    }
                    _ => self.rt_err_kind("TypeError", "Random.int requires integer arguments"),
                }
            }

            "uniform" => {
                if dot_call.arguments.len() != 2 {
                    return self
                        .rt_err_kind("TypeError", "Random.uniform(lo, hi) requires 2 arguments");
                }
                let r0 = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let r1 = match self.eval_expression(&dot_call.arguments[1]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let lo = match self.resolve(r0) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Random.uniform: lo must be a number");
                    }
                };
                let hi = match self.resolve(r1) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Random.uniform: hi must be a number");
                    }
                };
                if !lo.is_finite() || !hi.is_finite() {
                    return self.rt_err_kind("RangeError", "Random.uniform: bounds must be finite");
                }
                if lo >= hi {
                    return self.rt_err_kind("RangeError", "Random.uniform: lo must be < hi");
                }
                let v = lo + self.lcg_next_f64() * (hi - lo);
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(v))))
            }

            "normal" => {
                if dot_call.arguments.len() != 2 {
                    return self
                        .rt_err_kind("TypeError", "Random.normal(mean, std) requires 2 arguments");
                }
                let r0 = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let r1 = match self.eval_expression(&dot_call.arguments[1]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let mean = match self.resolve(r0) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Random.normal: mean must be a number");
                    }
                };
                let std = match self.resolve(r1) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Random.normal: std must be a number");
                    }
                };
                if !mean.is_finite() || !std.is_finite() {
                    return self
                        .rt_err_kind("RangeError", "Random.normal: mean and std must be finite");
                }
                if std < 0.0 {
                    return self
                        .rt_err_kind("RangeError", "Random.normal: std must be non-negative");
                }
                let v = self.lcg_normal(mean, std);
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(v))))
            }

            "normalTensor" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind(
                        "TypeError",
                        "Random.normalTensor([shape], mean, std) requires 3 arguments",
                    );
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0].clone()) {
                    Ok(s) => s,
                    Err(e) => return e,
                };
                let total: usize = shape.iter().product();
                let r1 = match self.eval_expression(&dot_call.arguments[1]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let r2 = match self.eval_expression(&dot_call.arguments[2]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let mean = match self.resolve(r1) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self.rt_err_kind(
                            "TypeError",
                            "Random.normalTensor: mean must be a number",
                        );
                    }
                };
                let std = match self.resolve(r2) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Random.normalTensor: std must be a number");
                    }
                };
                if !mean.is_finite() || !std.is_finite() {
                    return self.rt_err_kind(
                        "RangeError",
                        "Random.normalTensor: mean and std must be finite",
                    );
                }
                if std < 0.0 {
                    return self.rt_err_kind(
                        "RangeError",
                        "Random.normalTensor: std must be non-negative",
                    );
                }
                let data: Vec<f64> = (0..total).map(|_| self.lcg_normal(mean, std)).collect();
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Tensor {
                    shape,
                    data,
                    tid: 0,
                })))
            }

            "uniformTensor" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind(
                        "TypeError",
                        "Random.uniformTensor([shape], lo, hi) requires 3 arguments",
                    );
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0].clone()) {
                    Ok(s) => s,
                    Err(e) => return e,
                };
                let total: usize = shape.iter().product();
                let r1 = match self.eval_expression(&dot_call.arguments[1]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let r2 = match self.eval_expression(&dot_call.arguments[2]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let lo = match self.resolve(r1) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Random.uniformTensor: lo must be a number");
                    }
                };
                let hi = match self.resolve(r2) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Random.uniformTensor: hi must be a number");
                    }
                };
                if !lo.is_finite() || !hi.is_finite() {
                    return self
                        .rt_err_kind("RangeError", "Random.uniformTensor: bounds must be finite");
                }
                if lo >= hi {
                    return self.rt_err_kind("RangeError", "Random.uniformTensor: lo must be < hi");
                }
                let range = hi - lo;
                let data: Vec<f64> = (0..total)
                    .map(|_| lo + self.lcg_next_f64() * range)
                    .collect();
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Tensor {
                    shape,
                    data,
                    tid: 0,
                })))
            }

            "shuffle" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Random.shuffle(array) requires 1 argument");
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                match self.resolve(arr_ref).cloned() {
                    Some(ObjectData::Array {
                        element_type,
                        elements,
                    }) => {
                        let mut elems = elements;
                        let n = elems.len();
                        for i in (1..n).rev() {
                            let j = (self.lcg_next_u64() % (i as u64 + 1)) as usize;
                            elems.swap(i, j);
                        }
                        Ok(ExecutionFlow::Value(self.alloc(ObjectData::Array {
                            element_type,
                            elements: elems,
                        })))
                    }
                    _ => self.rt_err_kind("TypeError", "Random.shuffle requires an array"),
                }
            }

            "choice" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Random.choice(array) requires 1 argument");
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                match self.resolve(arr_ref).cloned() {
                    Some(ObjectData::Array { elements, .. }) => {
                        if elements.is_empty() {
                            return self.rt_err_kind("RangeError", "Random.choice: array is empty");
                        }
                        let idx = (self.lcg_next_u64() % elements.len() as u64) as usize;
                        Ok(ExecutionFlow::Value(self.plant(elements[idx].clone())))
                    }
                    _ => self.rt_err_kind("TypeError", "Random.choice requires an array"),
                }
            }

            "bernoulli" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Random.bernoulli(p) requires 1 argument");
                }
                let r = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(v)) => v,
                    other => return other,
                };
                let p = match self.resolve(r) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => {
                        return self.rt_err_kind(
                            "TypeError",
                            "Random.bernoulli: p must be a number in [0, 1]",
                        );
                    }
                };
                if !(0.0..=1.0).contains(&p) {
                    return self.rt_err_kind(
                        "RangeError",
                        format!("Random.bernoulli: p must be in [0, 1], got {p}"),
                    );
                }
                let b = self.lcg_next_f64() < p;
                Ok(ExecutionFlow::Value(self.bool_ref(b)))
            }

            _ => self.rt_err_kind(
                "ReferenceError",
                format!("Unknown Random method '{}'", dot_call.method),
            ),
        }
    }

    // ── LCG helpers (shared with Math.random) ────────────────────────────────────

    #[inline]
    pub(super) fn lcg_next_u64(&mut self) -> u64 {
        self.lcg_state = self
            .lcg_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.lcg_state >> 33
    }

    #[inline]
    pub(super) fn lcg_next_f64(&mut self) -> f64 {
        self.lcg_next_u64() as f64 / (1u64 << 31) as f64
    }

    /// Draw an offset in `[0, width)` without overflowing at the full i64
    /// domain. Widths supported by the historical 31-bit output retain their
    /// exact seeded sequence; wider ranges combine three draws so every i64
    /// value is reachable.
    fn lcg_bounded_offset(&mut self, width: u128) -> u64 {
        debug_assert!((1..=(1u128 << 64)).contains(&width));

        if width <= (1u128 << 31) {
            return self.lcg_next_u64() % width as u64;
        }

        if width == (1u128 << 64) {
            return self.lcg_wide_u64();
        }

        let bound = width as u64;
        // Rejection sampling avoids modulo bias for the newly supported wide
        // domain. It does not affect the established small-range stream above.
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let sample = self.lcg_wide_u64();
            if sample >= threshold {
                return sample % bound;
            }
        }
    }

    #[inline]
    fn lcg_wide_u64(&mut self) -> u64 {
        let high = self.lcg_next_u64();
        let middle = self.lcg_next_u64();
        let low = self.lcg_next_u64();
        (high << 33) | (middle << 2) | (low & 0b11)
    }

    // Box-Muller transform — produces one N(mean, std) sample
    pub(super) fn lcg_normal(&mut self, mean: f64, std: f64) -> f64 {
        let u1 = (self.lcg_next_f64() + 1e-12).min(1.0 - 1e-12);
        let u2 = self.lcg_next_f64();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        mean + std * z
    }
}
