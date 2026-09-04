//! Which names a program's own text can account for, and which it cannot.
//!
//! # Why this module exists
//!
//! Serez resolves names **at run time**. `ScopeStack::lookup` (`src/scope.rs:135`)
//! walks the frame stack inner-to-outer, and calling a function *pushes onto that
//! same stack* rather than starting a fresh environment. So a function body that
//! reads a name it does not declare picks up whatever the **caller** happens to
//! hold. Measured, on the 10.0.0 build:
//!
//! ```text
//! fn int leaky()  { return secret; }
//! fn int caller() { let secret = 42; return leaky(); }
//! out caller();                        // prints 42, exit 0
//!
//! fn int leaky() { return nowhere; }
//! out leaky();                         // SZ4001, exit 1
//! ```
//!
//! The same function is valid or invalid depending on who calls it. `--check`
//! cannot say which, because nothing resolves a name before the program runs.
//! `MATURITY_AUDIT.md` records this as critical and open.
//!
//! # What this module is for, and what it deliberately is not
//!
//! Whether an unresolved name *should* be a diagnostic is
//! **DEC-M4-002** — a product decision this module may not take, because it
//! changes which programs the language accepts. What the decision needs first is
//! a number: how much real code relies on dynamic resolution. This module
//! produces that number and nothing else.
//!
//! So it **reports no diagnostics, has no product consumers, and rejects
//! nothing** — the same shape `semantic::declarations` was introduced in. It
//! answers one question: *walking only lexical structure, which identifier uses
//! cannot be accounted for?*
//!
//! # The bias, stated up front
//!
//! A wrong number is worse input to a decision than no number, and the two
//! directions of wrong are not equally harmful here. Over-reporting would inflate
//! the apparent cost of DEC-M4-002 and argue against a change on false evidence.
//! So every ambiguity resolves toward **"treat it as bound"**, and the result is
//! a *lower bound* on dynamic resolution. Specifically:
//!
//!   * A file containing `import` is not counted at all. Its names may legitimately
//!     come from another module, and this module resolves one file at a time.
//!     Those files are reported separately as unresolvable rather than guessed at.
//!   * Every top-level declaration is visible everywhere in the file, regardless
//!     of position — matching the runtime, where a forward call to a function
//!     declared later works.
//!   * Builtin globals and the runtime namespaces are bound.
//!   * `this` and `super` are bound inside any class body.
//!
//! Nesting, by contrast, is modelled exactly, because nesting is the whole
//! question. Probed against 10.0.0 and matched here:
//!
//!   * A local used before its own `let` is **not** bound (`SZ4001`) — bindings
//!     inside a body take effect from their declaration onward, not block-wide.
//!   * A nested `fn` used before its declaration is **not** bound either; only
//!     top-level declarations are position-independent.
//!   * A closure **does** see the locals of the function enclosing it.

use crate::ast::*;
use crate::semantic::imports;
use crate::span::Span;
use std::collections::HashSet;

/// How a name was used at the site that could not be accounted for.
///
/// Kept apart because the cases carry different weight: a `Read` that resolves
/// into a caller's frame is the silent-capture hazard, while a `Write` to an
/// unaccounted name mutates a binding the function does not own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UseKind {
    /// The name appears as a value.
    Read,
    /// The name is the target of an assignment.
    Write,
    /// The name is called: `name(...)`.
    Call,
    /// The name names a class at a construction site: `new Name(...)`.
    Type,
    /// The name is a class's declared parent: `class X : Name`.
    ///
    /// Split from [`UseKind::Type`] because the two carry different weight.
    /// `new Missing()` already fails at run time with `SZ4001`, so a free `Type`
    /// use is a name that *would* have failed anyway. A free `Parent` is
    /// §5.39's finding: `class Child : Missing` runs to completion as long as
    /// nobody constructs it, so `--check` could not tell you that you inherit
    /// from something that does not exist. `semantic::validate` reports this
    /// kind and only this kind.
    Parent,
}

