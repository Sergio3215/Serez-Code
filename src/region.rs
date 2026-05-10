// ── region.rs ────────────────────────────────────────────────────────────────
// Módulo de gestión de memoria por regiones (Arena / Stack Allocator).
// No usa `unsafe` ni lifetimes explícitos — los "punteros" son índices enteros.

/// Indica en qué arena vive un objeto
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionId {
    Global, // Arena global: persiste toda la sesión del REPL
    Scoped, // Arena local: se resetea al salir de un bloque `{ ... }`
}

/// Referencia a un objeto: qué arena + índice dentro de su storage.
/// Equivalente a un puntero, pero seguro (no puede colgar).
#[derive(Debug, Clone, Copy)]
pub struct ObjectRef {
    pub region: RegionId,
    pub index: usize,
}

use crate::ast::{BlockStatement, Parameter};

/// Datos crudos de un valor de Serez-Code.
/// No posee lógica de scope — solo los bytes del valor.
#[derive(Debug, Clone)]
pub enum ObjectData {
    Integer(i64),
    Boolean(bool),
    Str(String),
    Array(Vec<ObjectRef>), // los elementos son refs, no datos inline
    Function {
        return_type: Option<String>,
        parameters: Vec<Parameter>,
        body: BlockStatement,
    },
    Null,
}

impl std::fmt::Display for ObjectData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectData::Integer(i) => write!(f, "Integer({})", i),
            ObjectData::Boolean(b) => write!(f, "Boolean({})", b),
            ObjectData::Str(s) => write!(f, "String(\"{}\")", s),
            ObjectData::Array(_) => write!(f, "Array([...])"),
            ObjectData::Function { .. } => write!(f, "Function"),
            ObjectData::Null => write!(f, "Null"),
        }
    }
}

/// Arena de memoria lineal — bump allocator sobre un Vec<ObjectData>.
///
/// Invariante: los índices son estables mientras no se llame a `reset_to`.
pub struct Arena {
    storage: Vec<ObjectData>,
}

impl Arena {
    pub fn new() -> Self {
        Arena {
            storage: Vec::new(),
        }
    }

    /// Aloca un objeto al final y devuelve su índice (análogo a bump-pointer++).
    pub fn alloc(&mut self, data: ObjectData) -> usize {
        let idx = self.storage.len();
        self.storage.push(data);
        idx
    }

    /// Lectura inmutable por índice.
    pub fn get(&self, index: usize) -> Option<&ObjectData> {
        self.storage.get(index)
    }

    /// Actualización in-place — usado por la reasignación de variables.
    /// Evita dangling refs: el índice existente se mantiene válido.
    pub fn update(&mut self, index: usize, data: ObjectData) {
        if let Some(slot) = self.storage.get_mut(index) {
            *slot = data;
        }
    }

    /// Devuelve el watermark actual (= número de objetos alocados).
    pub fn watermark(&self) -> usize {
        self.storage.len()
    }

    /// Libera todo lo alocado desde `mark` en adelante.
    /// O(k) donde k = número de objetos eliminados (por sus Drops).
    pub fn reset_to(&mut self, mark: usize) {
        self.storage.truncate(mark);
    }
}
impl ObjectData {
    pub fn type_name(&self) -> &str {
        match self {
            ObjectData::Integer(_) => "int",
            ObjectData::Boolean(_) => "bool",
            ObjectData::Str(_) => "string",
            ObjectData::Array(_) => "array",
            ObjectData::Function { .. } => "function",
            ObjectData::Null => "null",
        }
    }
}
