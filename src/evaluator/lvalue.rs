//! Rutas de escritura (lvalue paths) para receptores anidados.
//!
//! El intérprete tiene semántica de VALOR: leer `a[i]`, `o.campo` o
//! `this.celdas[i]` PLANTA UNA COPIA en un slot nuevo del arena. Para las
//! colecciones eso ya estaba resuelto caso por caso — `apply_field_writeback`
//! cubre `inst.campo.push(x)`, `apply_dict_writeback` cubre `d["k"].push(x)` —
//! pero siempre a UN nivel y sólo para una lista fija de métodos built-in.
//!
//! Lo que faltaba era el caso general: un método PROPIO llamado sobre un
//! receptor que no es una variable suelta.
//!
//! ```text
//! lista.push(new Celda())
//! lista[0].correr()      // mutaba la copia
//! out lista[0].veces     // 0 — no pasó nada
//! ```
//!
//! Este módulo aporta las dos mitades que faltaban:
//!
//! 1. `resolve_lvalue_path` — camina la expresión del receptor y devuelve la
//!    variable raíz más la cadena de saltos (`.campo` / `[clave]`) que hay que
//!    recorrer para volver a encontrar ese valor. Sólo las formas que de verdad
//!    persisten: una variable, lecturas de campo sin paréntesis e indexados. Un
//!    temporal (`getLista()[0]`) no produce ruta, y entonces no hay writeback —
//!    exactamente el comportamiento de hoy.
//! 2. `store_path` — guarda un valor en esa ruta, descendiendo por los
//!    contenedores hasta la hoja. Un solo `get_mut` sobre el slot raíz: no
//!    reconstruye ni re-clona los contenedores intermedios.
//!
//! Y `method_mutates_self`, el análisis estático que decide si vale la pena:
//! el writeback copia el receptor de vuelta a su contenedor, así que se paga
//! sólo cuando el método puede escribir en `this`.

use super::{ExecutionFlow, obj_data_to_key_str};
use crate::ast::{self, Expression};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};

/// Un salto desde la variable raíz hacia el slot anidado.
#[derive(Debug, Clone)]
pub(super) enum PathStep {
    /// `.nombre` — un campo de instancia.
    Field(String),
    /// `[clave]` — índice de array o clave de dict, ya evaluada.
    Key(OwnedValue),
}

impl super::Evaluator {
    /// Camina el receptor y devuelve `(raíz, saltos)` si la forma es escribible.
    ///
    /// Evalúa las expresiones de índice, así que tiene el mismo efecto de doble
    /// evaluación que ya tenía `dict_slot_ctx` (que también re-evalúa la clave
    /// después del receptor). Por eso se llama SÓLO cuando hace falta escribir:
    /// un método que no toca `this` nunca pasa por acá.
    pub(super) fn resolve_lvalue_path(
        &mut self,
        expr: &Expression,
    ) -> Option<(ObjectRef, Vec<PathStep>)> {
        match expr {
            Expression::Identifier { name, .. } => self.lookup_var(name).map(|r| (r, Vec::new())),

            // Lectura de campo: sin paréntesis y sin argumentos. Con paréntesis
            // es una llamada, y su resultado es un temporal que no persiste.
            Expression::DotCall(dc) if dc.arguments.is_empty() && !dc.has_parens => {
                let (root, mut steps) = self.resolve_lvalue_path(&dc.object)?;
                steps.push(PathStep::Field(dc.method.clone()));
                Some((root, steps))
            }

            Expression::Index(ix) => {
                let (root, mut steps) = self.resolve_lvalue_path(&ix.left)?;
                let key_ref = match self.eval_expression(&ix.index) {
                    Ok(ExecutionFlow::Value(r)) => r,
                    _ => return None,
                };
                // Un DateField indexa por su valor entero, igual que en IndexAssign.
                let key = match self.extract(key_ref) {
                    OwnedValue::DateField { value, .. } => OwnedValue::Integer(value),
                    other => other,
                };
                steps.push(PathStep::Key(key));
                Some((root, steps))
            }

            _ => None,
        }
    }