/// One identifier use that lexical structure cannot account for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeUse {
    pub name: String,
    pub kind: UseKind,
    /// Where the use is written.
    pub span: Span,
    /// The nearest enclosing named function, method or class, when there is one.
    /// `None` means the use sits at the file's top level, where "free" means
    /// genuinely undeclared rather than caller-dependent.
    pub enclosing: Option<String>,
}

/// What a single file's lexical structure can and cannot account for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeReport {
    /// Uses no enclosing lexical scope accounts for, in source order.
    pub free: Vec<FreeUse>,
    /// The file contains at least one `import`.
    ///
    /// Informational now rather than disqualifying: what decides whether `free`
    /// can be trusted is [`Self::unresolved_imports`], because an import whose
    /// module was read contributes names this walk already knows.
    pub has_imports: bool,
    /// At least one `import` could not be read, so a name in `free` may still
    /// exist at runtime.
    ///
    /// True whenever the file has any import until [`ScopeReport::resolve`] is
    /// called, which is the behaviour every caller had before imports could be
    /// resolved at all.
    pub unresolved_imports: bool,
    /// Every module specifier the file imports, wherever it appears.
    ///
    /// Wherever, not just at the top level: `import` is an ordinary statement
    /// and the corpus puts one inside a lambda —
    /// `test("...", () => { import "lib/x"; ... })` in `unit_export.sz`. It
    /// still lands in the global scope when it runs, so a resolver that only
    /// looked at top-level statements would miss the names and report them as
    /// undeclared. That was measured as three corpus failures and one ecosystem
    /// failure before this list replaced a top-level walk.
    pub import_specs: Vec<String>,
}

impl ScopeReport {
    /// Whether this file's result can be trusted. See `unresolved_imports`.
    pub fn is_conclusive(&self) -> bool {
        !self.unresolved_imports
    }

    /// Account for what the file's imports actually contribute.
    ///
    /// Removing a resolved name from `free` is the same answer as having bound
    /// it in frame 0 before the walk — a free use is reported exactly when the
    /// name is in no frame — and it costs one pass over the tree instead of two.
    pub fn resolve(&mut self, imported: &imports::ImportedNames) {
        self.free.retain(|use_| !imported.contains(&use_.name));
        self.unresolved_imports = !imported.is_complete();
    }
}

/// Every name a run of top-level statements binds.
///
/// One rule, used twice: [`Walker::seed_globals`] binds a file's own
/// declarations with it, and [`crate::semantic::imports`] works out what a
/// module contributes with it. If the two disagreed, a name would be local in
/// one file and unknown in the file that imported it.
pub fn declared_names(statements: &[Statement]) -> Vec<String> {
    let mut names = Vec::new();
    for statement in statements {
        collect_declared(statement, &mut names);
    }
    names
}

/// The single name a declaration binds, where it binds exactly one.
///
/// Destructuring binds several and has no single name, so it is `None` here —
/// `export let [a, b] = ...` is not syntax, and this is only used for the
/// contents of an `export`.
pub fn declared_name(statement: &Statement) -> Option<String> {
    match statement {
        Statement::Export(inner) => declared_name(inner),
        Statement::FunctionDeclaration(f) => Some(f.name.clone()),
        Statement::NativeDeclaration(n) => Some(n.name.clone()),
        Statement::ClassDeclaration(c) => Some(c.name.clone()),
        Statement::InterfaceDeclaration(i) => Some(i.name.clone()),
        Statement::EnumDeclaration(e) => Some(e.name.clone()),
        Statement::Let(l) => Some(l.name.clone()),
        _ => None,
    }
}

fn collect_declared(statement: &Statement, out: &mut Vec<String>) {
    match statement {
        Statement::Export(inner) => collect_declared(inner, out),
        Statement::LetDestructureArray(d) => {
            for slot in d.names.iter().flatten() {
                out.push(slot.clone());
            }
            if let Some(rest) = &d.rest {
                out.push(rest.clone());
            }
        }
        Statement::LetDestructureDict(d) => {
            for (key, alias) in &d.fields {
                out.push(alias.as_ref().unwrap_or(key).clone());
            }
        }
        other => {
            if let Some(name) = declared_name(other) {
                out.push(name);
            }
        }
    }
}

