// ── region.rs ────────────────────────────────────────────────────────────────
// Arena-based memory management module.
// No unsafe or explicit lifetimes — "pointers" are integer indices.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionId {
    Global,
    Scoped,
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectRef {
    pub region: RegionId,
    pub index: usize,
}

use crate::ast::{BlockStatement, Parameter};

#[derive(Debug, Clone)]
pub enum OwnedValue {
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
    Str(String),
    Array {
        element_type: Option<String>,
        elements: Vec<OwnedValue>,
    },
    Dict {
        key_type: String,
        value_type: String,
        entries: Vec<(OwnedValue, OwnedValue)>,
    },
    Function {
        return_type: Option<String>,
        parameters: Vec<Parameter>,
        body: BlockStatement,
        captured: Vec<(String, OwnedValue)>,
    },
    Null,
}

#[derive(Debug, Clone)]
pub enum ObjectData {
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
    Str(String),
    Array {
        element_type: Option<String>,
        elements: Vec<ObjectRef>,
    },
    Dict {
        key_type: String,
        value_type: String,
        entries: Vec<(ObjectRef, ObjectRef)>,
    },
    Function {
        return_type: Option<String>,
        parameters: Vec<Parameter>,
        body: BlockStatement,
        captured: Vec<(String, OwnedValue)>,
    },
    Null,
}

impl std::fmt::Display for ObjectData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectData::Integer(i) => write!(f, "Integer({})", i),
            ObjectData::Decimal(d) => write!(f, "Decimal({})", d),
            ObjectData::Boolean(b) => write!(f, "Boolean({})", b),
            ObjectData::Str(s) => write!(f, "String(\"{}\")", s),
            ObjectData::Array { element_type: Some(t), .. } => write!(f, "[{}]([...])", t),
            ObjectData::Array { .. } => write!(f, "Array([...])"),
            ObjectData::Dict { key_type, value_type, .. } => {
                write!(f, "Dict<{},{}>{{...}}", key_type, value_type)
            }
            ObjectData::Function { .. } => write!(f, "Function"),
            ObjectData::Null => write!(f, "Null"),
        }
    }
}

pub struct Arena {
    storage: Vec<ObjectData>,
}

impl Arena {
    pub fn new() -> Self {
        Arena {
            storage: Vec::new(),
        }
    }

    pub fn alloc(&mut self, data: ObjectData) -> usize {
        let idx = self.storage.len();
        self.storage.push(data);
        idx
    }

    pub fn get(&self, index: usize) -> Option<&ObjectData> {
        self.storage.get(index)
    }

    pub fn update(&mut self, index: usize, data: ObjectData) {
        if let Some(slot) = self.storage.get_mut(index) {
            *slot = data;
        }
    }

    pub fn watermark(&self) -> usize {
        self.storage.len()
    }

    pub fn reset_to(&mut self, mark: usize) {
        self.storage.truncate(mark);
    }
}

impl ObjectData {
    pub fn type_name(&self) -> &str {
        match self {
            ObjectData::Integer(_) => "int",
            ObjectData::Decimal(_) => "decimal",
            ObjectData::Boolean(_) => "bool",
            ObjectData::Str(_) => "string",
            ObjectData::Array { .. } => "array",
            ObjectData::Dict { .. } => "dict",
            ObjectData::Function { .. } => "function",
            ObjectData::Null => "null",
        }
    }
}