    /// Escribe `value` en `raíz + saltos`. Devuelve false si la ruta no existe
    /// (un campo que en realidad era un getter, un índice fuera de rango, una
    /// clave ausente): en ese caso no se escribe nada, que es lo que pasaba
    /// antes de tener writeback.
    pub(super) fn store_path(
        &mut self,
        root: ObjectRef,
        steps: &[PathStep],
        value: OwnedValue,
    ) -> bool {
        if steps.is_empty() {
            // Sin saltos la "ruta" es la variable misma; ya está escrita.
            return true;
        }
        let arena = match root.region {
            RegionId::Global => &mut self.global_arena,
            RegionId::Scoped => &mut self.scopes.arena,
        };
        let Some(data) = arena.get_mut(root.index) else {
            return false;
        };

        // El primer salto sale de un ObjectData (el slot del arena); los
        // siguientes ya viajan por OwnedValue puro.
        let Some(mut cur) = obj_child_mut(data, &steps[0]) else {
            return false;
        };
        for step in &steps[1..] {
            cur = match owned_child_mut(cur, step) {
                Some(next) => next,
                None => return false,
            };
        }
        *cur = value;
        true
    }

    /// Mira, sin escribir, qué contenedor hay al final de `raíz + saltos`.
    /// Lo usa la asignación indexada anidada para validar índice y tipo con los
    /// mismos mensajes que el camino directo, antes de tocar nada.
    pub(super) fn peek_path_container(
        &self,
        root: ObjectRef,
        steps: &[PathStep],
    ) -> Option<PathContainer> {
        let data = self.resolve(root)?;
        if steps.is_empty() {
            return match data {
                ObjectData::Array {
                    element_type,
                    elements,
                } => Some(PathContainer::Array(element_type.clone(), elements.len())),
                ObjectData::Dict {
                    key_type,
                    value_type,
                    ..
                } => Some(PathContainer::Dict(key_type.clone(), value_type.clone())),
                _ => None,
            };
        }
        let mut cur = obj_child(data, &steps[0])?;
        for step in &steps[1..] {
            cur = owned_child(cur, step)?;
        }
        match cur {
            OwnedValue::Array {
                element_type,
                elements,
            } => Some(PathContainer::Array(element_type.clone(), elements.len())),
            OwnedValue::Dict {
                key_type,
                value_type,
                ..
            } => Some(PathContainer::Dict(key_type.clone(), value_type.clone())),
            _ => None,
        }
    }

    /// Escribe `clave → valor` en el contenedor que hay al final de la ruta.
    /// A diferencia de `store_path`, la hoja acá es el CONTENEDOR y no el valor:
    /// por eso un dict puede INSERTAR una clave nueva, que es lo que se espera
    /// de `m["a"]["nueva"] = v`.
    ///
    /// `steps` nunca viene vacío: una ruta sin saltos es una variable suelta, y
    /// ésa la resuelve el camino directo mutando su slot en su lugar.
    pub(super) fn store_index_at_path(
        &mut self,
        root: ObjectRef,
        steps: &[PathStep],
        key: &OwnedValue,
        value: OwnedValue,
    ) -> bool {
        if steps.is_empty() {
            return false;
        }
        let arena = match root.region {
            RegionId::Global => &mut self.global_arena,
            RegionId::Scoped => &mut self.scopes.arena,
        };
        let Some(data) = arena.get_mut(root.index) else {
            return false;
        };
        let Some(mut cur) = obj_child_mut(data, &steps[0]) else {
            return false;
        };
        for step in &steps[1..] {
            cur = match owned_child_mut(cur, step) {
                Some(next) => next,
                None => return false,
            };
        }
        match cur {
            OwnedValue::Array { elements, .. } => {
                let OwnedValue::Integer(i) = key else {
                    return false;
                };
                let i = *i;
                if i < 0 || i as usize >= elements.len() {
                    return false;
                }
                elements[i as usize] = value;
                true
            }
            OwnedValue::Dict { entries, .. } => {
                let want = owned_key_str(key);
                for entry in entries.iter_mut() {
                    if owned_key_str(&entry.0) == want {
                        entry.1 = value;
                        return true;
                    }
                }
                entries.push((OwnedValue::Str(want), value));
                true
            }
            _ => false,
        }
    }

