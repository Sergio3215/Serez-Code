mod stmt;
mod expr;
mod ops;
mod check;
mod builtins;
mod classes;
mod methods_array;
mod methods_string;
mod control;
mod namespaces;
mod methods_set;
mod methods_tensor;
mod namespaces_crypto;
mod namespaces_socket;
mod namespaces_binary;
mod namespaces_gpu;
mod namespaces_memory;
mod namespaces_random;
mod namespaces_autodiff;
mod namespaces_os;

use crate::ast::{self, Program, Statement};
use crate::region::{Arena, ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Clone)]
struct StoredClass {
    parent: Option<String>,
    constructor: Option<ast::ClassConstructor>,
    /// Non-getter, non-setter methods (includes static). O(1) lookup by name.
    methods: HashMap<String, ast::ClassMethod>,
    /// Static-only methods (subset of methods). O(1) static dispatch.
    static_methods: HashMap<String, ast::ClassMethod>,
    /// Getter methods keyed by property name.
    getters: HashMap<String, ast::ClassMethod>,
    /// Setter methods keyed by property name.
    setters: HashMap<String, ast::ClassMethod>,
    is_abstract: bool,
    #[allow(dead_code)]
    is_sealed: bool,
    fields: Vec<ast::ClassField>,
}

// ── EvalResult ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EvalResult {
    Value(ObjectRef),       // Ejecución normal (retorno implícito)
    Return(ObjectRef),      // Ejecución interrumpida por `return`
    Break,                  // Señal de break — capturada por while/for
    Continue,               // Señal de continue — capturada por while/for
    BreakLabel(String),     // Señal de break con label
    ContinueLabel(String),  // Señal de continue con label
    Error,                  // Ocurrió un error
    Throw(ObjectRef),       // Excepción de usuario — propagada hasta try/catch
}

// ── Evaluator ─────────────────────────────────────────────────────────────────
struct CallFrame {
    name: String,
    line: usize,
    column: usize,
}
pub struct Evaluator {
    global_arena: Arena,
    global_bindings: HashMap<String, ObjectRef>,
    scopes: ScopeStack,
    null_ref: ObjectRef,
    true_ref: ObjectRef,
    false_ref: ObjectRef,
    // Cache for small integers [0..256] — index i maps to integer i
    int_cache: [ObjectRef; 257],
    call_stack: Vec<CallFrame>,
    interface_registry: HashMap<String, Vec<ast::InterfaceField>>,
    class_registry: HashMap<String, StoredClass>,
    constructing_class: Option<String>,
    executing_class: Option<String>,
    call_depth: usize,
    const_names: HashSet<String>,
    enum_registry: HashMap<String, Vec<String>>,
    sealed_classes: HashSet<String>,
    // LCG state for Math.random()
    lcg_state: u64,
    source_lines: Vec<String>,
    // true while executing inside an unsafe { } block
    in_unsafe_block: bool,
    // registered native function names
    native_fns: HashSet<String>,
    // set of already-imported canonical paths (prevents re-import and cycles)
    imported_files: HashSet<PathBuf>,
    // the directory of the currently executing file (for relative import resolution)
    current_dir: Option<PathBuf>,
    // Some(set) while executing an imported module — tracks exported names.
    // None at top level (main file) or when no export statements were used yet.
    current_module_exports: Option<HashSet<String>>,
    // Collects yielded values while executing a generator function body.
    // None = not inside a generator; Some(vec) = collecting yields.
    yield_collector: Option<Vec<OwnedValue>>,
    // Socket registry: maps socket IDs to live TCP streams
    socket_registry: HashMap<i64, std::net::TcpStream>,
    // Listener registry: maps listener IDs to bound TCP listeners
    listener_registry: HashMap<i64, std::net::TcpListener>,
    // Monotonically increasing ID counter for sockets and listeners
    socket_next_id: i64,
    // GPU buffer registry: maps buffer IDs to flat f64 data (CPU-backed)
    gpu_buffers: HashMap<i64, Vec<f64>>,
    // Monotonically increasing ID counter for GPU buffers
    gpu_next_id: i64,
    // Raw memory heap: maps allocation IDs to byte arrays
    memory_heap: HashMap<i64, Vec<u8>>,
    // Monotonically increasing ID counter for memory allocations
    memory_heap_next_id: i64,
    // Granted permissions: populated from serez.json + `use permissions { }` blocks
    permissions: HashSet<String>,
    // ── Autodiff tape ─────────────────────────────────────────────────────────
    ad_recording: bool,
    ad_tape: Vec<namespaces_autodiff::TapeEntry>,
    ad_grads: HashMap<u64, Vec<f64>>,
    ad_next_id: u64,
    // Maps stable tensor tid → tape_node_id for the current recording session
    ad_tensor_ids: HashMap<u64, u64>,
    // Monotonically increasing counter for stable tensor identity (tid)
    tensor_id_counter: u64,
}

