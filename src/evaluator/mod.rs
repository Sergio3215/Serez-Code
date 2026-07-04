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
mod methods_dec;
pub(crate) mod namespaces_datetime;
mod namespaces_os;
pub(crate) mod namespaces_gui;
mod namespaces_task;
mod namespaces_regex;
#[cfg(feature = "audio")]
mod namespaces_media;


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
    // ── GUI ───────────────────────────────────────────────────────────────────
    // Estado GUI del lado del intérprete (canvas + texto + input snapshot). La
    // ventana/EventLoop de winit viven en el HILO PRINCIPAL (ver main.rs y
    // namespaces_gui::gui_host_main_loop); se comunican por GUI_HOST. None hasta open.
    gui_state: Option<namespaces_gui::GuiState>,
    // Tipografía GUI (loadFont/setFont + cache de glifos). Vive fuera de gui_state
    // para sobrevivir abrir/cerrar ventana y servir measureText sin ventana.
    gui_fonts: Option<namespaces_gui::GuiFonts>,
    // Procesos lanzados con OS.spawn (no bloqueante). OS.tick() los cosecha y dispara
    // sus callbacks onOk/onErr en este hilo (cooperativo, sin background thread).
    spawned: Vec<namespaces_os::SpawnedJob>,
    // Audio (Media.playSound): stream + sinks activos. None hasta el primer uso.
    #[cfg(feature = "audio")]
    media: Option<namespaces_media::MediaState>,
    // ── Task context ──────────────────────────────────────────────────────────
    task_id: Option<i64>,
    task_arg: Option<String>,
    // ── Recoverable runtime errors (catchable by try/catch) ────────────────────
    // (kind, message) of the most recent recoverable runtime error, recorded by
    // `rt_err`/`rt_err_kind`. `eval_try` reads it to build the structured Error
    // object bound in `catch (e)`. `try_depth` > 0 means we are inside a try body
    // that has a catch handler, so `rt_err` suppresses the stderr noise (the error
    // is handled, not aborted).
    last_error: Option<(String, String)>,
    try_depth: usize,
}

// ── Free-identifier collection (for consistent lambda capture, B-83) ──────────
// Best-effort walk: collects identifier names referenced inside a block. Used to
// also snapshot referenced GLOBAL data vars when a lambda is created, so closures
// capture globals by value (snapshot) just like they already snapshot scoped
// locals. Over-approximation (e.g. collecting a nested lambda's own locals) is
// harmless; incompleteness only degrades to live global lookup (the prior
// behavior) and can never break a valid closure.
fn collect_idents_block(b: &crate::ast::BlockStatement, out: &mut Vec<String>) {
    for s in &b.statements { collect_idents_stmt(s, out); }
}

fn collect_idents_stmt(s: &crate::ast::Statement, out: &mut Vec<String>) {
    use crate::ast::Statement as St;
    match s {
        St::Let(l) => collect_idents_expr(&l.value, out),
        St::Assign(a) => { out.push(a.name.clone()); collect_idents_expr(&a.value, out); }
        St::Block(b) | St::Unsafe(b) => collect_idents_block(b, out),
        St::Return(r) => collect_idents_expr(&r.return_value, out),
        St::Expression(e) | St::Throw(e) | St::Yield(e) => collect_idents_expr(e, out),
        St::Out(o) => collect_idents_expr(&o.value, out),
        St::While(w) | St::DoWhile(w) => { collect_idents_expr(&w.condition, out); collect_idents_block(&w.body, out); }
        St::For(f) => {
            collect_idents_expr(&f.init.value, out);
            collect_idents_expr(&f.condition, out);
            collect_idents_expr(&f.update.value, out);
            collect_idents_block(&f.body, out);
        }
        St::ForEach(fe) => { collect_idents_expr(&fe.iterable, out); collect_idents_block(&fe.body, out); }
        St::IndexAssign(ia) => { collect_idents_expr(&ia.target, out); collect_idents_expr(&ia.index, out); collect_idents_expr(&ia.value, out); }
        St::FieldAssign(fa) => { out.push(fa.object.clone()); collect_idents_expr(&fa.value, out); }
        St::DerefAssign { ptr, value } => { collect_idents_expr(ptr, out); collect_idents_expr(value, out); }
        _ => {}
    }
}