/// Global functions the evaluator intercepts before any variable lookup
/// (`evaluator/expr.rs:296-312`). They are bound in every program.
const BUILTIN_GLOBALS: &[&str] = &[
    "parseInt",
    "parseDecimal",
    "readLine",
    "fetch",
    "super",
    "assert",
    "type_of",
    "abs",
    "sqrt",
    "floor",
    "ceil",
    "round",
    "min",
    "max",
    "pow",
    "log",
    "log2",
    "log10",
    "time",
    "env",
    "exit",
];

/// Classes the evaluator constructs without a user declaration:
/// `Tensor` and `Set` are intercepted in `Expression::New`
/// (`evaluator/expr.rs:1311,1315`) and `Error` in `evaluator/ops.rs:18`.
/// `Tensor` is also a namespace, and appears in both lists on purpose — they
/// are two different reasons for the same name to be bound.
const BUILTIN_CLASSES: &[&str] = &["Tensor", "Set", "Error"];

/// The runtime namespaces, as generated into `lsp/builtins_gen.rs` from the
/// evaluator. All 22 — not the 7 the parser guards, which is DEC-M4-003.
const NAMESPACES: &[&str] = &[
    "Autodiff", "Binary", "Crypto", "DateTime", "Dec", "Env", "File", "GPU", "Gui", "JSON", "Math",
    "Media", "Memory", "OS", "Random", "Regex", "Socket", "System", "Task", "Tensor", "Terminal",
    "Time",
];

/// Every identifier use in `program` that lexical structure cannot account for.
///
/// See the module documentation for what is deliberately treated as bound. The
/// result is a lower bound on dynamic resolution, not an exact count.
pub fn analyze(program: &Program) -> ScopeReport {
    let mut walker = Walker::new();
    walker.seed_globals(&program.statements);
    walker.statements(&program.statements);
    ScopeReport {
        free: walker.free,
        has_imports: walker.has_imports,
        // Until a caller resolves them, every import is an unread one. This is
        // exactly what `is_conclusive` meant before `imports` existed, so a
        // caller that cannot reach the filesystem keeps the old behaviour by
        // doing nothing.
        unresolved_imports: walker.has_imports,
        import_specs: walker.import_specs,
    }
}

struct Walker {
    /// Lexical frames, outermost first. Frame 0 holds the file's top-level names
    /// plus the builtins; a body pushes onto it, so a closure sees what encloses
    /// it and a top-level function sees only frame 0.
    frames: Vec<HashSet<String>>,
    /// Enclosing named functions, methods and classes, innermost last.
    enclosing: Vec<String>,
    free: Vec<FreeUse>,
    has_imports: bool,
    import_specs: Vec<String>,
}

impl Walker {
    fn new() -> Self {
        let mut root: HashSet<String> = HashSet::new();
        for name in BUILTIN_GLOBALS
            .iter()
            .chain(NAMESPACES.iter())
            .chain(BUILTIN_CLASSES.iter())
        {
            root.insert((*name).to_string());
        }
        Walker {
            frames: vec![root],
            enclosing: Vec::new(),
            free: Vec::new(),
            has_imports: false,
            import_specs: Vec::new(),
        }
    }

    /// Top-level declarations, collected before walking.
    ///
    /// Position-independent, because the runtime is: a call to a function
    /// declared further down the file resolves. Inside a body this does not
    /// hold, which is why it is done only here.
    fn seed_globals(&mut self, statements: &[Statement]) {
        for name in declared_names(statements) {
            self.bind(&name);
        }
    }

    // ── scope plumbing ────────────────────────────────────────────────────────

    fn push(&mut self) {
        self.frames.push(HashSet::new());
    }

    fn pop(&mut self) {
        self.frames.pop();
    }