    /// ¿El método `m` de `class_name` puede escribir en `this`?
    ///
    /// Decide si un writeback vale la pena. Es CONSERVADOR: ante la duda dice
    /// que sí, porque un falso positivo cuesta una copia y un falso negativo
    /// pierde la mutación. El resultado se cachea por (clase, método): el
    /// recorrido del cuerpo se hace una sola vez por método, no por llamada.
    pub(super) fn method_mutates_self(&mut self, class_name: &str, m: &ast::ClassMethod) -> bool {
        let key = (class_name.to_string(), m.name.clone());
        if let Some(&hit) = self.dispatch.mutator.get(&key) {
            return hit;
        }
        let mut found = false;
        writes_self_block(&m.body, &mut found);
        self.dispatch.mutator.insert(key, found);
        found
    }
}

/// Qué contenedor hay al final de una ruta, con su tipo declarado.
pub(super) enum PathContainer {
    /// `(tipo de elemento, cantidad)`
    Array(Option<String>, usize),
    /// `(tipo de clave, tipo de valor)`
    Dict(String, String),
}

/// Primer salto en sólo lectura, para `peek_path_container`.
fn obj_child<'a>(data: &'a ObjectData, step: &PathStep) -> Option<&'a OwnedValue> {
    match (data, step) {
        (ObjectData::Instance { fields, .. }, PathStep::Field(name)) => {
            fields.iter().find(|(k, _)| k == name).map(|(_, v)| v)
        }
        (ObjectData::Array { elements, .. }, PathStep::Key(OwnedValue::Integer(i))) => {
            if *i < 0 {
                return None;
            }
            elements.get(*i as usize)
        }
        (ObjectData::Dict { entries, .. }, PathStep::Key(k)) => {
            let want = owned_key_str(k);
            entries
                .iter()
                .find(|(ek, _)| owned_key_str(ek) == want)
                .map(|(_, v)| v)
        }
        // DEC-M12-001, in the path walker: a step written with a dot is a
        // *field* step, but what it means still depends on the receiver it
        // lands on. On a dict it is the key of that name, exactly as
        // `PathStep::Key` would be — which is what makes `dic.user.name = v`
        // reach the same slot as `dic["user"]["name"] = v`.
        //
        // Without this the walk simply stopped: `store_path` returned false and
        // the caller reported that the path did not exist, on a path that did.
        (ObjectData::Dict { entries, .. }, PathStep::Field(name)) => entries
            .iter()
            .find(|(ek, _)| owned_key_str(ek) == *name)
            .map(|(_, v)| v),
        _ => None,
    }
}

/// Saltos siguientes en sólo lectura.
fn owned_child<'a>(v: &'a OwnedValue, step: &PathStep) -> Option<&'a OwnedValue> {
    match (v, step) {
        (OwnedValue::Instance { fields, .. }, PathStep::Field(name)) => {
            fields.iter().find(|(k, _)| k == name).map(|(_, v)| v)
        }
        (OwnedValue::Array { elements, .. }, PathStep::Key(OwnedValue::Integer(i))) => {
            if *i < 0 {
                return None;
            }
            elements.get(*i as usize)
        }
        (OwnedValue::Dict { entries, .. }, PathStep::Key(k)) => {
            let want = owned_key_str(k);
            entries
                .iter()
                .find(|(ek, _)| owned_key_str(ek) == want)
                .map(|(_, v)| v)
        }
        // DEC-M12-001, in the path walker: a step written with a dot is a
        // *field* step, but what it means still depends on the receiver it
        // lands on. On a dict it is the key of that name, exactly as
        // `PathStep::Key` would be — which is what makes `dic.user.name = v`
        // reach the same slot as `dic["user"]["name"] = v`.
        //
        // Without this the walk simply stopped: `store_path` returned false and
        // the caller reported that the path did not exist, on a path that did.
        (OwnedValue::Dict { entries, .. }, PathStep::Field(name)) => entries
            .iter()
            .find(|(ek, _)| owned_key_str(ek) == *name)
            .map(|(_, v)| v),
        _ => None,
    }
}

