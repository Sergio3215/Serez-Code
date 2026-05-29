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

use crate::ast;
use crate::region::ObjectData;
use super::EvalResult;

impl super::Evaluator {
    pub(super) fn eval_random_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {

            "seed" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Random.seed(n) requires 1 argument");
                    return EvalResult::Error;
                }
                let r = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v, other => return other,
                };
                match self.resolve(r).cloned() {
                    Some(ObjectData::Integer(n)) => {
                        self.lcg_state = n as u64;
                        EvalResult::Value(self.null_ref)
                    }
                    _ => { eprintln!("❌ ERROR: Random.seed requires an integer"); EvalResult::Error }
                }
            }

            "decimal" => {
                if !dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: Random.decimal() takes no arguments");
                    return EvalResult::Error;
                }
                let v = self.lcg_next_f64();
                EvalResult::Value(self.alloc(ObjectData::Decimal(v)))
            }

            "int" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Random.int(min, max) requires 2 arguments");
                    return EvalResult::Error;
                }
                let r0 = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, other => return other };
                let r1 = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(v) => v, other => return other };
                match (self.resolve(r0).cloned(), self.resolve(r1).cloned()) {
                    (Some(ObjectData::Integer(lo)), Some(ObjectData::Integer(hi))) => {
                        if lo > hi {
                            eprintln!("❌ ERROR: Random.int: min ({}) > max ({})", lo, hi);
                            return EvalResult::Error;
                        }
                        let range = (hi - lo + 1) as u64;
                        let v = lo + (self.lcg_next_u64() % range) as i64;
                        EvalResult::Value(self.alloc(ObjectData::Integer(v)))
                    }
                    _ => { eprintln!("❌ ERROR: Random.int requires integer arguments"); EvalResult::Error }
                }
            }

            "uniform" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Random.uniform(lo, hi) requires 2 arguments");
                    return EvalResult::Error;
                }
                let r0 = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, other => return other };
                let r1 = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(v) => v, other => return other };
                let lo = match self.resolve(r0) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.uniform: lo must be a number"); return EvalResult::Error; }
                };
                let hi = match self.resolve(r1) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.uniform: hi must be a number"); return EvalResult::Error; }
                };
                if lo >= hi {
                    eprintln!("❌ ERROR: Random.uniform: lo must be < hi");
                    return EvalResult::Error;
                }
                let v = lo + self.lcg_next_f64() * (hi - lo);
                EvalResult::Value(self.alloc(ObjectData::Decimal(v)))
            }

            "normal" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Random.normal(mean, std) requires 2 arguments");
                    return EvalResult::Error;
                }
                let r0 = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, other => return other };
                let r1 = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(v) => v, other => return other };
                let mean = match self.resolve(r0) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.normal: mean must be a number"); return EvalResult::Error; }
                };
                let std = match self.resolve(r1) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.normal: std must be a number"); return EvalResult::Error; }
                };
                if std < 0.0 {
                    eprintln!("❌ ERROR: Random.normal: std must be non-negative");
                    return EvalResult::Error;
                }
                let v = self.lcg_normal(mean, std);
                EvalResult::Value(self.alloc(ObjectData::Decimal(v)))
            }

            "normalTensor" => {
                if dot_call.arguments.len() != 3 {
                    eprintln!("❌ ERROR: Random.normalTensor([shape], mean, std) requires 3 arguments");
                    return EvalResult::Error;
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0].clone()) {
                    Ok(s) => s, Err(e) => return e,
                };
                let total: usize = shape.iter().product();
                let r1 = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(v) => v, other => return other };
                let r2 = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(v) => v, other => return other };
                let mean = match self.resolve(r1) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.normalTensor: mean must be a number"); return EvalResult::Error; }
                };
                let std = match self.resolve(r2) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.normalTensor: std must be a number"); return EvalResult::Error; }
                };
                if std < 0.0 {
                    eprintln!("❌ ERROR: Random.normalTensor: std must be non-negative");
                    return EvalResult::Error;
                }
                let data: Vec<f64> = (0..total).map(|_| self.lcg_normal(mean, std)).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data, tid: 0 }))
            }

            "uniformTensor" => {
                if dot_call.arguments.len() != 3 {
                    eprintln!("❌ ERROR: Random.uniformTensor([shape], lo, hi) requires 3 arguments");
                    return EvalResult::Error;
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0].clone()) {
                    Ok(s) => s, Err(e) => return e,
                };
                let total: usize = shape.iter().product();
                let r1 = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(v) => v, other => return other };
                let r2 = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(v) => v, other => return other };
                let lo = match self.resolve(r1) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.uniformTensor: lo must be a number"); return EvalResult::Error; }
                };
                let hi = match self.resolve(r2) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.uniformTensor: hi must be a number"); return EvalResult::Error; }
                };
                if lo >= hi {
                    eprintln!("❌ ERROR: Random.uniformTensor: lo must be < hi");
                    return EvalResult::Error;
                }
                let range = hi - lo;
                let data: Vec<f64> = (0..total).map(|_| lo + self.lcg_next_f64() * range).collect();
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data, tid: 0 }))
            }

            "shuffle" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Random.shuffle(array) requires 1 argument");
                    return EvalResult::Error;
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v, other => return other,
                };
                match self.resolve(arr_ref).cloned() {
                    Some(ObjectData::Array { element_type, elements }) => {
                        let mut elems = elements;
                        let n = elems.len();
                        for i in (1..n).rev() {
                            let j = (self.lcg_next_u64() % (i as u64 + 1)) as usize;
                            elems.swap(i, j);
                        }
                        EvalResult::Value(self.alloc(ObjectData::Array { element_type, elements: elems }))
                    }
                    _ => { eprintln!("❌ ERROR: Random.shuffle requires an array"); EvalResult::Error }
                }
            }

            "choice" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Random.choice(array) requires 1 argument");
                    return EvalResult::Error;
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v, other => return other,
                };
                match self.resolve(arr_ref).cloned() {
                    Some(ObjectData::Array { elements, .. }) => {
                        if elements.is_empty() {
                            eprintln!("❌ ERROR: Random.choice: array is empty");
                            return EvalResult::Error;
                        }
                        let idx = (self.lcg_next_u64() % elements.len() as u64) as usize;
                        EvalResult::Value(self.plant(elements[idx].clone()))
                    }
                    _ => { eprintln!("❌ ERROR: Random.choice requires an array"); EvalResult::Error }
                }
            }

            "bernoulli" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Random.bernoulli(p) requires 1 argument");
                    return EvalResult::Error;
                }
                let r = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v, other => return other,
                };
                let p = match self.resolve(r) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Random.bernoulli: p must be a number in [0, 1]"); return EvalResult::Error; }
                };
                if !(0.0..=1.0).contains(&p) {
                    eprintln!("❌ ERROR: Random.bernoulli: p must be in [0, 1], got {}", p);
                    return EvalResult::Error;
                }
                let b = self.lcg_next_f64() < p;
                EvalResult::Value(self.bool_ref(b))
            }

            _ => {
                eprintln!("❌ ERROR: Unknown Random method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── LCG helpers (shared with Math.random) ────────────────────────────────────

    #[inline]
    pub(super) fn lcg_next_u64(&mut self) -> u64 {
        self.lcg_state = self.lcg_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.lcg_state >> 33
    }

    #[inline]
    pub(super) fn lcg_next_f64(&mut self) -> f64 {
        self.lcg_next_u64() as f64 / (1u64 << 31) as f64
    }

    // Box-Muller transform — produces one N(mean, std) sample
    pub(super) fn lcg_normal(&mut self, mean: f64, std: f64) -> f64 {
        let u1 = (self.lcg_next_f64() + 1e-12).min(1.0 - 1e-12);
        let u2 = self.lcg_next_f64();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        mean + std * z
    }
}
