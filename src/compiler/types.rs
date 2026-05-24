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

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_primitive ─────────────────────────────────────────────────────────

    #[test]
    fn primitives_fit_on_stack() {
        assert!(SzType::Int.is_primitive());
        assert!(SzType::Decimal.is_primitive());
        assert!(SzType::Bool.is_primitive());
        assert!(SzType::Null.is_primitive());
    }

    #[test]
    fn heap_and_void_types_are_not_primitive() {
        assert!(!SzType::Str.is_primitive());
        assert!(!SzType::Void.is_primitive());
        assert!(!SzType::Unknown.is_primitive());
        assert!(!SzType::Array(Box::new(SzType::Int)).is_primitive());
        assert!(!SzType::Class("Foo".to_string()).is_primitive());
        assert!(!SzType::Enum("Color".to_string()).is_primitive());
        assert!(!SzType::Dict(Box::new(SzType::Str), Box::new(SzType::Int)).is_primitive());
        assert!(!SzType::Function { params: vec![], ret: Box::new(SzType::Void) }.is_primitive());
    }

    // ── display ───────────────────────────────────────────────────────────────

    #[test]
    fn display_scalar_types() {
        assert_eq!(SzType::Int.display(),     "int");
        assert_eq!(SzType::Decimal.display(), "decimal");
        assert_eq!(SzType::Bool.display(),    "bool");
        assert_eq!(SzType::Str.display(),     "string");
        assert_eq!(SzType::Null.display(),    "null");
        assert_eq!(SzType::Void.display(),    "void");
        assert_eq!(SzType::Unknown.display(), "?");
    }

    #[test]
    fn display_named_types() {
        assert_eq!(SzType::Class("Point".to_string()).display(), "Point");
        assert_eq!(SzType::Enum("Color".to_string()).display(),  "Color");
    }

    #[test]
    fn display_array_nested() {
        assert_eq!(SzType::Array(Box::new(SzType::Int)).display(), "[int]");
        assert_eq!(
            SzType::Array(Box::new(SzType::Array(Box::new(SzType::Bool)))).display(),
            "[[bool]]"
        );
    }

    #[test]
    fn display_dict() {
        assert_eq!(
            SzType::Dict(Box::new(SzType::Str), Box::new(SzType::Int)).display(),
            "Dict<string,int>"
        );
    }

    #[test]
    fn display_function_type() {
        let ty = SzType::Function {
            params: vec![SzType::Int, SzType::Bool],
            ret:    Box::new(SzType::Decimal),
        };
        assert_eq!(ty.display(), "fn(int, bool) -> decimal");
    }

    #[test]
    fn display_function_no_params() {
        let ty = SzType::Function { params: vec![], ret: Box::new(SzType::Void) };
        assert_eq!(ty.display(), "fn() -> void");
    }

    // ── from_annotation ───────────────────────────────────────────────────────

    #[test]
    fn from_annotation_builtin_types() {
        assert_eq!(SzType::from_annotation("int"),     SzType::Int);
        assert_eq!(SzType::from_annotation("decimal"), SzType::Decimal);
        assert_eq!(SzType::from_annotation("bool"),    SzType::Bool);
        assert_eq!(SzType::from_annotation("string"),  SzType::Str);
        assert_eq!(SzType::from_annotation("void"),    SzType::Void);
        assert_eq!(SzType::from_annotation("null"),    SzType::Null);
    }

    #[test]
    fn from_annotation_unknown_name_becomes_class() {
        assert_eq!(SzType::from_annotation("Animal"), SzType::Class("Animal".to_string()));
        assert_eq!(SzType::from_annotation("Vec2"),   SzType::Class("Vec2".to_string()));
        assert_eq!(SzType::from_annotation("MyType"), SzType::Class("MyType".to_string()));
    }

    // ── equality ─────────────────────────────────────────────────────────────

    #[test]
    fn same_scalars_are_equal() {
        assert_eq!(SzType::Int,  SzType::Int);
        assert_eq!(SzType::Bool, SzType::Bool);
        assert_eq!(SzType::Void, SzType::Void);
    }

    #[test]
    fn different_scalars_are_not_equal() {
        assert_ne!(SzType::Int,  SzType::Decimal);
        assert_ne!(SzType::Bool, SzType::Int);
        assert_ne!(SzType::Str,  SzType::Null);
    }

    #[test]
    fn arrays_compared_by_element_type() {
        assert_eq!(
            SzType::Array(Box::new(SzType::Int)),
            SzType::Array(Box::new(SzType::Int))
        );
        assert_ne!(
            SzType::Array(Box::new(SzType::Int)),
            SzType::Array(Box::new(SzType::Bool))
        );
    }

    #[test]
    fn classes_compared_by_name() {
        assert_eq!(SzType::Class("A".to_string()), SzType::Class("A".to_string()));
        assert_ne!(SzType::Class("A".to_string()), SzType::Class("B".to_string()));
    }
}