/// Primer salto: de `ObjectData` (slot del arena) al hijo `OwnedValue`.
fn obj_child_mut<'a>(data: &'a mut ObjectData, step: &PathStep) -> Option<&'a mut OwnedValue> {
    match (data, step) {
        (ObjectData::Instance { fields, .. }, PathStep::Field(name)) => {
            fields.iter_mut().find(|(k, _)| k == name).map(|(_, v)| v)
        }
        (ObjectData::Array { elements, .. }, PathStep::Key(OwnedValue::Integer(i))) => {
            let i = *i;
            if i < 0 {
                return None;
            }
            elements.get_mut(i as usize)
        }
        // Reemplazar un VALOR en su lugar no mueve ninguna clave, así que el
        // índice hash residente en el slot sigue siendo válido.
        (ObjectData::Dict { entries, .. }, PathStep::Key(k)) => {
            let want = owned_key_str(k);
            entries
                .iter_mut()
                .find(|(ek, _)| owned_key_str(ek) == want)
                .map(|(_, v)| v)
        }
        // DEC-M12-001, in the path walker: a step written with a dot is a
        // *field* step, but what it means still depends on the receiver it
        // lands on. On a dict it is the key of that name, exactly as
        // `PathStep::Key` would be — which is what makes `dic.user.name = v`
        // reach the same slot as `dic["user"]["name"] = v`.
        //
        // Without this the walk simply stopped: `store_path` returned false and
        // the caller reported that the path did not exist, on a path that did.
        (ObjectData::Dict { entries, .. }, PathStep::Field(name)) => entries
            .iter_mut()
            .find(|(ek, _)| owned_key_str(ek) == *name)
            .map(|(_, v)| v),
        _ => None,
    }
}

/// Saltos siguientes: de `OwnedValue` a `OwnedValue`.
fn owned_child_mut<'a>(v: &'a mut OwnedValue, step: &PathStep) -> Option<&'a mut OwnedValue> {
    match (v, step) {
        (OwnedValue::Instance { fields, .. }, PathStep::Field(name)) => {
            fields.iter_mut().find(|(k, _)| k == name).map(|(_, v)| v)
        }
        (OwnedValue::Array { elements, .. }, PathStep::Key(OwnedValue::Integer(i))) => {
            let i = *i;
            if i < 0 {
                return None;
            }
            elements.get_mut(i as usize)
        }
        (OwnedValue::Dict { entries, .. }, PathStep::Key(k)) => {
            let want = owned_key_str(k);
            entries
                .iter_mut()
                .find(|(ek, _)| owned_key_str(ek) == want)
                .map(|(_, v)| v)
        }
        // DEC-M12-001, in the path walker: a step written with a dot is a
        // *field* step, but what it means still depends on the receiver it
        // lands on. On a dict it is the key of that name, exactly as
        // `PathStep::Key` would be — which is what makes `dic.user.name = v`
        // reach the same slot as `dic["user"]["name"] = v`.
        //
        // Without this the walk simply stopped: `store_path` returned false and
        // the caller reported that the path did not exist, on a path that did.
        (OwnedValue::Dict { entries, .. }, PathStep::Field(name)) => entries
            .iter_mut()
            .find(|(ek, _)| owned_key_str(ek) == *name)
            .map(|(_, v)| v),
        _ => None,
    }
}

/// Clave canónica de un `OwnedValue`, con la misma normalización que usa el
/// resto del intérprete para las claves de dict.
fn owned_key_str(v: &OwnedValue) -> String {
    obj_data_to_key_str(&owned_as_data(v))
}

/// Sólo los escalares que pueden ser clave; el resto cae en un discriminante
/// que nunca va a matchear (y por lo tanto no escribe nada).
fn owned_as_data(v: &OwnedValue) -> ObjectData {
    match v {
        OwnedValue::Str(s) => ObjectData::Str(s.clone()),
        OwnedValue::Integer(i) => ObjectData::Integer(*i),
        OwnedValue::Boolean(b) => ObjectData::Boolean(*b),
        OwnedValue::Decimal(d) => ObjectData::Decimal(*d),
        OwnedValue::Dec(d) => ObjectData::Dec(*d),
        _ => ObjectData::Null,
    }
}