    fn bind(&mut self, name: &str) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name.to_string());
        }
    }

    fn is_bound(&self, name: &str) -> bool {
        self.frames.iter().any(|f| f.contains(name))
    }

    fn use_name(&mut self, name: &str, kind: UseKind, span: Span) {
        if self.is_bound(name) {
            return;
        }
        self.free.push(FreeUse {
            name: name.to_string(),
            kind,
            span,
            enclosing: self.enclosing.last().cloned(),
        });
    }

    /// A body that owns a scope: push, run, pop.
    fn scoped(&mut self, f: impl FnOnce(&mut Self)) {
        self.push();
        f(self);
        self.pop();
    }

    fn block(&mut self, b: &BlockStatement) {
        self.scoped(|w| w.statements(&b.statements));
    }

    /// A function-shaped body: its own frame, `this`, parameters, then the body.
    ///
    /// `this` is bound unconditionally rather than only for instance methods.
    /// A static method reaching for `this` is a separate question, and guessing
    /// at it here would manufacture free uses that are really a different defect.
    fn function_body(&mut self, params: &[Parameter], body: &BlockStatement, bind_this: bool) {
        self.scoped(|w| {
            if bind_this {
                w.bind("this");
                w.bind("super");
            }
            for p in params {
                w.bind(&p.name);
            }
            // Defaults are evaluated in the call's own scope, so they are walked
            // after the parameters are bound — a later default may name an
            // earlier parameter.
            for p in params {
                if let Some(d) = &p.default_value {
                    w.expression(d, UseKind::Read);
                }
            }
            w.statements(&body.statements);
        });
    }

    // ── statements ────────────────────────────────────────────────────────────

    /// Walk a statement sequence, deferring nested function **bodies** to the end
    /// of it.
    ///
    /// # Why the bodies are deferred
    ///
    /// A `fn` declaration binds its name where the statement executes, and that
    /// part is modelled in place — a name used at statement level before its own
    /// declaration is genuinely `SZ4001`. But a function *body* does not run
    /// there. It runs whenever the function is called, by which time every
    /// declaration in the enclosing block may have executed.
    ///
    /// Walking the body in place conflated the two and produced a **false
    /// positive** on mutually recursive nested functions, which are legitimate,
    /// working Serez. Measured against the 10.0.0 binary:
    ///
    /// ```text
    /// fn void outer() {
    ///     fn int a(int n) { if (n == 0) { return 1; } return b(n - 1); }
    ///     fn int b(int n) { if (n == 0) { return 2; } return a(n - 1); }
    ///     out a(3);                                   // prints 2, exit 0
    /// }
    /// ```
    ///
    /// # The case this deliberately stops reporting
    ///
    /// A lexical walker cannot separate the program above from this one, which
    /// fails at run time with `SZ4001` because the call happens before `b`'s
    /// declaration:
    ///
    /// ```text
    /// fn void outer() {
    ///     fn int a() { return b(); }
    ///     out a();                                    // SZ4001
    ///     fn int b() { return 1; }
    /// }
    /// ```
    ///
    /// Telling them apart needs flow analysis, not scope analysis. So this takes
    /// the direction the module header commits to — *every ambiguity resolves
    /// toward "bound"* — and stops reporting the second. That is a false
    /// negative, and it is the only acceptable side to be wrong on now that
    /// `semantic::validate` makes these findings **fatal**: missing a real error
    /// leaves the runtime to catch it exactly as before, while inventing one
    /// rejects a correct program.
    fn statements(&mut self, statements: &[Statement]) {
        let mut deferred: Vec<&FunctionDeclaration> = Vec::new();
        for statement in statements {
            // `export fn` is the same declaration with a wrapper.
            let effective = match statement {
                Statement::Export(inner) => inner.as_ref(),
                other => other,
            };
            match effective {
                Statement::FunctionDeclaration(f) => {
                    self.bind(&f.name);
                    deferred.push(f);
                }
                _ => self.statement(statement),
            }
        }
        for f in deferred {
            self.enclosing.push(f.name.clone());
            self.function_body(&f.function.parameters, &f.function.body, false);
            self.enclosing.pop();
        }
    }

    fn statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Import(spec) => {
                self.has_imports = true;
                self.import_specs.push(spec.clone());
            }
            Statement::UsePermissions(_) => {}

            Statement::Export(inner) => self.statement(inner),

            Statement::Let(l) => {
                // The value is evaluated before the binding exists: a local used
                // before its own `let` is SZ4001, probed against 10.0.0.
                self.expression(&l.value, UseKind::Read);
                self.bind(&l.name);
            }
            Statement::LetDestructureArray(d) => {
                self.expression(&d.value, UseKind::Read);
                for slot in d.names.iter().flatten() {
                    self.bind(slot);
                }
                if let Some(rest) = &d.rest {
                    self.bind(rest);
                }
            }
            Statement::LetDestructureDict(d) => {
                self.expression(&d.value, UseKind::Read);
                for (key, alias) in &d.fields {
                    self.bind(alias.as_ref().unwrap_or(key));
                }
            }

            Statement::Assign(a) => {
                self.expression(&a.value, UseKind::Read);
                self.use_name(&a.name, UseKind::Write, a.span);
            }

            Statement::FunctionDeclaration(f) => {
                // Unreachable in practice: `statements` intercepts every
                // declaration in a sequence so it can defer the body, and a
                // declaration only ever appears in one. Kept correct rather than
                // `unreachable!()` so a future caller that walks a lone statement
                // gets the in-place behaviour instead of a panic.
                self.bind(&f.name);
                self.enclosing.push(f.name.clone());
                self.function_body(&f.function.parameters, &f.function.body, false);
                self.enclosing.pop();
            }
            Statement::NativeDeclaration(n) => self.bind(&n.name),

            Statement::ClassDeclaration(c) => self.class(c),

            Statement::InterfaceDeclaration(i) => self.bind(&i.name),
            Statement::EnumDeclaration(e) => self.bind(&e.name),

            Statement::Block(b) | Statement::Unsafe(b) => self.block(b),

            Statement::Expression(e) => self.expression(e, UseKind::Read),
            Statement::Out(o) => self.expression(&o.value, UseKind::Read),
            Statement::Return(r) => self.expression(&r.return_value, UseKind::Read),
            Statement::Throw(e) | Statement::Yield(e) => self.expression(e, UseKind::Read),

            Statement::While(w) | Statement::DoWhile(w) => {
                self.expression(&w.condition, UseKind::Read);
                self.block(&w.body);
            }

            Statement::For(f) => self.scoped(|w| {
                // The init value is evaluated before the loop variable exists.
                w.expression(&f.init.value, UseKind::Read);
                w.bind(&f.init.name);
                w.expression(&f.condition, UseKind::Read);
                w.expression(&f.update.value, UseKind::Read);
                w.use_name(&f.update.name, UseKind::Write, f.update.span);
                w.block(&f.body);
            }),

            Statement::ForEach(f) => {
                // The iterable is evaluated in the enclosing scope.
                self.expression(&f.iterable, UseKind::Read);
                self.scoped(|w| {
                    match &f.var {
                        ForEachVar::Name(n) => w.bind(n),
                        ForEachVar::Array(slots, rest) => {
                            for slot in slots.iter().flatten() {
                                w.bind(slot);
                            }
                            if let Some(rest) = rest {
                                w.bind(rest);
                            }
                        }
                    }
                    w.block(&f.body);
                });
            }

            Statement::Switch(s) => {
                self.expression(&s.value, UseKind::Read);
                for case in &s.cases {
                    for v in &case.values {
                        self.expression(v, UseKind::Read);
                    }
                    self.block(&case.body);
                }
                if let Some(d) = &s.default {
                    self.block(d);
                }
            }

            Statement::Try(t) => {
                self.block(&t.body);
                if let Some(catch_body) = &t.catch_body {
                    self.scoped(|w| {
                        if let Some(v) = &t.catch_var {
                            w.bind(v);
                        }
                        w.statements(&catch_body.statements);
                    });
                }
                if let Some(f) = &t.finally_body {
                    self.block(f);
                }
            }

            Statement::IndexAssign(a) => {
                self.expression(&a.target, UseKind::Read);
                self.expression(&a.index, UseKind::Read);
                self.expression(&a.value, UseKind::Read);
            }
            Statement::FieldAssign(a) => {
                self.use_name(&a.object, UseKind::Read, a.span);
                self.expression(&a.value, UseKind::Read);
            }
            Statement::NestedFieldAssign(a) => {
                self.expression(&a.object, UseKind::Read);
                self.expression(&a.value, UseKind::Read);
            }
            Statement::DerefAssign { ptr, value } => {
                self.expression(ptr, UseKind::Read);
                self.expression(value, UseKind::Read);
            }

            Statement::Break
            | Statement::Continue
            | Statement::BreakLabel(_)
            | Statement::ContinueLabel(_) => {}
        }
    }

    fn class(&mut self, c: &ClassDeclaration) {
        self.bind(&c.name);
        if let Some(parent) = &c.parent {
            self.use_name(parent, UseKind::Parent, c.span);
        }
        self.enclosing.push(c.name.clone());

        for field in &c.fields {
            if let Some(default) = &field.default_value {
                self.scoped(|w| {
                    w.bind("this");
                    w.expression(default, UseKind::Read);
                });
            }
        }
        if let Some(ctor) = &c.constructor {
            self.function_body(&ctor.parameters, &ctor.body, true);
        }
        for method in &c.methods {
            self.function_body(&method.parameters, &method.body, true);
        }

        self.enclosing.pop();
    }

    // ── expressions ───────────────────────────────────────────────────────────

    fn expression(&mut self, expression: &Expression, kind: UseKind) {
        match expression {
            Expression::Identifier { name, span } => self.use_name(name, kind, *span),

            Expression::Integer { .. }
            | Expression::Decimal { .. }
            | Expression::Dec { .. }
            | Expression::String { .. }
            | Expression::Boolean { .. }
            | Expression::Null { .. } => {}

            Expression::Prefix { right, .. } => self.expression(right, UseKind::Read),
            // `x is int` parses as `Infix("is", x, Identifier("int"))`
            // (`parser/expressions.rs:555`). The right side is a *type name*,
            // not a value the program reads, so walking it as one manufactures a
            // free use for every type keyword in the corpus — which is exactly
            // what the first run of the measurement reported, at scale.
            Expression::Infix(i) if i.operator == "is" => self.expression(&i.left, UseKind::Read),
            Expression::Infix(i) => {
                self.expression(&i.left, UseKind::Read);
                self.expression(&i.right, UseKind::Read);
            }
            Expression::Ternary(t) => {
                self.expression(&t.condition, UseKind::Read);
                self.expression(&t.then_expr, UseKind::Read);
                self.expression(&t.else_expr, UseKind::Read);
            }

            Expression::ArrayLiteral(a) => {
                for e in &a.elements {
                    self.expression(e, UseKind::Read);
                }
            }
            Expression::DictLiteral(d) => {
                for (k, v) in &d.entries {
                    self.expression(k, UseKind::Read);
                    self.expression(v, UseKind::Read);
                }
            }
            Expression::EntryLiteral { key, value, .. } => {
                self.expression(key, UseKind::Read);
                self.expression(value, UseKind::Read);
            }
            Expression::ObjectPatch { fields, .. } => {
                for (_, v) in fields {
                    self.expression(v, UseKind::Read);
                }
            }
            Expression::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let StringPart::Expr(e) = part {
                        self.expression(e, UseKind::Read);
                    }
                }
            }

            // A closure: its frame sits on top of the enclosing one, so it sees
            // enclosing locals. Probed — `let x = 1; let f = () => x;` returns 1.
            Expression::FunctionLiteral(f) => self.function_body(&f.parameters, &f.body, false),
            Expression::Lambda(l) => self.scoped(|w| {
                for p in &l.params {
                    w.bind(p);
                }
                match &l.body {
                    LambdaBody::Block(b) => w.statements(&b.statements),
                    LambdaBody::Expr(e) => w.expression(e, UseKind::Read),
                }
            }),

            Expression::Call(c) => {
                self.expression(&c.function, UseKind::Call);
                for a in &c.arguments {
                    self.expression(a, UseKind::Read);
                }
            }
            Expression::DotCall(d) => {
                self.expression(&d.object, UseKind::Read);
                for a in &d.arguments {
                    self.expression(a, UseKind::Read);
                }
            }
            Expression::New(n) => {
                self.use_name(&n.class_name, UseKind::Type, n.span);
                match &n.args {
                    NewArgs::Positional(args) => {
                        for a in args {
                            self.expression(a, UseKind::Read);
                        }
                    }
                    NewArgs::Fields(fields) => {
                        for (_, v) in fields {
                            self.expression(v, UseKind::Read);
                        }
                    }
                }
            }

            Expression::Index(i) => {
                self.expression(&i.left, UseKind::Read);
                self.expression(&i.index, UseKind::Read);
            }

            Expression::If(i) => {
                self.expression(&i.condition, UseKind::Read);
                self.block(&i.consequence);
                if let Some(alt) = &i.alternative {
                    self.block(alt);
                }
            }

            Expression::Match(m) => {
                self.expression(&m.subject, UseKind::Read);
                for arm in &m.arms {
                    self.scoped(|w| {
                        w.pattern(&arm.pattern);
                        if let Some(g) = &arm.guard {
                            w.expression(g, UseKind::Read);
                        }
                        w.statements(&arm.body.statements);
                    });
                }
            }

            Expression::UnsafeBlock(b) => self.block(b),

            Expression::Spread { value, .. }
            | Expression::AddressOf { value, .. }
            | Expression::Deref { value, .. } => self.expression(value, UseKind::Read),

            Expression::SizeOf { target, .. } => {
                if let SizeOfTarget::Expr(e) = target {
                    self.expression(e, UseKind::Read);
                }
            }
        }
    }

    /// A pattern binds names rather than reading them.
    ///
    /// `Literal` is walked as a read: the parser turns a bare identifier into
    /// `Binding`, so a `Literal` holding one would be a case worth seeing rather
    /// than silently dropping.
    fn pattern(&mut self, pattern: &MatchPattern) {
        match pattern {
            MatchPattern::Wildcard => {}
            MatchPattern::Binding(name) => self.bind(name),
            MatchPattern::Literal(e) => self.expression(e, UseKind::Read),
            MatchPattern::Or(patterns) => {
                for p in patterns {
                    self.pattern(p);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn report(source: &str) -> ScopeReport {
        let mut parser = Parser::new(Lexer::new(source.to_string()));
        let program = parser.parse_program();
        analyze(&program)
    }

    fn free_names(source: &str) -> Vec<String> {
        let mut names: Vec<String> = report(source).free.into_iter().map(|u| u.name).collect();
        names.sort();
        names.dedup();
        names
    }

    #[test]
    fn a_function_reading_its_callers_local_is_free() {
        // The hazard itself. Probed against 10.0.0: this program prints 42.
        let names = free_names(
            r#"
            fn int leaky() { return secret; }
            fn int caller() { let secret = 42; return leaky(); }
            out caller();
        "#,
        );
        assert_eq!(names, vec!["secret"]);
    }

    #[test]
    fn a_top_level_declaration_is_visible_before_it_is_written() {
        // Matches the runtime: a forward call resolves.
        assert!(free_names("fn int a() { return b(); } fn int b() { return 1; }").is_empty());
    }

    #[test]
    fn mutually_recursive_nested_functions_are_bound() {
        // The false positive that had to go before `semantic::validate` could
        // make these findings fatal. Legitimate, working Serez —
        // `tests/unit_functions_adv.sz` runs exactly this and asserts the
        // results — and the old model reported `isOdd` free because it walked
        // `isEven`'s body at the point of declaration.
        assert!(
            free_names(
                "fn void outer() {
                     fn bool isEven(int n) { if (n == 0) { return true; } return isOdd(n - 1); }
                     fn bool isOdd(int n) { if (n == 0) { return false; } return isEven(n - 1); }
                     out isEven(4);
                 }"
            )
            .is_empty()
        );
    }

    #[test]
    fn a_nested_function_still_has_to_be_declared_somewhere() {
        // The positive control. Deferring bodies must not mean a body can reach
        // anything at all — a name declared in no enclosing scope is still free.
        assert_eq!(
            free_names("fn void outer() { fn int a() { return nowhere(); } out a(); }"),
            vec!["nowhere".to_string()]
        );
    }

    #[test]
    fn a_nested_function_called_before_its_declaration_is_still_free() {
        // The other control, and the one that proves the fix did not simply
        // switch the check off: the *call site* is still position-dependent,
        // because it is a statement and statements run in order. Probed against
        // 10.0.0 — this is `SZ4001: Variable not found: later`.
        assert_eq!(
            free_names("fn void outer() { out later(); fn int later() { return 1; } }"),
            vec!["later".to_string()]
        );
    }

    #[test]
    fn a_local_used_before_its_own_let_is_free() {
        // Matches the runtime: SZ4001.
        assert_eq!(
            free_names("fn void f() { out later; let later = 3; }"),
            vec!["later"]
        );
    }

    #[test]
    fn a_closure_sees_the_locals_of_the_function_around_it() {
        assert!(free_names("fn int o() { let x = 1; let f = () => x; return f(); }").is_empty());
    }

    #[test]
    fn parameters_and_this_are_bound() {
        assert!(
            free_names("class C { public C(int n) { this.n = n; } public int g() { return 1; } }")
                .is_empty()
        );
    }

    #[test]
    fn loop_catch_and_match_bindings_are_bound() {
        assert!(
            free_names("fn void f() { for (let i = 0; i < 3; i = i + 1) { out i; } }").is_empty()
        );
        assert!(
            free_names("fn void f() { let xs = [1, 2]; for (let x in xs) { out x; } }").is_empty()
        );
        assert!(free_names("fn void f() { try { out 1; } catch (e) { out e; } }").is_empty());
        assert!(free_names("fn int f(int v) { return match v { n => { out n; } }; }").is_empty());
    }

    #[test]
    fn builtins_and_namespaces_are_bound() {
        assert!(free_names("out abs(-1); out Math.floor(3.7); out type_of(1);").is_empty());
        assert!(free_names("let s = new Set(); let t = new Tensor([1]);").is_empty());
    }

    #[test]
    fn the_right_side_of_is_is_a_type_not_a_read() {
        // Found by the corpus measurement, which reported `int`, `string`,
        // `bool`, `decimal`, `array`, `dec`, `any` and `null` as free uses.
        assert!(free_names("fn void f(any x) { out (x is int); out (x is string); }").is_empty());
    }

    #[test]
    fn a_file_with_an_import_is_marked_inconclusive() {
        let r = report("import \"std/math\"; out helper();");
        assert!(r.has_imports);
        assert!(!r.is_conclusive());
    }

    #[test]
    fn a_use_records_how_it_was_used_and_what_encloses_it() {
        let r = report("fn int f() { return ghost; }");
        assert_eq!(r.free.len(), 1);
        assert_eq!(r.free[0].kind, UseKind::Read);
        assert_eq!(r.free[0].enclosing.as_deref(), Some("f"));
        assert!(r.free[0].span.line > 0);
    }

    #[test]
    fn an_assignment_to_an_unaccounted_name_is_a_write() {
        let r = report("fn void f() { ghost = 1; }");
        assert_eq!(r.free.len(), 1);
        assert_eq!(r.free[0].kind, UseKind::Write);
    }

    #[test]
    fn a_destructured_binding_is_bound() {
        assert!(free_names("fn void f() { let [a, b] = [1, 2]; out a; out b; }").is_empty());
    }
}
