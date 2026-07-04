// Hand-maintained language facts for completion/hover. The per-namespace
// method catalog lives in builtins_gen.rs (generated from the evaluator).
pub use super::builtins_gen::{NAMESPACES, VALUE_METHODS};

/// Language keywords, from `token::lookup_ident`.
pub static KEYWORDS: &[&str] = &[
    "fn", "let", "const", "true", "false", "if", "else", "while", "for",
    "return", "out", "break", "continue", "class", "interface", "new",
    "public", "private", "switch", "case", "default", "try", "catch",
    "finally", "throw", "in", "enum", "abstract", "sealed", "get", "set",
    "do", "static", "is", "unsafe", "sizeof", "native", "import", "export",
    "yield", "match", "use", "null",
];

/// Type keywords usable in annotations.
pub static TYPE_KEYWORDS: &[&str] = &[
    "int", "decimal", "dec", "string", "bool", "any", "void",
];

/// Global builtin functions dispatched by name in the evaluator
/// (expr.rs Call fast path), with a short signature for hover.
pub static BUILTIN_FUNCTIONS: &[(&str, &str)] = &[
    ("parseInt", "parseInt(value) -> int"),
    ("parseDecimal", "parseDecimal(value) -> decimal"),
    ("readLine", "readLine() -> string"),
    ("fetch", "fetch(url, options?) -> dict"),
    ("assert", "assert(condition, message?)"),
    ("type_of", "type_of(value) -> string"),
    ("abs", "abs(x)"),
    ("sqrt", "sqrt(x) -> decimal"),
    ("floor", "floor(x) -> int"),
    ("ceil", "ceil(x) -> int"),
    ("round", "round(x) -> int"),
    ("min", "min(a, b)"),
    ("max", "max(a, b)"),
    ("pow", "pow(base, exp)"),
    ("log", "log(x) -> decimal"),
    ("log2", "log2(x) -> decimal"),
    ("log10", "log10(x) -> decimal"),
    ("time", "time() -> int (epoch ms)"),
    ("env", "env(name) -> string?"),
    ("exit", "exit(code)"),
];

/// One-line description per namespace, for hover.
pub static NAMESPACE_DOCS: &[(&str, &str)] = &[
    ("Autodiff", "Diferenciación automática: tape, backward, optimizadores (SGD/Adam), losses"),
    ("Binary", "Empaquetado binario: hex, utf8, enteros LE/BE"),
    ("Crypto", "Criptografía: hashes, HMAC, base64, Ed25519, randomBytes (CSPRNG)"),
    ("DateTime", "Fechas: now/utcNow/from/fromEpoch, campos inmutables, format"),
    ("Dec", "Decimal exacto base-10: parse, fromInt, MAX/MIN"),
    ("Env", "Variables de entorno y argumentos del proceso (permiso Env)"),
    ("File", "Ficheros: read/write/append, binario, stat, listDir (permiso File)"),
    ("GPU", "Buffers y kernels numéricos (backend CPU)"),
    ("Gui", "Ventanas y dibujo 2D: eventos, texto, imágenes, clipboard (permiso Gui)"),
    ("JSON", "parse / stringify / pretty"),
    ("Math", "Constantes y utilidades: PI, E, clamp, random, atan2"),
    ("Memory", "Memoria manual: alloc/free/read/write (requiere unsafe)"),
    ("OS", "Procesos: exec, spawn/tick (async), platform, pid (permiso OS)"),
    ("Random", "Aleatorios: int, decimal, normal, choice, shuffle, seed"),
    ("Regex", "Expresiones regulares (usar raw strings r\"...\"): test, match, findAll, split, replace"),
    ("Socket", "TCP y WebSocket: listen/connect/send/recv (permiso Socket)"),
    ("System", "Información del sistema: cpuCount, memoria, hostname, uptime"),
    ("Task", "Workers nativos share-nothing: run, message, poll (permiso Task)"),
    ("Tensor", "Constructores estáticos de tensores: zeros, ones, eye, from"),
    ("Terminal", "Terminal crudo: raw mode, eventos de teclado/ratón (permiso Terminal)"),
    ("Time", "Reloj y sleep (permiso Time)"),
];

/// Permission names accepted in `use permissions { ... }`.
pub static PERMISSIONS: &[&str] = &[
    "File", "OS", "Env", "Time", "Terminal", "Socket", "Gui", "Task", "System",
];

pub fn is_namespace(name: &str) -> bool {
    NAMESPACES.iter().any(|(ns, _)| *ns == name)
}

pub fn namespace_methods(name: &str) -> Option<&'static [&'static str]> {
    NAMESPACES.iter().find(|(ns, _)| *ns == name).map(|(_, m)| *m)
}

pub fn namespace_doc(name: &str) -> Option<&'static str> {
    NAMESPACE_DOCS.iter().find(|(ns, _)| *ns == name).map(|(_, d)| *d)
}

pub fn builtin_function(name: &str) -> Option<&'static str> {
    BUILTIN_FUNCTIONS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}
