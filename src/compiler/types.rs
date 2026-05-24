/// Compile-time type system for Serez-Code.
///
/// Unlike the interpreter's runtime `ObjectData`, `SzType` is resolved
/// statically during compilation — every node in the HIR and MIR carries
/// one of these variants. LLVM uses them to select the correct IR type.
#[derive(Debug, Clone, PartialEq)]
pub enum SzType {
    /// 64-bit signed integer  →  LLVM i64
    Int,
    /// 64-bit IEEE float      →  LLVM double
    Decimal,
    /// Boolean                →  LLVM i1
    Bool,
    /// Heap-allocated string  →  LLVM { i64, i8* }
    Str,
    /// Null / absence of value
    Null,
    /// No value (function that returns nothing)
    Void,

    /// Homogeneous array      →  LLVM { i64 len, T* ptr }
    Array(Box<SzType>),
    /// Key-value map          →  LLVM { i64 len, Entry* ptr }
    Dict(Box<SzType>, Box<SzType>),

    /// First-class function type
    Function {
        params: Vec<SzType>,
        ret: Box<SzType>,
    },

    /// User-defined class instance  →  LLVM named struct
    Class(String),
    /// Enum variant                 →  LLVM i32 tag
    Enum(String),

    /// Type could not be inferred at compile time (error recovery only)
    Unknown,
}

impl SzType {
    /// Returns true if this type can be stored directly on the LLVM stack
    /// (i.e. has a fixed, known size at compile time).
    pub fn is_primitive(&self) -> bool {
        matches!(self, SzType::Int | SzType::Decimal | SzType::Bool | SzType::Null)
    }

    /// Human-readable name — used in error messages.
    pub fn display(&self) -> String {
        match self {
            SzType::Int => "int".into(),
            SzType::Decimal => "decimal".into(),
            SzType::Bool => "bool".into(),
            SzType::Str => "string".into(),
            SzType::Null => "null".into(),
            SzType::Void => "void".into(),
            SzType::Array(t) => format!("[{}]", t.display()),
            SzType::Dict(k, v) => format!("Dict<{},{}>", k.display(), v.display()),
            SzType::Function { params, ret } => {
                let ps: Vec<String> = params.iter().map(|p| p.display()).collect();
                format!("fn({}) -> {}", ps.join(", "), ret.display())
            }
            SzType::Class(name) => name.clone(),
            SzType::Enum(name) => name.clone(),
            SzType::Unknown => "?".into(),
        }
    }

    /// Parse a type annotation string (from AST) into an SzType.
    pub fn from_annotation(s: &str) -> SzType {
        match s {
            "int"     => SzType::Int,
            "decimal" => SzType::Decimal,
            "bool"    => SzType::Bool,
            "string"  => SzType::Str,
            "void"    => SzType::Void,
            "null"    => SzType::Null,
            other     => SzType::Class(other.to_string()),
        }
    }
}
