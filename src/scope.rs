// ── scope.rs ──────────────────────────────────────────────────────────────────
// Gestión de ámbitos (scopes) para Serez-Code.
// Implementa una pila de frames donde cada frame posee:
//   - una tabla de bindings (nombre → ObjectRef)
//   - un watermark para resetear la arena al salir del bloque

use std::collections::HashMap;
use crate::region::{Arena, ObjectData, ObjectRef};

/// Un frame de ámbito local: tabla de variables + marca de reset.
pub struct ScopeFrame {
    pub bindings: HashMap<String, ObjectRef>,
    /// Tamaño de la Arena en el momento de entrar a este bloque.
    /// Al hacer pop(), la arena se truncará a este valor.
    pub watermark: usize,
}

/// Pila de ámbitos con una Arena compartida para los datos locales.
///
/// La Arena compartida actúa como un stack implícito:
///   depth 0 aloca en [0..mark0)
///   depth 1 aloca en [mark0..mark1)
///   pop de depth 1 → truncate(mark0) → libera [mark0..mark1)
pub struct ScopeStack {
    frames: Vec<ScopeFrame>,
    /// Arena única para todos los scopes locales.
    pub arena: Arena,
}

impl ScopeStack {
    pub fn new() -> Self {
        ScopeStack {
            frames: Vec::new(),
            arena: Arena::new(),
        }
    }

    /// ¿Hay algún scope activo?
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Entra a un bloque `{ ... }` — guarda el watermark actual.
    pub fn push(&mut self) {
        let mark = self.arena.watermark();
        self.frames.push(ScopeFrame {
            bindings: HashMap::new(),
            watermark: mark,
        });
    }

    /// Sale del bloque — libera toda la memoria alocada en este frame.
    pub fn pop(&mut self) {
        if let Some(frame) = self.frames.pop() {
            self.arena.reset_to(frame.watermark);
        }
    }

    /// Declara una variable nueva en el frame más interno.
    pub fn declare(&mut self, name: String, obj_ref: ObjectRef) {
        if let Some(frame) = self.frames.last_mut() {
            frame.bindings.insert(name, obj_ref);
        }
    }

    /// Reasigna una variable existente en cualquier frame (inner → outer).
    /// Actualiza el dato IN-PLACE en la arena (no aloca nuevo slot).
    /// Retorna `true` si la variable fue encontrada y actualizada.
    pub fn assign(&mut self, name: &str, new_data: ObjectData) -> bool {
        // Buscamos el ObjectRef en los frames (solo lectura de frames)
        let existing_ref = {
            let mut found = None;
            for frame in self.frames.iter().rev() {
                if let Some(&r) = frame.bindings.get(name) {
                    found = Some(r);
                    break;
                }
            }
            found
        };

        // Si encontramos el ref, actualizamos in-place en la arena
        if let Some(r) = existing_ref {
            self.arena.update(r.index, new_data);
            return true;
        }
        false
    }

    /// Busca el ObjectRef de una variable (inner → outer).
    pub fn lookup(&self, name: &str) -> Option<ObjectRef> {
        for frame in self.frames.iter().rev() {
            if let Some(&r) = frame.bindings.get(name) {
                return Some(r);
            }
        }
        None
    }
}