impl Evaluator {
    pub fn new() -> Self {
        let mut global_arena = Arena::new();
        let null_idx = global_arena.alloc(ObjectData::Null);
        let null_ref = ObjectRef { region: RegionId::Global, index: null_idx };

        let true_idx  = global_arena.alloc(ObjectData::Boolean(true));
        let true_ref  = ObjectRef { region: RegionId::Global, index: true_idx };
        let false_idx = global_arena.alloc(ObjectData::Boolean(false));
        let false_ref = ObjectRef { region: RegionId::Global, index: false_idx };

        // Pre-allocate integers 0..=256 in the global arena
        let mut int_cache = [null_ref; 257];
        for i in 0usize..=256 {
            let idx = global_arena.alloc(ObjectData::Integer(i as i64));
            int_cache[i] = ObjectRef { region: RegionId::Global, index: idx };
        }

        // Seed LCG with current time
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(12345);

        Evaluator {
            global_arena,
            global_bindings: HashMap::new(),
            scopes: ScopeStack::new(),
            null_ref,
            true_ref,
            false_ref,
            int_cache,
            call_stack: Vec::new(),
            interface_registry: HashMap::new(),
            class_registry: HashMap::new(),
            constructing_class: None,
            executing_class: None,
            call_depth: 0,
            const_names: HashSet::new(),
            enum_registry: HashMap::new(),
            sealed_classes: HashSet::new(),
            lcg_state: seed,
            source_lines: Vec::new(),
            in_unsafe_block: false,
            native_fns: HashSet::new(),
            imported_files: HashSet::new(),
            current_dir: None,
            current_module_exports: None,
            yield_collector: None,
            socket_registry: HashMap::new(),
            listener_registry: HashMap::new(),
            socket_next_id: 1,
            gpu_buffers: HashMap::new(),
            gpu_next_id: 1,
            memory_heap: HashMap::new(),
            memory_heap_next_id: 1,
            permissions: HashSet::new(),
            ad_recording: false,
            ad_tape: Vec::new(),
            ad_grads: HashMap::new(),
            ad_next_id: 1,
            ad_tensor_ids: HashMap::new(),
            tensor_id_counter: 1,
        }
    }

    pub fn set_permissions(&mut self, perms: Vec<String>) {
        for p in perms {
            self.permissions.insert(p);
        }
    }

    pub fn set_source(&mut self, lines: Vec<String>) {
        self.source_lines = lines;
    }

    pub fn set_current_file(&mut self, path: &std::path::Path) {
        if let Some(dir) = path.parent() {
            self.current_dir = Some(dir.to_path_buf());
        }
        if let Ok(canonical) = path.canonicalize() {
            self.imported_files.insert(canonical);
        }
    }

    #[inline(always)]
    fn bool_ref(&self, b: bool) -> ObjectRef {
        if b { self.true_ref } else { self.false_ref }
    }

    #[inline(always)]
    fn int_ref(&mut self, i: i64) -> ObjectRef {
        if i >= 0 && i <= 256 {
            self.int_cache[i as usize]
        } else {
            self.alloc(ObjectData::Integer(i))
        }
    }

    fn print_call_stack(&self) {
        for frame in self.call_stack.iter().rev() {
            eprintln!("    called from '{}' [line {}:{}]", frame.name, frame.line, frame.column);
            if let Some(src) = self.source_lines.get(frame.line.saturating_sub(1)) {
                let ln = frame.line.to_string();
                eprintln!("    {} | {}", ln, src.trim_end());
                eprintln!("    {}   {}^", " ".repeat(ln.len()), " ".repeat(frame.column.saturating_sub(1)));
            }
        }
        eprintln!();
    }

    fn alloc(&mut self, data: ObjectData) -> ObjectRef {
        // Auto-assign stable tid to new tensors: tid==0 is the sentinel for "unassigned".
        let data = if let ObjectData::Tensor { shape, data: d, tid: 0 } = data {
            let tid = self.tensor_id_counter;
            self.tensor_id_counter += 1;
            ObjectData::Tensor { shape, data: d, tid }
        } else {
            data
        };
        if self.scopes.is_empty() {
            let idx = self.global_arena.alloc(data);
            ObjectRef { region: RegionId::Global, index: idx }
        } else {
            let idx = self.scopes.arena.alloc(data);
            ObjectRef { region: RegionId::Scoped, index: idx }
        }
    }

    // Allocate a new Tensor — tid is auto-assigned by alloc().
    pub(super) fn alloc_tensor(&mut self, shape: Vec<usize>, data: Vec<f64>) -> ObjectRef {
        self.alloc(ObjectData::Tensor { shape, data, tid: 0 })
    }

    pub fn resolve(&self, obj_ref: ObjectRef) -> Option<&ObjectData> {
        match obj_ref.region {
            RegionId::Global => self.global_arena.get(obj_ref.index),
            RegionId::Scoped => self.scopes.arena.get(obj_ref.index),
        }
    }

    fn lookup_var(&self, name: &str) -> Option<ObjectRef> {
        if let Some(r) = self.scopes.lookup(name) {
            return Some(r);
        }
        self.global_bindings.get(name).copied()
    }

    /// Captures the current lexical environment as global-arena ObjectRefs.
    /// Each captured variable is promoted to the global arena so that mutations
    /// inside the closure persist across calls (B-27 fix).
    ///
    /// `rebind_outer`: when true (named `fn` declarations), the outer scope's
    /// binding is updated to point to the same global-arena slot so that mutations
    /// inside the function are visible to the caller (reference semantics).
    /// When false (lambda / arrow expressions), each capture is an independent copy
    /// — the outer variable is not aliased (value-snapshot semantics).
    ///
    /// Returns an empty vec at global scope (nothing to capture).
    fn capture_env(&mut self, rebind_outer: bool) -> Vec<(String, ObjectRef)> {
        let bindings = self.scopes.all_bindings();
        let mut result = Vec::new();
        for (name, r) in bindings {
            let global_ref = match r.region {
                RegionId::Global => r,
                RegionId::Scoped => {
                    let owned = self.extract(r);
                    let gref = self.plant_global(owned);
                    if rebind_outer {
                        self.scopes.rebind(&name, gref);
                    }
                    gref
                }
            };
            result.push((name, global_ref));
        }
        result
    }