// ── Análisis estático: ¿el cuerpo escribe en `this`? ──────────────────────────
//
// Mismo estilo de recorrido que `collect_idents_*` en mod.rs: exhaustivo sobre
// las formas que pueden contener una escritura, y descendiendo a los cuerpos de
// lambdas (una closure que escribe en `this` cuenta, aunque corra después).
//
// Cuenta como escritura sobre `this`:
//   · `this.campo = v`                 → FieldAssign con object "this"
//   · `this.items[i] = v`              → IndexAssign sobre una lectura de this
//   · `this.metodo(...)`               → puede mutar; conservador
//   · `this.items.push(x)`             → mutador built-in sobre un campo propio
//   · `this.celda.correr()`            → con writeback, muta el campo
//
// NO cuenta una lectura pura (`this.campo`, `this.items.length()`), que es el
// caso que queremos dejar sin costo.

fn writes_self_block(b: &ast::BlockStatement, found: &mut bool) {
    for s in &b.statements {
        if *found {
            return;
        }
        writes_self_stmt(s, found);
    }
}

fn writes_self_stmt(s: &ast::Statement, found: &mut bool) {
    use ast::Statement as St;
    if *found {
        return;
    }
    match s {
        St::FieldAssign(fa) => {
            if fa.object == "this" {
                *found = true;
                return;
            }
            writes_self_expr(&fa.value, found);
        }
        St::IndexAssign(ia) => {
            if roots_at_this(&ia.target) {
                *found = true;
                return;
            }
            writes_self_expr(&ia.target, found);
            writes_self_expr(&ia.index, found);
            writes_self_expr(&ia.value, found);
        }
        St::Let(l) => writes_self_expr(&l.value, found),
        St::Assign(a) => writes_self_expr(&a.value, found),
        St::Block(b) | St::Unsafe(b) => writes_self_block(b, found),
        St::Return(r) => writes_self_expr(&r.return_value, found),
        St::Expression(e) | St::Throw(e) | St::Yield(e) => writes_self_expr(e, found),
        St::Out(o) => writes_self_expr(&o.value, found),
        St::While(w) | St::DoWhile(w) => {
            writes_self_expr(&w.condition, found);
            writes_self_block(&w.body, found);
        }
        St::For(f) => {
            writes_self_expr(&f.init.value, found);
            writes_self_expr(&f.condition, found);
            writes_self_expr(&f.update.value, found);
            writes_self_block(&f.body, found);
        }
        St::ForEach(fe) => {
            writes_self_expr(&fe.iterable, found);
            writes_self_block(&fe.body, found);
        }
        St::DerefAssign { ptr, value } => {
            writes_self_expr(ptr, found);
            writes_self_expr(value, found);
        }
        _ => {}
    }
}

