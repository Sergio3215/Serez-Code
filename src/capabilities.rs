//! Normative Native Operation Capability Registry (DEC-ASYNC-001 / v11.1.0).
//!
//! This module is the single source of truth for the execution capabilities of all
//! native built-in functions and namespace operations in Serez Code.
//!
//! Both the semantic analysis layer (`src/semantic/validate.rs`) and the runtime
//! evaluator (`src/evaluator/`) derive their capability rules from this registry,
//! preventing drift between compilation and execution.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationCapability {
    /// Operation is strictly synchronous. Cannot be preceded by `await`.
    SyncOnly,
    /// Operation supports dual execution paths: synchronous when called without `await`,
    /// and asynchronous when called directly under `await`.
    DualPath,
}

/// Global built-in functions (e.g. `fetch`, `parseInt`).
pub const GLOBAL_BUILTINS: &[(&str, OperationCapability)] = &[
    ("fetch", OperationCapability::DualPath),
    ("parseInt", OperationCapability::SyncOnly),
    ("parseDecimal", OperationCapability::SyncOnly),
    ("readLine", OperationCapability::SyncOnly),
    ("assert", OperationCapability::SyncOnly),
    ("type_of", OperationCapability::SyncOnly),
    ("abs", OperationCapability::SyncOnly),
    ("sqrt", OperationCapability::SyncOnly),
    ("floor", OperationCapability::SyncOnly),
    ("ceil", OperationCapability::SyncOnly),
    ("round", OperationCapability::SyncOnly),
    ("min", OperationCapability::SyncOnly),
    ("max", OperationCapability::SyncOnly),
    ("pow", OperationCapability::SyncOnly),
    ("log", OperationCapability::SyncOnly),
    ("log2", OperationCapability::SyncOnly),
    ("log10", OperationCapability::SyncOnly),
    ("time", OperationCapability::SyncOnly),
    ("env", OperationCapability::SyncOnly),
    ("exit", OperationCapability::SyncOnly),
    ("super", OperationCapability::SyncOnly),
];

/// Known native namespaces and their default operation capability.
/// All current namespace operations in Serez Code v11.1.0 are `SyncOnly`.
/// When a namespace gains dual-path operations in the future (e.g. `Socket.receive`),
/// it is registered in `NAMESPACE_OVERRIDES`.
pub const KNOWN_NAMESPACES: &[&str] = &[
    "JSON", "Math", "Dec", "Time", "DateTime", "System", "OS", "File", "Terminal", "Socket",
    "Task", "Env", "Crypto", "Memory", "Gui", "Media", "Gpu", "Regex", "Autodiff", "Binary",
    "Random",
];

/// Explicit overrides for specific namespace methods when they differ from the
/// namespace default (`SyncOnly`).
pub const NAMESPACE_METHOD_OVERRIDES: &[(&str, &str, OperationCapability)] = &[
    // Future dual-path operations will be listed here, e.g.:
    // ("Socket", "receive", OperationCapability::DualPath),
];

/// Look up the capability of a global built-in function by name.
pub fn lookup_global_capability(name: &str) -> Option<OperationCapability> {
    for &(builtin_name, cap) in GLOBAL_BUILTINS {
        if builtin_name == name {
            return Some(cap);
        }
    }
    None
}

/// Look up the capability of a namespace method call (e.g. `OS.platform`).
pub fn lookup_namespace_capability(namespace: &str, method: &str) -> Option<OperationCapability> {
    for &(ns, m, cap) in NAMESPACE_METHOD_OVERRIDES {
        if ns == namespace && m == method {
            return Some(cap);
        }
    }
    // If the namespace is known and no override exists, it is SyncOnly
    if KNOWN_NAMESPACES.contains(&namespace) {
        return Some(OperationCapability::SyncOnly);
    }
    None
}

/// Returns true if the given global function is dual-path.
pub fn is_dual_path_global(name: &str) -> bool {
    lookup_global_capability(name) == Some(OperationCapability::DualPath)
}

/// Returns true if the given global or namespace operation is sync-only.
pub fn is_sync_only_operation(namespace: Option<&str>, name: &str) -> bool {
    match namespace {
        Some(ns) => lookup_namespace_capability(ns, name) == Some(OperationCapability::SyncOnly),
        None => lookup_global_capability(name) == Some(OperationCapability::SyncOnly),
    }
}