    pub fn display(&self, obj_ref: ObjectRef) -> String {
        match self.resolve(obj_ref) {
            Some(ObjectData::Integer(i)) => format!("{}", i),
            Some(ObjectData::Decimal(d)) => {
                if d.fract() == 0.0 {
                    format!("{:.1}", d)
                } else {
                    // 10 significant decimal places, trailing zeros trimmed
                    let s = format!("{:.10}", d);
                    s.trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
            Some(ObjectData::Boolean(b)) => format!("{}", b),
            Some(ObjectData::Str(s)) => format!("{}", s),
            Some(ObjectData::Array { elements: refs, .. }) => {
                let elems: Vec<String> = refs.iter().map(|&r| self.display(r)).collect();
                format!("[{}]", elems.join(", "))
            }
            Some(ObjectData::Dict { entries, .. }) => {
                let kv: Vec<(ObjectRef, ObjectRef)> = entries.iter().copied().collect();
                let pairs: Vec<String> = kv
                    .into_iter()
                    .map(|(k, v)| format!("{}: {}", self.display(k), self.display(v)))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
            Some(ObjectData::Function { .. }) => "Function".to_string(),
            Some(ObjectData::Instance { class_name, fields }) => {
                let pairs: Vec<String> = fields.iter()
                    .map(|(n, v)| format!("{}: {}", n, v.display_str()))
                    .collect();
                format!("{}{{ {} }}", class_name, pairs.join(", "))
            }
            Some(ObjectData::EnumVariant { enum_name, variant }) => {
                format!("{}.{}", enum_name, variant)
            }
            Some(ObjectData::Set { elements: refs }) => {
                let elems: Vec<String> = refs.iter().map(|&r| self.display(r)).collect();
                format!("[{}]", elems.join(", "))
            }
            Some(ObjectData::Tensor { shape, data, .. }) => crate::region::format_tensor(shape, data),
            Some(ObjectData::Ptr(name)) => format!("&{}", name),
            Some(ObjectData::Null) => "null".to_string(),
            None => "❌ Referencia inválida".to_string(),
        }
    }

    // Extrae un valor completo de la arena a un OwnedValue independiente.
    // Debe llamarse ANTES de scopes.pop() para que los índices aún sean válidos.
    fn extract(&self, obj_ref: ObjectRef) -> OwnedValue {
        self.extract_inner(obj_ref, 0)
    }

    fn extract_inner(&self, obj_ref: ObjectRef, depth: usize) -> OwnedValue {
        const MAX_DEPTH: usize = 500;
        if depth > MAX_DEPTH {
            eprintln!("❌ ERROR: Maximum nesting depth ({}) exceeded", MAX_DEPTH);
            return OwnedValue::Null;
        }
        match self.resolve(obj_ref) {
            Some(ObjectData::Integer(i)) => OwnedValue::Integer(*i),
            Some(ObjectData::Decimal(d)) => OwnedValue::Decimal(*d),
            Some(ObjectData::Boolean(b)) => OwnedValue::Boolean(*b),
            Some(ObjectData::Str(s)) => OwnedValue::Str(s.clone()),
            Some(ObjectData::Array { element_type, elements: refs }) => {
                OwnedValue::Array {
                    element_type: element_type.clone(),
                    elements: refs.iter().map(|&r| self.extract_inner(r, depth + 1)).collect(),
                }
            }
            Some(ObjectData::Dict { key_type, value_type, entries }) => OwnedValue::Dict {
                key_type: key_type.clone(),
                value_type: value_type.clone(),
                entries: entries.iter().map(|&(k, v)| (self.extract_inner(k, depth + 1), self.extract_inner(v, depth + 1))).collect(),
            },
            Some(ObjectData::Function {
                return_type,
                parameters,
                body,
                captured,
                is_generator,
            }) => OwnedValue::Function {
                return_type: return_type.clone(),
                parameters: parameters.clone(),
                body: body.clone(),
                captured: captured.clone(),
                is_generator: *is_generator,
            },
            Some(ObjectData::Instance { class_name, fields }) => OwnedValue::Instance {
                class_name: class_name.clone(),
                fields: fields.clone(),
            },
            Some(ObjectData::EnumVariant { enum_name, variant }) => OwnedValue::EnumVariant {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
            },
            Some(ObjectData::Set { elements: refs }) => OwnedValue::Set {
                elements: refs.iter().map(|&r| self.extract_inner(r, depth + 1)).collect(),
            },
            Some(ObjectData::Tensor { shape, data, tid }) => {
                OwnedValue::Tensor { shape: shape.clone(), data: data.clone(), tid: *tid }
            }
            Some(ObjectData::Ptr(name)) => OwnedValue::Ptr(name.clone()),
            Some(ObjectData::Null) | None => OwnedValue::Null,
        }
    }

    // Re-aloca un OwnedValue en la arena activa (scope padre o global).
    // Debe llamarse DESPUÉS de scopes.pop().
    fn plant(&mut self, value: OwnedValue) -> ObjectRef {
        match value {
            OwnedValue::Integer(i) => self.alloc(ObjectData::Integer(i)),
            OwnedValue::Decimal(d) => self.alloc(ObjectData::Decimal(d)),
            OwnedValue::Boolean(b) => self.alloc(ObjectData::Boolean(b)),
            OwnedValue::Str(s) => self.alloc(ObjectData::Str(s)),
            OwnedValue::Array { element_type, elements: items } => {
                let refs: Vec<ObjectRef> = items.into_iter().map(|v| self.plant(v)).collect();
                self.alloc(ObjectData::Array { element_type, elements: refs })
            }
            OwnedValue::Dict { key_type, value_type, entries } => {
                let planted: Vec<(ObjectRef, ObjectRef)> = entries
                    .into_iter()
                    .map(|(k, v)| (self.plant(k), self.plant(v)))
                    .collect();
                self.alloc(ObjectData::Dict { key_type, value_type, entries: planted })
            }
            OwnedValue::Function {
                return_type,
                parameters,
                body,
                captured,
                is_generator,
            } => self.alloc(ObjectData::Function {
                return_type,
                parameters,
                body,
                captured,
                is_generator,
            }),
            OwnedValue::Instance { class_name, fields } => {
                self.alloc(ObjectData::Instance { class_name, fields })
            }
            OwnedValue::EnumVariant { enum_name, variant } => {
                self.alloc(ObjectData::EnumVariant { enum_name, variant })
            }
            OwnedValue::Set { elements: items } => {
                let refs: Vec<ObjectRef> = items.into_iter().map(|v| self.plant(v)).collect();
                self.alloc(ObjectData::Set { elements: refs })
            }
            OwnedValue::Tensor { shape, data, tid } => {
                self.alloc(ObjectData::Tensor { shape, data, tid })
            }
            OwnedValue::Ptr(name) => self.alloc(ObjectData::Ptr(name)),
            OwnedValue::Null => self.null_ref,
        }
    }

    // Igual que plant() pero siempre aloca en la arena global.
    // Necesario cuando se muta un array global desde dentro de un scope:
    // los nuevos elementos deben vivir en la misma arena que el array.
    fn plant_global(&mut self, value: OwnedValue) -> ObjectRef {
        match value {
            OwnedValue::Integer(i) => {
                let idx = self.global_arena.alloc(ObjectData::Integer(i));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Decimal(d) => {
                let idx = self.global_arena.alloc(ObjectData::Decimal(d));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Boolean(b) => {
                let idx = self.global_arena.alloc(ObjectData::Boolean(b));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Str(s) => {
                let idx = self.global_arena.alloc(ObjectData::Str(s));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Array { element_type, elements: items } => {
                let refs: Vec<ObjectRef> =
                    items.into_iter().map(|v| self.plant_global(v)).collect();
                let idx = self.global_arena.alloc(ObjectData::Array { element_type, elements: refs });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Dict { key_type, value_type, entries } => {
                let planted: Vec<(ObjectRef, ObjectRef)> = entries
                    .into_iter()
                    .map(|(k, v)| (self.plant_global(k), self.plant_global(v)))
                    .collect();
                let idx = self.global_arena.alloc(ObjectData::Dict {
                    key_type,
                    value_type,
                    entries: planted,
                });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Function { return_type, parameters, body, captured, is_generator } => {
                let idx = self.global_arena.alloc(ObjectData::Function {
                    return_type,
                    parameters,
                    body,
                    captured,
                    is_generator,
                });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Instance { class_name, fields } => {
                let idx = self.global_arena.alloc(ObjectData::Instance { class_name, fields });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::EnumVariant { enum_name, variant } => {
                let idx = self.global_arena.alloc(ObjectData::EnumVariant { enum_name, variant });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Set { elements: items } => {
                let refs: Vec<ObjectRef> = items.into_iter().map(|v| self.plant_global(v)).collect();
                let idx = self.global_arena.alloc(ObjectData::Set { elements: refs });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Tensor { shape, data, tid } => {
                let idx = self.global_arena.alloc(ObjectData::Tensor { shape, data, tid });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Ptr(name) => {
                let idx = self.global_arena.alloc(ObjectData::Ptr(name));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Null => self.null_ref,
        }
    }

    // ── Evaluación de Programa ──────────────────────────────────────────────

    pub fn eval_program(&mut self, program: &Program) -> Option<ObjectRef> {
        let mut result = self.null_ref;
        for statement in &program.statements {
            // Out statements at top level produce values that are immediately consumed
            // (printed) and never retained. Use a scratch watermark so display
            // temporaries don't accumulate in the global arena for the program lifetime.
            //
            // NOTE: Expression statements (e.g. function calls used as statements) are
            // intentionally excluded here. A function call may have persistent side effects
            // such as IndexAssign on a global array: those allocations land in the global
            // arena and must survive the statement boundary. Resetting to a pre-call
            // watermark would free them, producing dangling refs.
            let scratch_mark = match statement {
                Statement::Out(_) => Some(self.global_arena.watermark()),
                _ => None,
            };

            match self.eval_statement(statement) {
                EvalResult::Value(v) => {
                    if scratch_mark.is_none() {
                        result = v;
                    }
                    if let Some(mark) = scratch_mark {
                        self.global_arena.reset_to(mark);
                    }
                }
                EvalResult::Return(_) => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    eprintln!("❌ FLASH SCOPE ERROR: 'return' cannot be used outside of a function or conditional or loops.");
                    return None;
                }
                EvalResult::Break | EvalResult::BreakLabel(_) => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    eprintln!("❌ FLASH SCOPE ERROR: 'break' cannot be used outside of a loop.");
                    return None;
                }
                EvalResult::Continue | EvalResult::ContinueLabel(_) => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    eprintln!("❌ FLASH SCOPE ERROR: 'continue' cannot be used outside of a loop.");
                    return None;
                }
                EvalResult::Error => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    return None;
                }
                EvalResult::Throw(r) => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    let msg = self.display(r);
                    eprintln!("❌ UNCAUGHT EXCEPTION: {msg}");
                    return None;
                }
            }
        }
        Some(result)
    }

    fn is_truthy(&self, data: &ObjectData) -> bool {
        match data {
            ObjectData::Boolean(b) => *b,
            ObjectData::Null => false,
            _ => true,
        }
    }

    fn update_dict(
        &mut self,
        obj_ref: ObjectRef,
        key_type: String,
        value_type: String,
        entries: Vec<(ObjectRef, ObjectRef)>,
    ) {
        let data = ObjectData::Dict { key_type, value_type, entries };
        match obj_ref.region {
            RegionId::Global => self.global_arena.update(obj_ref.index, data),
            RegionId::Scoped => self.scopes.arena.update(obj_ref.index, data),
        }
    }

    fn update_array(&mut self, arr_ref: ObjectRef, element_type: Option<String>, elems: Vec<ObjectRef>) {
        let data = ObjectData::Array { element_type, elements: elems };
        match arr_ref.region {
            RegionId::Global => self.global_arena.update(arr_ref.index, data),
            RegionId::Scoped => self.scopes.arena.update(arr_ref.index, data),
        }
    }

    // ── fmt_value: display respecting op_str / op_str on array elements ────────

    /// Rich display: calls `op_str()` on Instances if defined, recurses into Arrays,
    /// falls back to `display()` for everything else.
    /// Returns `Err(EvalResult)` if `op_str` throws or errors, so callers can propagate.
    fn fmt_value(&mut self, obj_ref: ObjectRef) -> Result<String, EvalResult> {
        match self.resolve(obj_ref).cloned() {
            Some(ObjectData::Instance { ref class_name, .. }) => {
                let cn = class_name.clone();
                if self.find_method(&cn, "op_str").is_some() {
                    match self.call_op_method(obj_ref, &cn, "op_str", vec![], 0, 0) {
                        EvalResult::Value(r) => {
                            if let Some(ObjectData::Str(s)) = self.resolve(r) {
                                return Ok(s.clone());
                            }
                            Ok(self.display(r))
                        }
                        EvalResult::Throw(v) => Err(EvalResult::Throw(v)),
                        EvalResult::Error    => Err(EvalResult::Error),
                        other                => Err(other),
                    }
                } else {
                    Ok(self.display(obj_ref))
                }
            }
            Some(ObjectData::Array { elements, .. }) => {
                let mut parts = Vec::with_capacity(elements.len());
                for elem_ref in elements {
                    parts.push(self.fmt_value(elem_ref)?);
                }
                Ok(format!("[{}]", parts.join(", ")))
            }
            _ => Ok(self.display(obj_ref)),
        }
    }

    // ── Operator overloading dispatch ─────────────────────────────────────────

    /// Calls an `op_*` overload method on `inst_ref`. `arg_vals` are the already-extracted
    /// arguments (0 for unary like `op_neg`, 1 for binary like `op_add`).
    fn call_op_method(
        &mut self,
        inst_ref: ObjectRef,
        class_name: &str,
        method_name: &str,
        arg_vals: Vec<OwnedValue>,
        line: usize,
        column: usize,
    ) -> EvalResult {
        let method = match self.find_method(class_name, method_name) {
            Some(m) => m,
            None => {
                eprintln!("❌ ERROR: no operator overload '{}' on class '{}'", method_name, class_name);
                return EvalResult::Error;
            }
        };

        if self.call_depth >= 1000 {
            eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
            return EvalResult::Error;
        }

        let old_executing_class = self.executing_class.take();
        self.executing_class = Some(class_name.to_string());
        self.call_stack.push(CallFrame {
            name: format!("{}::{}", class_name, method_name),
            line,
            column,
        });
        self.scopes.push();
        self.call_depth += 1;
        self.scopes.declare("this".to_string(), inst_ref);

        for (i, param) in method.parameters.iter().enumerate() {
            let arg_ref = if i < arg_vals.len() {
                self.plant(arg_vals[i].clone())
            } else {
                self.null_ref
            };
            self.scopes.declare(param.name.clone(), arg_ref);
        }

        let mut result_ref = self.null_ref;
        let mut error = false;
        let mut method_throw: Option<ObjectRef> = None;
        for stmt in &method.body.statements {
            match self.eval_statement(stmt) {
                EvalResult::Value(_)  => {}
                EvalResult::Return(v) => { result_ref = v; break; }
                EvalResult::Throw(v)  => { method_throw = Some(v); break; }
                EvalResult::Error     => { error = true; break; }
                EvalResult::Break | EvalResult::Continue
                | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                    eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop in operator method '{}'.", method_name);
                    error = true;
                    break;
                }
            }
        }

        let owned = self.extract(result_ref);
        let throw_owned = method_throw.map(|r| self.extract(r));
        self.call_depth -= 1;
        self.scopes.pop();
        self.call_stack.pop();
        self.executing_class = old_executing_class;

        if error { return EvalResult::Error; }
        if let Some(t) = throw_owned { return EvalResult::Throw(self.plant(t)); }
        EvalResult::Value(self.plant(owned))
    }

    // ── Callback calling helper ───────────────────────────────────────────────

    fn call_function(&mut self, func_ref: ObjectRef, arg_vals: Vec<OwnedValue>) -> EvalResult {
        if self.call_depth >= 1000 {
            eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
            return EvalResult::Error;
        }
        let func_data = self.resolve(func_ref).cloned();
        match func_data {
            Some(ObjectData::Function { parameters, body, captured, .. }) => {
                let has_rest = parameters.last().map(|p| p.is_rest).unwrap_or(false);
                let required = parameters.iter().filter(|p| !p.is_rest && p.default_value.is_none()).count();
                let max_pos  = if has_rest { usize::MAX } else { parameters.len() };
                if arg_vals.len() < required || arg_vals.len() > max_pos {
                    eprintln!(
                        "❌ ERROR: Callback expected {} argument(s), got {}",
                        parameters.len(), arg_vals.len()
                    );
                    return EvalResult::Error;
                }
                self.scopes.push();
                self.call_depth += 1;
                for (name, cap_ref) in &captured {
                    self.scopes.declare(name.clone(), *cap_ref);
                }
                for (i, param) in parameters.iter().enumerate() {
                    if param.is_rest {
                        let rest_refs: Vec<ObjectRef> = arg_vals[i..].iter()
                            .map(|v| self.plant(v.clone()))
                            .collect();
                        let rest_ref = self.alloc(ObjectData::Array { element_type: None, elements: rest_refs });
                        self.scopes.declare(param.name.clone(), rest_ref);
                        break;
                    }
                    let local_ref = if i < arg_vals.len() {
                        self.plant(arg_vals[i].clone())
                    } else if let Some(default_expr) = &param.default_value {
                        let default_expr = default_expr.clone();
                        match self.eval_expression(&default_expr) {
                            EvalResult::Value(v) => v,
                            _ => self.null_ref,
                        }
                    } else {
                        self.null_ref
                    };
                    self.scopes.declare(param.name.clone(), local_ref);
                }
                let mut result_ref = self.null_ref;
                let mut fn_throw: Option<ObjectRef> = None;
                for s in &body.statements {
                    match self.eval_statement(s) {
                        EvalResult::Value(_) => {} // only explicit return contributes result
                        EvalResult::Return(v) => { result_ref = v; break; }
                        EvalResult::Throw(v)  => { fn_throw = Some(v); break; }
                        EvalResult::Error => {
                            self.call_depth -= 1;
                            self.scopes.pop();
                            return EvalResult::Error;
                        }
                        EvalResult::Break | EvalResult::Continue
                        | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                            eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop.");
                            self.call_depth -= 1;
                            self.scopes.pop();
                            return EvalResult::Error;
                        }
                    }
                }
                if let Some(thrown) = fn_throw {
                    let owned = self.extract(thrown);
                    self.call_depth -= 1;
                    self.scopes.pop();
                    return EvalResult::Throw(self.plant(owned));
                }
                let owned = self.extract(result_ref);
                self.call_depth -= 1;
                self.scopes.pop();
                EvalResult::Value(self.plant(owned))
            }
            _ => {
                eprintln!("❌ ERROR: Callback is not a function");
                EvalResult::Error
            }
        }
    }

    fn callback_param_count(&self, func_ref: ObjectRef) -> Option<usize> {
        match self.resolve(func_ref) {
            Some(ObjectData::Function { parameters, .. }) => Some(parameters.len()),
            _ => None,
        }
    }

    // ── Built-in global functions ─────────────────────────────────────────────

}

// ── Free helpers ──────────────────────────────────────────────────────────────

/// Maps a binary operator symbol to its overload method name on instances.
/// Returns `""` for operators that cannot be overloaded.
fn operator_to_method_name(op: &str) -> &'static str {
    match op {
        "+"  => "op_add",
        "-"  => "op_sub",
        "*"  => "op_mul",
        "/"  => "op_div",
        "%"  => "op_mod",
        "==" => "op_eq",
        "!=" => "op_ne",
        "<"  => "op_lt",
        "<=" => "op_le",
        ">"  => "op_gt",
        ">=" => "op_ge",
        _    => "",
    }
}

// ── Math namespace ─────────────────────────────────────────────────────────────

/// Formats a decimal the same way `display()` does — trims trailing zeros beyond 10 decimal places.
fn format_decimal(d: f64) -> String {
    if d.fract() == 0.0 {
        format!("{:.1}", d)
    } else {
        let s = format!("{:.10}", d);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn obj_data_eq(a: &Option<ObjectData>, b: &Option<ObjectData>) -> bool {
    match (a, b) {
        (Some(ObjectData::Integer(x)),  Some(ObjectData::Integer(y)))  => x == y,
        (Some(ObjectData::Decimal(x)),  Some(ObjectData::Decimal(y)))  => x == y,
        (Some(ObjectData::Boolean(x)),  Some(ObjectData::Boolean(y)))  => x == y,
        (Some(ObjectData::Str(x)),      Some(ObjectData::Str(y)))      => x == y,
        (Some(ObjectData::Null),        Some(ObjectData::Null))        => true,
        _ => false,
    }
}

fn obj_data_to_key_str(data: &ObjectData) -> String {
    match data {
        ObjectData::Str(s) => s.clone(),
        ObjectData::Integer(i) => i.to_string(),
        ObjectData::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}

fn type_matches(expected: &str, data: &ObjectData) -> bool {
    match (expected, data) {
        ("int", ObjectData::Integer(_)) => true,
        ("decimal", ObjectData::Decimal(_)) => true,
        ("string", ObjectData::Str(_)) => true,
        ("bool", ObjectData::Boolean(_)) => true,
        ("null", ObjectData::Null) => true,
        ("void", ObjectData::Null) => true,
        ("dict", ObjectData::Dict { .. }) => true,
        ("array", ObjectData::Array { .. }) => true,
        ("any", _) => true,
        // "[type]" param accepts any array (element type enforced at construction)
        (t, ObjectData::Array { .. }) if t.starts_with('[') && t.ends_with(']') => true,
        // Nullable: "int?" accepts int or null
        (t, ObjectData::Null) if t.ends_with('?') => true,
        (t, d) if t.ends_with('?') => {
            let base = &t[..t.len() - 1];
            type_matches(base, d)
        }
        (t, ObjectData::Instance { class_name, .. }) => t == class_name.as_str(),
        _ => false,
    }
}

// ── JSON helpers ────────────────────────────────────────────────────────────

fn json_stringify_owned(val: &OwnedValue) -> String {
    match val {
        OwnedValue::Null => "null".to_string(),
        OwnedValue::Boolean(b) => b.to_string(),
        OwnedValue::Integer(i) => i.to_string(),
        OwnedValue::Decimal(d) => {
            if !d.is_finite() { return "null".to_string(); }
            if d.fract() == 0.0 { format!("{:.1}", d) }
            else {
                let s = format!("{:.10}", d);
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            }
        }
        OwnedValue::Str(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            format!("\"{}\"", escaped)
        }
        OwnedValue::Array { elements, .. } => {
            let parts: Vec<String> = elements.iter().map(json_stringify_owned).collect();
            format!("[{}]", parts.join(","))
        }
        OwnedValue::Dict { entries, .. } => {
            let parts: Vec<String> = entries.iter().map(|(k, v)| {
                let key = match k {
                    OwnedValue::Str(s) => format!("\"{}\"", s.replace('"', "\\\"")),
                    OwnedValue::Integer(i) => format!("\"{}\"", i),
                    other => format!("\"{}\"", other.display_str()),
                };
                format!("{}:{}", key, json_stringify_owned(v))
            }).collect();
            format!("{{{}}}", parts.join(","))
        }
        OwnedValue::Instance { class_name: _, fields } => {
            let parts: Vec<String> = fields.iter().map(|(k, v)| {
                format!("\"{}\":{}", k.replace('"', "\\\""), json_stringify_owned(v))
            }).collect();
            format!("{{{}}}", parts.join(","))
        }
        OwnedValue::Set { elements } => {
            let parts: Vec<String> = elements.iter().map(json_stringify_owned).collect();
            format!("[{}]", parts.join(","))
        }
        OwnedValue::Tensor { shape, data, .. } => {
            fn nest_json(shape: &[usize], data: &[f64], off: usize) -> String {
                if shape.len() == 1 {
                    let vs: Vec<String> = (0..shape[0]).map(|i| {
                        let v = data[off + i];
                        if v.fract() == 0.0 { format!("{:.1}", v) }
                        else { format!("{:.10}", v).trim_end_matches('0').trim_end_matches('.').to_string() }
                    }).collect();
                    format!("[{}]", vs.join(","))
                } else {
                    let stride: usize = shape[1..].iter().product();
                    let rows: Vec<String> = (0..shape[0]).map(|i| nest_json(&shape[1..], data, off + i * stride)).collect();
                    format!("[{}]", rows.join(","))
                }
            }
            nest_json(shape, data, 0)
        }
        OwnedValue::EnumVariant { enum_name, variant } => {
            format!("\"{}\"", format!("{}.{}", enum_name, variant).replace('"', "\\\""))
        }
        OwnedValue::Function { .. } => "null".to_string(),
        OwnedValue::Ptr(name) => format!("\"&{}\"", name),
    }
}

// A minimal recursive-descent JSON parser — no external crates.
fn json_parse(input: &str) -> Result<OwnedValue, String> {
    let chars: Vec<char> = input.chars().collect();
    let (val, pos) = json_parse_value(&chars, 0)?;
    let pos = json_skip_ws(&chars, pos);
    if pos != chars.len() {
        return Err(format!("unexpected trailing characters at position {}", pos));
    }
    Ok(val)
}

fn json_skip_ws(chars: &[char], mut pos: usize) -> usize {
    while pos < chars.len() && (chars[pos] == ' ' || chars[pos] == '\t' || chars[pos] == '\n' || chars[pos] == '\r') {
        pos += 1;
    }
    pos
}

fn json_parse_value(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    let pos = json_skip_ws(chars, pos);
    if pos >= chars.len() {
        return Err("unexpected end of input".to_string());
    }
    match chars[pos] {
        '"'       => json_parse_string(chars, pos),
        '['       => json_parse_array(chars, pos),
        '{'       => json_parse_object(chars, pos),
        't'       => {
            if chars.get(pos..pos+4) == Some(&['t','r','u','e']) {
                Ok((OwnedValue::Boolean(true), pos + 4))
            } else { Err(format!("invalid token at {}", pos)) }
        }
        'f'       => {
            if chars.get(pos..pos+5) == Some(&['f','a','l','s','e']) {
                Ok((OwnedValue::Boolean(false), pos + 5))
            } else { Err(format!("invalid token at {}", pos)) }
        }
        'n'       => {
            if chars.get(pos..pos+4) == Some(&['n','u','l','l']) {
                Ok((OwnedValue::Null, pos + 4))
            } else { Err(format!("invalid token at {}", pos)) }
        }
        '-' | '0'..='9' => json_parse_number(chars, pos),
        c => Err(format!("unexpected character '{}' at position {}", c, pos)),
    }
}

fn json_parse_string(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    // pos points to opening '"'
    let mut i = pos + 1;
    let mut s = String::new();
    while i < chars.len() {
        match chars[i] {
            '"' => { return Ok((OwnedValue::Str(s), i + 1)); }
            '\\' => {
                i += 1;
                if i >= chars.len() { return Err("unterminated string escape".to_string()); }
                match chars[i] {
                    '"'  => s.push('"'),
                    '\\' => s.push('\\'),
                    '/'  => s.push('/'),
                    'n'  => s.push('\n'),
                    'r'  => s.push('\r'),
                    't'  => s.push('\t'),
                    'b'  => s.push('\u{0008}'),
                    'f'  => s.push('\u{000C}'),
                    'u'  => {
                        if i + 4 >= chars.len() { return Err("invalid \\u escape".to_string()); }
                        let hex: String = chars[i+1..i+5].iter().collect();
                        let code = u32::from_str_radix(&hex, 16)
                            .map_err(|_| format!("invalid \\u{}", hex))?;
                        let ch = char::from_u32(code)
                            .ok_or_else(|| format!("invalid unicode codepoint {}", code))?;
                        s.push(ch);
                        i += 4;
                    }
                    c => { s.push('\\'); s.push(c); }
                }
                i += 1;
            }
            c => { s.push(c); i += 1; }
        }
    }
    Err("unterminated string".to_string())
}

fn json_parse_array(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    let mut i = json_skip_ws(chars, pos + 1); // skip '['
    let mut elements = Vec::new();
    if i < chars.len() && chars[i] == ']' {
        return Ok((OwnedValue::Array { element_type: None, elements }, i + 1));
    }
    loop {
        let (val, next) = json_parse_value(chars, i)?;
        elements.push(val);
        i = json_skip_ws(chars, next);
        if i >= chars.len() { return Err("unterminated array".to_string()); }
        match chars[i] {
            ']' => { return Ok((OwnedValue::Array { element_type: None, elements }, i + 1)); }
            ',' => { i = json_skip_ws(chars, i + 1); }
            c   => { return Err(format!("expected ',' or ']', got '{}'", c)); }
        }
    }
}

fn json_parse_object(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    let mut i = json_skip_ws(chars, pos + 1); // skip '{'
    let mut entries: Vec<(OwnedValue, OwnedValue)> = Vec::new();
    if i < chars.len() && chars[i] == '}' {
        return Ok((OwnedValue::Dict { key_type: "string".to_string(), value_type: "any".to_string(), entries }, i + 1));
    }
    loop {
        i = json_skip_ws(chars, i);
        if i >= chars.len() || chars[i] != '"' {
            return Err(format!("expected string key at position {}", i));
        }
        let (key, next_k) = json_parse_string(chars, i)?;
        i = json_skip_ws(chars, next_k);
        if i >= chars.len() || chars[i] != ':' {
            return Err(format!("expected ':' at position {}", i));
        }
        i = json_skip_ws(chars, i + 1);
        let (val, next_v) = json_parse_value(chars, i)?;
        entries.push((key, val));
        i = json_skip_ws(chars, next_v);
        if i >= chars.len() { return Err("unterminated object".to_string()); }
        match chars[i] {
            '}' => { return Ok((OwnedValue::Dict { key_type: "string".to_string(), value_type: "any".to_string(), entries }, i + 1)); }
            ',' => { i = json_skip_ws(chars, i + 1); }
            c   => { return Err(format!("expected ',' or '}}', got '{}'", c)); }
        }
    }
}

fn json_parse_number(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    let mut i = pos;
    let mut s = String::new();
    if i < chars.len() && chars[i] == '-' { s.push('-'); i += 1; }
    while i < chars.len() && chars[i].is_ascii_digit() { s.push(chars[i]); i += 1; }
    let is_float = i < chars.len() && (chars[i] == '.' || chars[i] == 'e' || chars[i] == 'E');
    if i < chars.len() && chars[i] == '.' {
        s.push('.');
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() { s.push(chars[i]); i += 1; }
    }
    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
        s.push(chars[i]); i += 1;
        if i < chars.len() && (chars[i] == '+' || chars[i] == '-') { s.push(chars[i]); i += 1; }
        while i < chars.len() && chars[i].is_ascii_digit() { s.push(chars[i]); i += 1; }
    }
    if is_float || s.contains('.') || s.contains('e') || s.contains('E') {
        let f: f64 = s.parse().map_err(|_| format!("invalid number '{}'", s))?;
        Ok((OwnedValue::Decimal(f), i))
    } else {
        let n: i64 = s.parse().map_err(|_| format!("invalid integer '{}'", s))?;
        Ok((OwnedValue::Integer(n), i))
    }
}