fn writes_self_expr(e: &ast::Expression, found: &mut bool) {
    use ast::Expression as Ex;
    if *found {
        return;
    }
    match e {
        Ex::DotCall(d) => {
            // Cualquier LLAMADA cuyo receptor arranque en `this` cuenta como
            // escritura. Es deliberadamente grueso — `this.items.length()` queda
            // marcado igual que `this.items.push(x)` — y no cuesta nada: este
            // análisis se consulta SÓLO cuando el receptor de la llamada externa
            // es una ruta anidada (`a[i].m()`, `o.f.m()`). Un `obj.metodo()`
            // sobre una variable suelta, que es el caso masivo, ni lo mira.
            //
            // Afinarlo exigiría una lista blanca de los built-in de sólo lectura
            // (todo string/array/dict), y equivocarse ahí PIERDE una mutación,
            // que es justo el bug que esto viene a cerrar.
            //
            // Lo que sí queda afuera, que es lo que importa: los métodos que sólo
            // LEEN campos (`this.a + this.b`, comparaciones, getters).
            if (d.has_parens || !d.arguments.is_empty()) && roots_at_this(&d.object) {
                *found = true;
                return;
            }
            writes_self_expr(&d.object, found);
            for a in &d.arguments {
                writes_self_expr(a, found);
            }
        }
        Ex::Prefix {
            operator: _,
            right: inner,
            ..
        }
        | Ex::Spread { value: inner, .. }
        | Ex::AddressOf { value: inner, .. }
        | Ex::Deref { value: inner, .. } => writes_self_expr(inner, found),
        Ex::Infix(i) => {
            writes_self_expr(&i.left, found);
            writes_self_expr(&i.right, found);
        }
        Ex::Call(c) => {
            writes_self_expr(&c.function, found);
            for a in &c.arguments {
                writes_self_expr(a, found);
            }
        }
        Ex::Index(ix) => {
            writes_self_expr(&ix.left, found);
            writes_self_expr(&ix.index, found);
        }
        Ex::ArrayLiteral(al) => {
            for el in &al.elements {
                writes_self_expr(el, found);
            }
        }
        Ex::DictLiteral(dl) => {
            for (k, v) in &dl.entries {
                writes_self_expr(k, found);
                writes_self_expr(v, found);
            }
        }
        Ex::EntryLiteral {
            key: k, value: v, ..
        } => {
            writes_self_expr(k, found);
            writes_self_expr(v, found);
        }
        Ex::Ternary(t) => {
            writes_self_expr(&t.condition, found);
            writes_self_expr(&t.then_expr, found);
            writes_self_expr(&t.else_expr, found);
        }
        Ex::If(ife) => {
            writes_self_expr(&ife.condition, found);
            writes_self_block(&ife.consequence, found);
            if let Some(alt) = &ife.alternative {
                writes_self_block(alt, found);
            }
        }
        Ex::InterpolatedString { parts, .. } => {
            for p in parts {
                if let ast::StringPart::Expr(ex) = p {
                    writes_self_expr(ex, found);
                }
            }
        }
        Ex::New(n) => match &n.args {
            ast::NewArgs::Positional(v) => {
                for a in v {
                    writes_self_expr(a, found);
                }
            }
            ast::NewArgs::Fields(f) => {
                for (_, a) in f {
                    writes_self_expr(a, found);
                }
            }
        },
        Ex::Match(m) => {
            writes_self_expr(&m.subject, found);
            for arm in &m.arms {
                if let Some(g) = &arm.guard {
                    writes_self_expr(g, found);
                }
                writes_self_block(&arm.body, found);
            }
        }
        Ex::FunctionLiteral(fl) => writes_self_block(&fl.body, found),
        Ex::Lambda(l) => match &l.body {
            ast::LambdaBody::Block(b) => writes_self_block(b, found),
            ast::LambdaBody::Expr(ex) => writes_self_expr(ex, found),
        },
        Ex::UnsafeBlock(b) => writes_self_block(b, found),
        Ex::ObjectPatch { fields, .. } => {
            for (_, ex) in fields {
                writes_self_expr(ex, found);
            }
        }
        _ => {}
    }
}

// ── ¿El cuerpo de un constructor llama a `super(...)`? ───────────────────────
//
// Lo usa eval_new_class para decidir si encadena al padre por su cuenta. Vive
// acá porque reusa la misma plantilla de recorrido que el análisis de arriba.
// Conservador: con que aparezca en cualquier rama alcanza para no insertar nada.

pub(super) fn calls_super_block(b: &ast::BlockStatement, found: &mut bool) {
    for s in &b.statements {
        if *found {
            return;
        }
        calls_super_stmt(s, found);
    }
}

fn calls_super_stmt(s: &ast::Statement, found: &mut bool) {
    use ast::Statement as St;
    if *found {
        return;
    }
    match s {
        St::Expression(e) | St::Throw(e) | St::Yield(e) => calls_super_expr(e, found),
        St::Let(l) => calls_super_expr(&l.value, found),
        St::Assign(a) => calls_super_expr(&a.value, found),
        St::FieldAssign(fa) => calls_super_expr(&fa.value, found),
        St::NestedFieldAssign(fa) => {
            calls_super_expr(&fa.object, found);
            calls_super_expr(&fa.value, found);
        }
        St::IndexAssign(ia) => {
            calls_super_expr(&ia.target, found);
            calls_super_expr(&ia.index, found);
            calls_super_expr(&ia.value, found);
        }
        St::Block(b) | St::Unsafe(b) => calls_super_block(b, found),
        St::Return(r) => calls_super_expr(&r.return_value, found),
        St::Out(o) => calls_super_expr(&o.value, found),
        St::While(w) | St::DoWhile(w) => {
            calls_super_expr(&w.condition, found);
            calls_super_block(&w.body, found);
        }
        St::For(f) => {
            calls_super_expr(&f.init.value, found);
            calls_super_expr(&f.condition, found);
            calls_super_expr(&f.update.value, found);
            calls_super_block(&f.body, found);
        }
        St::ForEach(fe) => {
            calls_super_expr(&fe.iterable, found);
            calls_super_block(&fe.body, found);
        }
        St::DerefAssign { ptr, value } => {
            calls_super_expr(ptr, found);
            calls_super_expr(value, found);
        }
        _ => {}
    }
}