fn collect_idents_expr(e: &crate::ast::Expression, out: &mut Vec<String>) {
    use crate::ast::Expression as Ex;
    match e {
        Ex::Identifier(n) => out.push(n.clone()),
        Ex::Prefix(_, inner) | Ex::Spread(inner) | Ex::AddressOf(inner) | Ex::Deref(inner) => collect_idents_expr(inner, out),
        Ex::Infix(i) => { collect_idents_expr(&i.left, out); collect_idents_expr(&i.right, out); }
        Ex::Call(c) => { collect_idents_expr(&c.function, out); for a in &c.arguments { collect_idents_expr(a, out); } }
        Ex::DotCall(d) => { collect_idents_expr(&d.object, out); for a in &d.arguments { collect_idents_expr(a, out); } }
        Ex::Index(ix) => { collect_idents_expr(&ix.left, out); collect_idents_expr(&ix.index, out); }
        Ex::ArrayLiteral(al) => { for el in &al.elements { collect_idents_expr(el, out); } }
        Ex::DictLiteral(dl) => { for (k, v) in &dl.entries { collect_idents_expr(k, out); collect_idents_expr(v, out); } }
        Ex::EntryLiteral(k, v) => { collect_idents_expr(k, out); collect_idents_expr(v, out); }
        Ex::Ternary(t) => { collect_idents_expr(&t.condition, out); collect_idents_expr(&t.then_expr, out); collect_idents_expr(&t.else_expr, out); }
        Ex::If(ife) => {
            collect_idents_expr(&ife.condition, out);
            collect_idents_block(&ife.consequence, out);
            if let Some(alt) = &ife.alternative { collect_idents_block(alt, out); }
        }
        Ex::InterpolatedString(parts) => { for p in parts { if let crate::ast::StringPart::Expr(ex) = p { collect_idents_expr(ex, out); } } }
        Ex::New(n) => match &n.args {
            crate::ast::NewArgs::Positional(v) => { for a in v { collect_idents_expr(a, out); } }
            crate::ast::NewArgs::Fields(f) => { for (_, a) in f { collect_idents_expr(a, out); } }
        },
        Ex::Match(m) => {
            collect_idents_expr(&m.subject, out);
            for arm in &m.arms {
                if let Some(g) = &arm.guard { collect_idents_expr(g, out); }
                collect_idents_block(&arm.body, out);
            }
        }
        Ex::FunctionLiteral(fl) => collect_idents_block(&fl.body, out),
        Ex::Lambda(l) => match &l.body {
            crate::ast::LambdaBody::Block(b) => collect_idents_block(b, out),
            crate::ast::LambdaBody::Expr(ex) => collect_idents_expr(ex, out),
        },
        Ex::UnsafeBlock(b) => collect_idents_block(b, out),
        Ex::ObjectPatch(fields) => { for (_, ex) in fields { collect_idents_expr(ex, out); } }
        _ => {}
    }
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
            gui_state: None,
            gui_fonts: None,
            #[cfg(feature = "audio")]
            media: None,
            spawned: Vec::new(),
            task_id: None,
            task_arg: None,
            last_error: None,
            try_depth: 0,
        }
    }

    /// Raise a recoverable runtime error with the default kind ("RuntimeError").
    pub(crate) fn rt_err(&mut self, msg: impl Into<String>) -> EvalResult {
        self.rt_err_kind("RuntimeError", msg)
    }

    /// Raise a recoverable runtime error tagged with a `kind` (e.g.
    /// "IndexOutOfBounds", "DivisionByZero", "TypeError"). Records the (kind,
    /// message) pair so an enclosing `try/catch` can bind a structured Error
    /// object (`e.message`, `e.kind`). Prints to stderr only when NOT inside a
    /// try-with-catch, so caught errors don't spam the console. Always returns
    /// `EvalResult::Error`, which `eval_try` intercepts for recoverable errors.
    pub(crate) fn rt_err_kind(&mut self, kind: impl Into<String>, msg: impl Into<String>) -> EvalResult {
        let m = msg.into();
        let k = kind.into();
        if self.try_depth == 0 {
            eprintln!("❌ ERROR: {}", m);
            self.print_call_stack();
        }
        self.last_error = Some((k, m));
        EvalResult::Error
    }

    /// Read-only diagnostic: current sizes of the two arenas (object slots).
    /// global never shrinks (top-level + everything promoted); scoped is the
    /// rewindable stack. NOT a GC — just instrumentation for measuring growth.
    pub fn arena_stats(&self) -> (usize, usize) {
        (self.global_arena.watermark(), self.scopes.arena.watermark())
    }

    pub fn set_permissions(&mut self, perms: Vec<String>) {
        for p in perms {
            self.permissions.insert(p);
        }
    }

    pub fn set_task_context(&mut self, id: i64, arg: String) {
        self.task_id = Some(id);
        self.task_arg = Some(arg);
        // Por defecto, dale permiso de "Task" a sí mismo para que los workers puedan usar Task.reply / Task.message
        self.permissions.insert("Task".to_string());
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

    /// Nearest binding of `name` (inner → outer, then globals) that holds a
    /// Function. Lets a call reach a function shadowed by a non-callable
    /// binding of the same name (e.g. a parameter `h` over a global `fn h`).
    pub(super) fn lookup_callable(&self, name: &str) -> Option<ObjectRef> {
        for r in self.scopes.lookup_chain(name) {
            if matches!(self.resolve(r), Some(ObjectData::Function { .. })) {
                return Some(r);
            }
        }
        let r = self.global_bindings.get(name).copied()?;
        match self.resolve(r) {
            Some(ObjectData::Function { .. }) => Some(r),
            _ => None,
        }
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

    /// Capture for lambdas — CELL semantics: the lambda and the enclosing
    /// scope share the variable, so mutations inside the closure are visible
    /// outside (and vice versa), at every nesting level. A scoped local is
    /// promoted once to a global-arena cell and the OUTER binding is rebound
    /// to that same cell (exactly like named `fn` captures, `rebind_outer`);
    /// a second lambda capturing the same variable sees the already-Global
    /// ref and reuses the cell. Referenced global data vars are captured as
    /// their live binding (no snapshot). Per-iteration `let`s still get one
    /// fresh cell per iteration because each iteration declares a new binding.
    ///
    /// Scoped locals are captured ONLY when the body references them: each
    /// cell lives in the GLOBAL arena (it must outlive the frame), which
    /// never shrinks — capturing every visible local leaked one permanent
    /// slot per unused local per lambda creation (deadly for per-frame /
    /// per-iteration lambdas). `this` is always considered referenced,
    /// defensively.
    fn capture_lambda_env(&mut self, body: &crate::ast::BlockStatement) -> Vec<(String, ObjectRef)> {
        let mut names: Vec<String> = Vec::new();
        collect_idents_block(body, &mut names);
        let mut referenced: std::collections::HashSet<String> = names.into_iter().collect();
        referenced.insert("this".to_string());

        // Same walk/order as capture_env, filtered to referenced names.
        let bindings = self.scopes.all_bindings();
        let mut captured = Vec::new();
        for (name, r) in bindings {
            if !referenced.contains(&name) { continue; }
            let global_ref = match r.region {
                RegionId::Global => r,
                RegionId::Scoped => {
                    let owned = self.extract(r);
                    let gref = self.plant_global(owned);
                    // Alias the enclosing binding to the promoted cell so a
                    // mutation inside the closure is seen by the outer scope.
                    self.scopes.rebind(&name, gref);
                    gref
                }
            };
            captured.push((name, global_ref));
        }

        let mut have: std::collections::HashSet<String> =
            captured.iter().map(|(n, _)| n.clone()).collect();
        for name in referenced {
            if have.contains(&name) { continue; }
            have.insert(name.clone());
            // Scoped locals are already captured above.
            if self.scopes.lookup(&name).is_some() { continue; }
            let gref = match self.global_bindings.get(&name) { Some(&r) => r, None => continue };
            if matches!(self.resolve(gref), Some(ObjectData::Function { .. })) { continue; }
            // Live binding, not a snapshot: writes meet in the global slot.
            captured.push((name, gref));
        }
        captured
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
            Some(ObjectData::Dec(d)) => d.to_string(),
            Some(ObjectData::Boolean(b)) => format!("{}", b),
            Some(ObjectData::Str(s)) => format!("{}", s),
            Some(ObjectData::Array { elements, .. }) => {
                let elems: Vec<String> = elements.iter().map(|e| e.display_str()).collect();
                format!("[{}]", elems.join(", "))
            }
            Some(ObjectData::Dict { entries, .. }) => {
                let pairs: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.display_str(), v.display_str()))
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
            Some(ObjectData::Set { elements, .. }) => {
                let elems: Vec<String> = elements.iter().map(|e| e.display_str()).collect();
                format!("Set[{}]", elems.join(", "))
            }
            Some(ObjectData::Tensor { shape, data, .. }) => crate::region::format_tensor(shape, data),
            Some(ObjectData::DateTime { epoch_ms, utc }) => crate::region::format_datetime(*epoch_ms, *utc),
            Some(ObjectData::DateField { value, .. }) => format!("{}", value),
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

    fn extract_inner_owned(&self, owned: OwnedValue, depth: usize) -> OwnedValue {
        const MAX_DEPTH: usize = 500;
        if depth > MAX_DEPTH {
            eprintln!("❌ ERROR: Maximum nesting depth ({}) exceeded", MAX_DEPTH);
            return OwnedValue::Null;
        }
        match owned {
            OwnedValue::Array { element_type, elements } => {
                OwnedValue::Array {
                    element_type,
                    elements: elements.into_iter().map(|e| self.extract_inner_owned(e, depth + 1)).collect(),
                }
            }
            OwnedValue::Dict { key_type, value_type, entries } => {
                OwnedValue::Dict {
                    key_type,
                    value_type,
                    entries: entries.into_iter()
                        .map(|(k, v)| (self.extract_inner_owned(k, depth + 1), self.extract_inner_owned(v, depth + 1)))
                        .collect(),
                }
            }
            OwnedValue::Set { elements } => {
                OwnedValue::Set {
                    elements: elements.into_iter().map(|e| self.extract_inner_owned(e, depth + 1)).collect(),
                }
            }
            other => other,
        }
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
            Some(ObjectData::Dec(d)) => OwnedValue::Dec(*d),
            Some(ObjectData::Boolean(b)) => OwnedValue::Boolean(*b),
            Some(ObjectData::Str(s)) => OwnedValue::Str(s.clone()),
            Some(ObjectData::Array { element_type, elements }) => {
                OwnedValue::Array {
                    element_type: element_type.clone(),
                    elements: elements.iter().map(|e| self.extract_inner_owned(e.clone(), depth + 1)).collect(),
                }
            }
            Some(ObjectData::Dict { key_type, value_type, entries, .. }) => OwnedValue::Dict {
                key_type: key_type.clone(),
                value_type: value_type.clone(),
                entries: entries.iter().map(|(k, v)| (self.extract_inner_owned(k.clone(), depth + 1), self.extract_inner_owned(v.clone(), depth + 1))).collect(),
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
            Some(ObjectData::Set { elements, .. }) => OwnedValue::Set {
                elements: elements.iter().map(|e| self.extract_inner_owned(e.clone(), depth + 1)).collect(),
            },
            Some(ObjectData::Tensor { shape, data, tid }) => {
                OwnedValue::Tensor { shape: shape.clone(), data: data.clone(), tid: *tid }
            }
            Some(ObjectData::DateTime { epoch_ms, utc }) => {
                OwnedValue::DateTime { epoch_ms: *epoch_ms, utc: *utc }
            }
            Some(ObjectData::DateField { epoch_ms, utc, field, value }) => {
                OwnedValue::DateField { epoch_ms: *epoch_ms, utc: *utc, field: *field, value: *value }
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
            OwnedValue::Dec(d) => self.alloc(ObjectData::Dec(d)),
            OwnedValue::Boolean(b) => self.alloc(ObjectData::Boolean(b)),
            OwnedValue::Str(s) => self.alloc(ObjectData::Str(s)),
            OwnedValue::Array { element_type, elements: items } => {
                self.alloc(ObjectData::Array { element_type, elements: items })
            }
            OwnedValue::Dict { key_type, value_type, entries } => {
                self.alloc(ObjectData::Dict { key_type, value_type, entries, index: Default::default() })
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
                self.alloc(ObjectData::Set { elements: items, index: Default::default() })
            }
            OwnedValue::Tensor { shape, data, tid } => {
                self.alloc(ObjectData::Tensor { shape, data, tid })
            }
            OwnedValue::DateTime { epoch_ms, utc } => {
                self.alloc(ObjectData::DateTime { epoch_ms, utc })
            }
            OwnedValue::DateField { epoch_ms, utc, field, value } => {
                self.alloc(ObjectData::DateField { epoch_ms, utc, field, value })
            }
            OwnedValue::Ptr(name) => self.alloc(ObjectData::Ptr(name)),
            OwnedValue::Null => self.null_ref,
        }
    }

    // Cuando se ASIGNA un contenedor (Array/Dict/Set) a una variable desde dentro
    // de un bloque anidado, sus refs internas pueden apuntar a un scope que se
    // libera ANTES que la variable (la variable vive en un frame más externo).
    // Al hacer pop de ese bloque interno, esas refs quedan colgando y el elemento
    // se lee como basura ("Index operator not supported" / longitud correcta pero
    // elemento corrupto). Se promueve el contenedor (deep) a la arena global para
    // que sus elementos sobrevivan. Escalares e instancias (campos OwnedValue) no
    // sufren esto y se devuelven sin tocar — evita fugas con `i = i + 1`, etc.
    // Igual que plant() pero siempre aloca en la arena global.
    // Necesario cuando se muta un array global desde dentro de un scope:
    // los nuevos elementos deben vivir en la misma arena que el array.
    fn plant_global(&mut self, value: OwnedValue) -> ObjectRef {
        match value {
            OwnedValue::Integer(i) => {
                let idx = self.global_arena.alloc(ObjectData::Integer(i));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Dec(d) => {
                let idx = self.global_arena.alloc(ObjectData::Dec(d));
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
                let idx = self.global_arena.alloc(ObjectData::Array { element_type, elements: items });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Dict { key_type, value_type, entries } => {
                let idx = self.global_arena.alloc(ObjectData::Dict {
                    key_type,
                    value_type,
                    entries,
                    index: Default::default(),
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
                let idx = self.global_arena.alloc(ObjectData::Set { elements: items, index: Default::default() });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Tensor { shape, data, tid } => {
                let idx = self.global_arena.alloc(ObjectData::Tensor { shape, data, tid });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::DateTime { epoch_ms, utc } => {
                let idx = self.global_arena.alloc(ObjectData::DateTime { epoch_ms, utc });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::DateField { epoch_ms, utc, field, value } => {
                let idx = self.global_arena.alloc(ObjectData::DateField { epoch_ms, utc, field, value });
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
        entries: Vec<(OwnedValue, OwnedValue)>,
    ) {
        let data = ObjectData::Dict { key_type, value_type, entries, index: Default::default() };
        match obj_ref.region {
            RegionId::Global => self.global_arena.update(obj_ref.index, data),
            RegionId::Scoped => self.scopes.arena.update(obj_ref.index, data),
        }
    }

    fn update_array(&mut self, arr_ref: ObjectRef, element_type: Option<String>, elems: Vec<OwnedValue>) {
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
                for elem in elements {
                    let elem_planted = self.plant(elem);
                    parts.push(self.fmt_value(elem_planted)?);
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
                for (name, cap_ref) in captured.iter() {
                    self.scopes.declare(name.clone(), *cap_ref);
                }
                for (i, param) in parameters.iter().enumerate() {
                    if param.is_rest {
                        let rest_items: Vec<OwnedValue> = arg_vals[i.min(arg_vals.len())..].iter()
                            .cloned()
                            .collect();
                        let rest_ref = self.alloc(ObjectData::Array { element_type: None, elements: rest_items });
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
        (Some(ObjectData::Dec(x)),      Some(ObjectData::Dec(y)))      => x == y,
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

pub(super) fn owned_to_key_str(owned: &OwnedValue) -> String {
    match owned {
        OwnedValue::Str(s) => s.clone(),
        OwnedValue::Integer(i) => i.to_string(),
        OwnedValue::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}

pub(super) fn owned_to_obj_data(owned: &OwnedValue) -> ObjectData {
    match owned {
        OwnedValue::Null => ObjectData::Null,
        OwnedValue::Integer(i) => ObjectData::Integer(*i),
        OwnedValue::Decimal(d) => ObjectData::Decimal(*d),
        OwnedValue::Dec(d) => ObjectData::Dec(*d),
        OwnedValue::Boolean(b) => ObjectData::Boolean(*b),
        OwnedValue::Str(s) => ObjectData::Str(s.clone()),
        OwnedValue::Array { element_type, elements: _ } => {
            ObjectData::Array { element_type: element_type.clone(), elements: Vec::new() }
        }
        OwnedValue::Dict { key_type, value_type, entries: _ } => {
            ObjectData::Dict { key_type: key_type.clone(), value_type: value_type.clone(), entries: Vec::new(), index: Default::default() }
        }
        OwnedValue::Set { elements: _ } => ObjectData::Set { elements: Vec::new(), index: Default::default() },
        OwnedValue::Function { .. } => ObjectData::Null,
        OwnedValue::Instance { class_name, fields: _ } => ObjectData::Instance { class_name: class_name.clone(), fields: Vec::new() },
        OwnedValue::Tensor { shape, data, tid } => ObjectData::Tensor { shape: shape.clone(), data: data.clone(), tid: *tid },
        OwnedValue::DateTime { epoch_ms, utc } => ObjectData::DateTime { epoch_ms: *epoch_ms, utc: *utc },
        OwnedValue::DateField { epoch_ms, utc, field, value } => ObjectData::DateField { epoch_ms: *epoch_ms, utc: *utc, field: *field, value: *value },
        OwnedValue::EnumVariant { .. } => ObjectData::Null,
        OwnedValue::Ptr(_) => ObjectData::Null,
    }
}

fn type_matches(expected: &str, data: &ObjectData) -> bool {
    match (expected, data) {
        ("int", ObjectData::Integer(_)) => true,
        ("decimal", ObjectData::Decimal(_)) => true,
        ("dec", ObjectData::Dec(_)) => true,
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
        // `x is function` — closures and named functions both report as "function"
        // (matches type_of), so the introspection is symmetric.
        ("function", ObjectData::Function { .. }) => true,
        ("DateTime", ObjectData::DateTime { .. }) => true,
        // A DateField behaves as an int, so it satisfies an `int` parameter.
        ("int", ObjectData::DateField { .. }) => true,
        ("DateTime", ObjectData::DateField { .. }) => false,
        _ => false,
    }
}

// ── JSON helpers ────────────────────────────────────────────────────────────

fn json_stringify_owned(val: &OwnedValue) -> String {
    match val {
        OwnedValue::Null => "null".to_string(),
        OwnedValue::Boolean(b) => b.to_string(),
        OwnedValue::Integer(i) => i.to_string(),
        // Exact decimal serializes as a JSON number literal preserving scale.
        OwnedValue::Dec(d) => d.to_string(),
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
        // A DateTime serializes as an ISO 8601 string; a DateField as its int.
        OwnedValue::DateTime { epoch_ms, utc } => {
            format!("\"{}\"", crate::region::format_datetime(*epoch_ms, *utc))
        }
        OwnedValue::DateField { value, .. } => format!("{}", value),
        OwnedValue::Function { .. } => "null".to_string(),
        OwnedValue::Ptr(name) => format!("\"&{}\"", name),
    }
}

// Pretty-print an OwnedValue as indented JSON. `indent` is the number of
// spaces per nesting level; an indent of 0 falls back to compact output.
fn json_pretty_owned(val: &OwnedValue, indent: usize) -> String {
    if indent == 0 {
        return json_stringify_owned(val);
    }
    json_pretty_inner(val, indent, 0)
}

fn json_pretty_inner(val: &OwnedValue, indent: usize, level: usize) -> String {
    match val {
        OwnedValue::Array { elements, .. } | OwnedValue::Set { elements } => {
            if elements.is_empty() { return "[]".to_string(); }
            let pad = " ".repeat(indent * (level + 1));
            let pad_close = " ".repeat(indent * level);
            let parts: Vec<String> = elements
                .iter()
                .map(|e| format!("{}{}", pad, json_pretty_inner(e, indent, level + 1)))
                .collect();
            format!("[\n{}\n{}]", parts.join(",\n"), pad_close)
        }
        OwnedValue::Dict { entries, .. } => {
            if entries.is_empty() { return "{}".to_string(); }
            let pad = " ".repeat(indent * (level + 1));
            let pad_close = " ".repeat(indent * level);
            let parts: Vec<String> = entries.iter().map(|(k, v)| {
                let key = match k {
                    OwnedValue::Str(s) => format!("\"{}\"", s.replace('"', "\\\"")),
                    OwnedValue::Integer(i) => format!("\"{}\"", i),
                    other => format!("\"{}\"", other.display_str()),
                };
                format!("{}{}: {}", pad, key, json_pretty_inner(v, indent, level + 1))
            }).collect();
            format!("{{\n{}\n{}}}", parts.join(",\n"), pad_close)
        }
        OwnedValue::Instance { fields, .. } => {
            if fields.is_empty() { return "{}".to_string(); }
            let pad = " ".repeat(indent * (level + 1));
            let pad_close = " ".repeat(indent * level);
            let parts: Vec<String> = fields.iter().map(|(k, v)| {
                format!("{}\"{}\": {}", pad, k.replace('"', "\\\""), json_pretty_inner(v, indent, level + 1))
            }).collect();
            format!("{{\n{}\n{}}}", parts.join(",\n"), pad_close)
        }
        // Scalars (and tensors) have no nested structure to indent.
        _ => json_stringify_owned(val),
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