fn calls_super_expr(e: &ast::Expression, found: &mut bool) {
    use ast::Expression as Ex;
    if *found {
        return;
    }
    match e {
        Ex::Call(c) => {
            if matches!(c.function.as_ref(), Ex::Identifier { name: n, .. } if n == "super") {
                *found = true;
                return;
            }
            calls_super_expr(&c.function, found);
            for a in &c.arguments {
                calls_super_expr(a, found);
            }
        }
        Ex::Prefix {
            operator: _,
            right: inner,
            ..
        }
        | Ex::Spread { value: inner, .. }
        | Ex::AddressOf { value: inner, .. }
        | Ex::Deref { value: inner, .. } => calls_super_expr(inner, found),
        Ex::Infix(i) => {
            calls_super_expr(&i.left, found);
            calls_super_expr(&i.right, found);
        }
        Ex::DotCall(d) => {
            calls_super_expr(&d.object, found);
            for a in &d.arguments {
                calls_super_expr(a, found);
            }
        }
        Ex::Index(ix) => {
            calls_super_expr(&ix.left, found);
            calls_super_expr(&ix.index, found);
        }
        Ex::ArrayLiteral(al) => {
            for el in &al.elements {
                calls_super_expr(el, found);
            }
        }
        Ex::DictLiteral(dl) => {
            for (k, v) in &dl.entries {
                calls_super_expr(k, found);
                calls_super_expr(v, found);
            }
        }
        Ex::EntryLiteral {
            key: k, value: v, ..
        } => {
            calls_super_expr(k, found);
            calls_super_expr(v, found);
        }
        Ex::Ternary(t) => {
            calls_super_expr(&t.condition, found);
            calls_super_expr(&t.then_expr, found);
            calls_super_expr(&t.else_expr, found);
        }
        Ex::If(ife) => {
            calls_super_expr(&ife.condition, found);
            calls_super_block(&ife.consequence, found);
            if let Some(alt) = &ife.alternative {
                calls_super_block(alt, found);
            }
        }
        Ex::InterpolatedString { parts, .. } => {
            for p in parts {
                if let ast::StringPart::Expr(ex) = p {
                    calls_super_expr(ex, found);
                }
            }
        }
        Ex::New(n) => match &n.args {
            ast::NewArgs::Positional(v) => {
                for a in v {
                    calls_super_expr(a, found);
                }
            }
            ast::NewArgs::Fields(f) => {
                for (_, a) in f {
                    calls_super_expr(a, found);
                }
            }
        },
        Ex::Match(m) => {
            calls_super_expr(&m.subject, found);
            for arm in &m.arms {
                if let Some(g) = &arm.guard {
                    calls_super_expr(g, found);
                }
                calls_super_block(&arm.body, found);
            }
        }
        Ex::FunctionLiteral(fl) => calls_super_block(&fl.body, found),
        Ex::Lambda(l) => match &l.body {
            ast::LambdaBody::Block(b) => calls_super_block(b, found),
            ast::LambdaBody::Expr(ex) => calls_super_expr(ex, found),
        },
        Ex::UnsafeBlock(b) => calls_super_block(b, found),
        Ex::ObjectPatch { fields, .. } => {
            for (_, ex) in fields {
                calls_super_expr(ex, found);
            }
        }
        _ => {}
    }
}

/// ¿La cadena de lecturas (`.campo`, `[i]`) arranca en `this`? Vale también
/// para `this` a secas, que es el receptor de `this.metodo(...)`.
fn roots_at_this(e: &Expression) -> bool {
    match e {
        Expression::Identifier { name: n, .. } => n == "this",
        Expression::DotCall(d) if d.arguments.is_empty() && !d.has_parens => {
            roots_at_this(&d.object)
        }
        Expression::Index(ix) => roots_at_this(&ix.left),
        _ => false,
    }
}
