#![allow(unused_imports)]
use super::{
    CallFrame, DefaultArgumentResult, EvalResult, StoredClass, format_decimal, json_parse,
    json_stringify_owned, obj_data_eq, obj_data_to_key_str, operator_to_method_name, type_matches,
};
use super::{ExecutionFlow, RuntimeFailure};
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;

impl super::Evaluator {
    pub(super) fn inheritance_would_cycle(&self, class_name: &str, parent_name: &str) -> bool {
        let mut current = parent_name.to_string();
        let mut visited = HashSet::new();

        loop {
            if current == class_name {
                return true;
            }
            if !visited.insert(current.clone()) {
                // Defensive: do not attach a new class to an already-corrupt graph.
                return true;
            }
            let Some(class) = self.class_registry.get(&current) else {
                // Forward parent references remain compatible. They are checked
                // when the hierarchy is used, or when that parent is declared.
                return false;
            };
            let Some(parent) = &class.parent else {
                return false;
            };
            current = parent.clone();
        }
    }

    pub(super) fn eval_new_interface(
        &mut self,
        new_expr: &ast::NewExpression,
        iface_fields: Vec<ast::InterfaceField>,
    ) -> EvalResult {
        let provided = match &new_expr.args {
            ast::NewArgs::Fields(f) => f.clone(),
            ast::NewArgs::Positional(_) => {
                let message = format!(
                    "Interface '{}' must be instantiated with {{ field: value }} syntax",
                    new_expr.class_name,
                );
                return self.rt_err_kind("TypeError", message);
            }
        };

        // Check for extra fields not declared in the interface
        for (provided_name, _) in &provided {
            if !iface_fields.iter().any(|f| &f.name == provided_name) {
                let message = format!(
                    "Field '{}' is not declared in interface '{}'",
                    provided_name, new_expr.class_name,
                );
                return self.rt_err_kind("TypeError", message);
            }
        }

        let mut fields: Vec<(String, OwnedValue)> = Vec::new();
        for iface_field in &iface_fields {
            let entry = provided.iter().find(|(n, _)| n == &iface_field.name);
            match entry {
                Some((_, expr)) => {
                    let val_ref = match self.eval_expression(expr) {
                        Ok(ExecutionFlow::Value(r)) => r,
                        other => return other,
                    };
                    if let Some(actual) = self.resolve(val_ref) {
                        if !type_matches(&iface_field.type_name, actual) {
                            let message = format!(
                                "Interface field '{}' expects '{}' but got '{}'",
                                iface_field.name,
                                iface_field.type_name,
                                actual.type_name(),
                            );
                            return self.rt_err_kind("TypeError", message);
                        }
                    }
                    let owned = self.extract(val_ref);
                    fields.push((iface_field.name.clone(), owned));
                }
                None => {
                    let message = format!(
                        "Missing field '{}' when creating '{}'",
                        iface_field.name, new_expr.class_name,
                    );
                    return self.rt_err_kind("TypeError", message);
                }
            }
        }

        Ok(ExecutionFlow::Value(self.alloc(ObjectData::Instance {
            class_name: new_expr.class_name.clone(),
            fields,
        })))
    }

    pub(super) fn eval_new_class(
        &mut self,
        new_expr: &ast::NewExpression,
        class: StoredClass,
    ) -> EvalResult {
        // ── Abstract class check ──────────────────────────────────────────────
        if class.is_abstract {
            let message = format!(
                "Cannot instantiate abstract class '{}'",
                new_expr.class_name,
            );
            return self.rt_err_kind("TypeError", message);
        }

        let arg_exprs = match &new_expr.args {
            ast::NewArgs::Positional(a) => a.clone(),
            ast::NewArgs::Fields(_) => {
                let message = format!(
                    "Class '{}' uses positional arguments, not field syntax",
                    new_expr.class_name,
                );
                return self.rt_err_kind("TypeError", message);
            }
        };

        // Evaluate args before pushing scope
        let mut arg_vals: Vec<OwnedValue> = Vec::new();
        for expr in &arg_exprs {
            match self.eval_expression(expr) {
                Ok(ExecutionFlow::Value(r)) => arg_vals.push(self.extract(r)),
                other => return other,
            }
        }

        // ── Class field defaults ──────────────────────────────────────────────
        // Evaluate default values for class fields and add to initial instance
        let mut initial_fields: Vec<(String, OwnedValue)> = Vec::new();
        for field in &class.fields {
            if let Some(ref default_expr) = field.default_value {
                match self.eval_expression(default_expr) {
                    Ok(ExecutionFlow::Value(r)) => {
                        let owned = self.extract(r);
                        initial_fields.push((field.name.clone(), owned));
                    }
                    other => return other,
                }
            }
        }

        // Allocate instance with default field values
        let instance_ref = self.alloc(ObjectData::Instance {
            class_name: new_expr.class_name.clone(),
            fields: initial_fields,
        });

        if let Some(ctor) = class.constructor {
            let has_rest = ctor.parameters.last().map(|p| p.is_rest).unwrap_or(false);
            let required = ctor
                .parameters
                .iter()
                .filter(|p| !p.is_rest && p.default_value.is_none())
                .count();
            let max_pos = if has_rest {
                usize::MAX
            } else {
                ctor.parameters.len()
            };
            if arg_vals.len() < required || arg_vals.len() > max_pos {
                let message = format!(
                    "Constructor '{}' expects {} arguments, got {}",
                    new_expr.class_name,
                    ctor.parameters.len(),
                    arg_vals.len(),
                );
                return self.rt_err_kind("TypeError", message);
            }

            self.scopes.push();
            self.scopes.declare("this".to_string(), instance_ref);

            for (i, param) in ctor.parameters.iter().enumerate() {
                if param.is_rest {
                    let rest_items: Vec<OwnedValue> =
                        arg_vals[i.min(arg_vals.len())..].iter().cloned().collect();
                    let rest_ref = self.alloc(ObjectData::Array {
                        element_type: None,
                        elements: rest_items,
                    });
                    self.scopes.declare(param.name.clone(), rest_ref);
                    break;
                }
                let arg_ref = if i < arg_vals.len() {
                    self.plant(arg_vals[i].clone())
                } else if let Some(default_expr) = &param.default_value {
                    let default_expr = default_expr.clone();
                    match self.eval_default_argument(&default_expr) {
                        DefaultArgumentResult::Value(value) => value,
                        DefaultArgumentResult::Throw(owned) => {
                            self.scopes.pop();
                            return Ok(ExecutionFlow::Throw(self.plant(owned)));
                        }
                        DefaultArgumentResult::Error => {
                            self.scopes.pop();
                            return Err(RuntimeFailure);
                        }
                    }
                } else {
                    self.null_ref
                };
                // A declared constructor parameter type is enforced exactly as a
                // function's is. Arity was already checked above, but the type was
                // not, so `new Point("x", 1)` bound a string into an `int` field
                // and only surfaced wherever that field was later used as a number.
                // Defaults are always trailing, so a supplied argument never comes
                // after one: checking here matches the function path's "validate
                // every supplied argument before running any default".
                if i < arg_vals.len() {
                    if let Some(expected_type) = &param.type_name {
                        // Classify before raising: `resolve` holds an immutable
                        // borrow that must end before a diagnostic is recorded.
                        let mismatch = match self.resolve(arg_ref) {
                            Some(data) if type_matches(expected_type.as_str(), data) => None,
                            Some(data) => Some(data.type_name().to_string()),
                            None => Some("null".to_string()),
                        };
                        if let Some(actual) = mismatch {
                            let message = format!(
                                "Parameter '{}' of constructor '{}' expected '{}' but received '{}'",
                                param.name, new_expr.class_name, expected_type, actual
                            );
                            self.scopes.pop();
                            return self.rt_err_kind("TypeError", message);
                        }
                    }
                }
                self.scopes.declare(param.name.clone(), arg_ref);
            }

            let old_class = self.constructing_class.replace(new_expr.class_name.clone());

            let mut body_error = false;
            let mut ctor_throw: Option<ObjectRef> = None;

            // Encadenar al padre si el cuerpo no lo hace, ANTES de correr el
            // cuerpo: los campos del padre tienen que existir para que el
            // constructor propio pueda leerlos o pisarlos.
            if !self.ctor_calls_super(&new_expr.class_name, &ctor) {
                match self.run_implicit_super(&new_expr.class_name, instance_ref, true) {
                    Err(RuntimeFailure) => body_error = true,
                    Ok(ExecutionFlow::Throw(v)) => ctor_throw = Some(v),
                    _ => {}
                }
            }

            for stmt in &ctor.body.statements {
                if body_error || ctor_throw.is_some() {
                    break;
                }
                match self.eval_statement(stmt) {
                    Err(RuntimeFailure) => {
                        body_error = true;
                        break;
                    }
                    Ok(ExecutionFlow::Return(_)) => break,
                    Ok(ExecutionFlow::Value(_)) => {}
                    Ok(ExecutionFlow::Throw(v)) => {
                        ctor_throw = Some(v);
                        break;
                    }
                    Ok(ExecutionFlow::Break)
                    | Ok(ExecutionFlow::Continue)
                    | Ok(ExecutionFlow::BreakLabel(_))
                    | Ok(ExecutionFlow::ContinueLabel(_)) => {
                        let _ = self.rt_err("break/continue used outside a loop");
                        body_error = true;
                        break;
                    }
                }
            }

            self.constructing_class = old_class;

            // El objeto vivo no siempre es `instance_ref`: si el cuerpo creó una
            // closure que captura `this`, `capture_lambda_env` lo PROMOVIÓ a la
            // arena global y rebindeó el nombre. Hay que leer el binding, no la
            // variable de Rust, o se devuelve la copia vieja.
            let live_ref = self.scopes.lookup("this").unwrap_or(instance_ref);
            let throw_owned = ctor_throw.map(|r| self.extract(r));
            self.scopes.pop();

            if body_error {
                return Err(RuntimeFailure);
            }
            if let Some(owned) = throw_owned {
                return Ok(ExecutionFlow::Throw(self.plant(owned)));
            }

            // Se devuelve el slot que el constructor usó, NO una copia suya.
            //
            // Antes acá había un extract + plant, y ésa era la razón de que una
            // closure creada en el constructor no funcionara: capturaba el
            // `this` de la construcción y el `new` devolvía OTRO slot, así que
            // sus escrituras iban a un objeto que ya nadie leía. Registrar
            // efectos o callbacks en el constructor —lo natural viniendo de
            // React— quedaba mudo. Creada en un método normal andaba, porque
            // ahí no hay re-plant de por medio.
            //
            // La copia tampoco hacía falta para la vida del valor: `instance_ref`
            // se aloca ANTES del push del constructor, así que su pop no lo
            // toca, y el plant que había después alocaba a la misma profundidad.
            // De paso se ahorra una copia profunda de la instancia por cada
            // `new`.
            Ok(ExecutionFlow::Value(live_ref))
        } else {
            if !arg_vals.is_empty() {
                let message = format!(
                    "Class '{}' has no constructor but received {} arguments",
                    new_expr.class_name,
                    arg_vals.len(),
                );
                return self.rt_err_kind("TypeError", message);
            }
            // Sin constructor propio pero CON padre: hay que correr el del padre
            // igual, o la instancia nace sin sus campos. Es el caso más común
            // viniendo de React, donde el constructor es opcional.
            if self
                .class_registry
                .get(&new_expr.class_name)
                .and_then(|c| c.parent.clone())
                .is_some()
            {
                self.scopes.push();
                self.scopes.declare("this".to_string(), instance_ref);
                let old_class = self.constructing_class.replace(new_expr.class_name.clone());
                let res = self.run_implicit_super(&new_expr.class_name, instance_ref, false);
                self.constructing_class = old_class;
                // El `this` vivo puede haber sido promovido si el constructor del
                // padre creó una closure que lo captura — mismo caso que arriba.
                let live_ref = self.scopes.lookup("this").unwrap_or(instance_ref);
                let throw_owned = match res {
                    Ok(ExecutionFlow::Throw(v)) => Some(self.extract(v)),
                    Err(RuntimeFailure) => {
                        self.scopes.pop();
                        return Err(RuntimeFailure);
                    }
                    _ => None,
                };
                self.scopes.pop();
                if let Some(owned) = throw_owned {
                    return Ok(ExecutionFlow::Throw(self.plant(owned)));
                }
                return Ok(ExecutionFlow::Value(live_ref));
            }
            Ok(ExecutionFlow::Value(instance_ref))
        }
    }

    /// ¿El cuerpo del constructor llama a `super(...)` en algún lado?
    ///
    /// Decide si hay que encadenar al padre implícitamente. Es un recorrido
    /// conservador: alcanza con que `super(` aparezca en cualquier rama para no
    /// insertar nada (mejor no llamarlo dos veces que llamarlo de más).
    /// Cacheado por clase: el escaneo se hace una vez, no en cada `new`.
    fn ctor_calls_super(&mut self, class_name: &str, ctor: &ast::ClassConstructor) -> bool {
        if let Some(&hit) = self.dispatch.super_call.get(class_name) {
            return hit;
        }
        let mut found = false;
        super::lvalue::calls_super_block(&ctor.body, &mut found);
        self.dispatch
            .super_call
            .insert(class_name.to_string(), found);
        found
    }

    /// Encadena al constructor del padre sin que el usuario escriba `super()`.
    ///
    /// Es lo que hacen Java, C# y JavaScript: si una subclase no llama al
    /// constructor del padre, se llama solo. Sin esto, `class App:Window` sin
    /// constructor —lo normal viniendo de React— dejaba la instancia SIN NINGUNO
    /// de los campos del padre, y el programa moría después con un mensaje que
    /// no apuntaba a nada: "'App' has no field or method named 'effects'".
    ///
    /// Sólo se encadena si el constructor del padre se puede llamar SIN
    /// argumentos. Si exige alguno, la subclase tiene que escribir `super(...)`
    /// a mano, y acá se distinguen dos situaciones:
    ///
    /// - La subclase **tiene** constructor y no llamó a super: se deja pasar en
    ///   silencio, como siempre. Es un estilo que el lenguaje permitía y que hay
    ///   código usando — inicializar los campos del padre uno mismo
    ///   (`public Perro(string n) { this.nombre = n }`). Convertirlo en error
    ///   rompería programas que andan.
    /// - La subclase **no tiene** constructor: ahí no hay nadie que inicialice
    ///   nada, el objeto nace vacío y el fallo aparece mucho después con un
    ///   mensaje que no apunta a la causa. Eso sí se avisa.
    fn run_implicit_super(
        &mut self,
        class_name: &str,
        instance_ref: ObjectRef,
        has_own_ctor: bool,
    ) -> EvalResult {
        let Some(parent_name) = self
            .class_registry
            .get(class_name)
            .and_then(|c| c.parent.clone())
        else {
            return Ok(ExecutionFlow::Value(self.null_ref));
        };
        let parent_class = match self.class_registry.get(&parent_name).cloned() {
            Some(parent) => parent,
            None => {
                let message = format!(
                    "Parent class '{}' declared by '{}' is not defined",
                    parent_name, class_name,
                );
                return self.rt_err_kind("ReferenceError", message);
            }
        };
        let Some(parent_ctor) = parent_class.constructor else {
            return Ok(ExecutionFlow::Value(self.null_ref));
        };

        let required = parent_ctor
            .parameters
            .iter()
            .filter(|p| !p.is_rest && p.default_value.is_none())
            .count();
        if required > 0 {
            if has_own_ctor {
                return Ok(ExecutionFlow::Value(self.null_ref));
            }
            let message = format!(
                "'{}' extends '{}', whose constructor needs {} argument(s), so it cannot be \
                 chained automatically. Give '{}' a constructor that calls super(...) with them.",
                class_name, parent_name, required, class_name,
            );
            return self.rt_err_kind("TypeError", message);
        }

        // eval_super_call lee `constructing_class` para encontrar al padre y
        // espera `this` en scope: es exactamente el estado en el que estamos.
        let _ = instance_ref;
        self.eval_super_call(&[])
    }

    pub(super) fn eval_super_call(&mut self, args: &[ast::Expression]) -> EvalResult {
        let current_class = match &self.constructing_class {
            Some(c) => c.clone(),
            None => {
                return self.rt_err_kind("TypeError", "super() called outside of a constructor");
            }
        };
        let parent_name = match self
            .class_registry
            .get(&current_class)
            .and_then(|c| c.parent.clone())
        {
            Some(p) => p,
            None => {
                let message =
                    format!("Class '{}' has no parent to call super() on", current_class,);
                return self.rt_err_kind("TypeError", message);
            }
        };
        let parent_class = match self.class_registry.get(&parent_name).cloned() {
            Some(parent) => parent,
            None => {
                let message = format!(
                    "Parent class '{}' declared by '{}' is not defined",
                    parent_name, current_class,
                );
                return self.rt_err_kind("ReferenceError", message);
            }
        };
        let parent_ctor = match parent_class.constructor {
            Some(ctor) => ctor,
            None if args.is_empty() => return Ok(ExecutionFlow::Value(self.null_ref)),
            None => {
                let message = format!(
                    "Parent class '{}' has no constructor but super() received {} argument(s)",
                    parent_name,
                    args.len(),
                );
                return self.rt_err_kind("TypeError", message);
            }
        };

        let mut arg_vals: Vec<OwnedValue> = Vec::new();
        for expr in args {
            match self.eval_expression(expr) {
                Ok(ExecutionFlow::Value(r)) => arg_vals.push(self.extract(r)),
                other => return other,
            }
        }

        let has_rest = parent_ctor
            .parameters
            .last()
            .map(|p| p.is_rest)
            .unwrap_or(false);
        let required = parent_ctor
            .parameters
            .iter()
            .filter(|p| !p.is_rest && p.default_value.is_none())
            .count();
        let max_pos = if has_rest {
            usize::MAX
        } else {
            parent_ctor.parameters.len()
        };
        if arg_vals.len() < required || arg_vals.len() > max_pos {
            let message = format!(
                "super() for '{}' expects {} arguments, got {}",
                parent_name,
                parent_ctor.parameters.len(),
                arg_vals.len(),
            );
            return self.rt_err_kind("TypeError", message);
        }

        // Execute parent constructor body — "this" is already bound in the current scope
        self.scopes.push();
        for (i, param) in parent_ctor.parameters.iter().enumerate() {
            if param.is_rest {
                let rest_owned: Vec<OwnedValue> =
                    arg_vals[i.min(arg_vals.len())..].iter().cloned().collect();
                let rest_ref = self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: rest_owned,
                });
                self.scopes.declare(param.name.clone(), rest_ref);
                break;
            }
            let arg_ref = if i < arg_vals.len() {
                self.plant(arg_vals[i].clone())
            } else if let Some(default_expr) = &param.default_value {
                let default_expr = default_expr.clone();
                match self.eval_default_argument(&default_expr) {
                    DefaultArgumentResult::Value(value) => value,
                    DefaultArgumentResult::Throw(owned) => {
                        self.scopes.pop();
                        return Ok(ExecutionFlow::Throw(self.plant(owned)));
                    }
                    DefaultArgumentResult::Error => {
                        self.scopes.pop();
                        return Err(RuntimeFailure);
                    }
                }
            } else {
                self.null_ref
            };
            self.scopes.declare(param.name.clone(), arg_ref);
        }

        let old_class = self.constructing_class.replace(parent_name);

        let mut error = false;
        let mut super_throw: Option<ObjectRef> = None;
        for stmt in &parent_ctor.body.statements {
            match self.eval_statement(stmt) {
                Err(RuntimeFailure) => {
                    error = true;
                    break;
                }
                Ok(ExecutionFlow::Return(_)) => break,
                Ok(ExecutionFlow::Value(_)) => {}
                Ok(ExecutionFlow::Throw(v)) => {
                    super_throw = Some(v);
                    break;
                }
                Ok(ExecutionFlow::Break)
                | Ok(ExecutionFlow::Continue)
                | Ok(ExecutionFlow::BreakLabel(_))
                | Ok(ExecutionFlow::ContinueLabel(_)) => {
                    let _ = self.rt_err("break/continue used outside a loop");
                    error = true;
                    break;
                }
            }
        }

        self.constructing_class = old_class;
        let throw_owned = super_throw.map(|r| self.extract(r));
        self.scopes.pop();

        if error {
            return Err(RuntimeFailure);
        }
        if let Some(owned) = throw_owned {
            return Ok(ExecutionFlow::Throw(self.plant(owned)));
        }
        Ok(ExecutionFlow::Value(self.null_ref))
    }

    pub(super) fn eval_super_method_call(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        let current_class = match &self.executing_class {
            Some(c) => c.clone(),
            None => {
                let message = format!(
                    "super.{}() called outside of a class method",
                    dot_call.method,
                );
                return self.rt_err_kind("TypeError", message);
            }
        };

        let parent_name = match self
            .class_registry
            .get(&current_class)
            .and_then(|c| c.parent.clone())
        {
            Some(p) => p,
            None => {
                let message = format!(
                    "Class '{}' has no parent — cannot call super.{}()",
                    current_class, dot_call.method,
                );
                return self.rt_err_kind("TypeError", message);
            }
        };

        if !self.class_registry.contains_key(&parent_name) {
            let message = format!(
                "Parent class '{}' declared by '{}' is not defined",
                parent_name, current_class,
            );
            return self.rt_err_kind("ReferenceError", message);
        }

        let method = match self.find_method(&parent_name, &dot_call.method) {
            Some(m) => m,
            None => {
                let message = format!(
                    "Parent class '{}' has no method '{}'",
                    parent_name, dot_call.method,
                );
                return self.rt_err_kind("ReferenceError", message);
            }
        };

        let this_ref = match self.scopes.lookup("this") {
            Some(r) => r,
            None => {
                let message = format!("super.{}() called with no 'this' in scope", dot_call.method);
                return self.rt_err_kind("TypeError", message);
            }
        };

        let mut arg_vals: Vec<OwnedValue> = Vec::new();
        for expr in &dot_call.arguments {
            match self.eval_expression(expr) {
                Ok(ExecutionFlow::Value(r)) => arg_vals.push(self.extract(r)),
                other => return other,
            }
        }

        let has_rest = method.parameters.last().map(|p| p.is_rest).unwrap_or(false);
        let required = method
            .parameters
            .iter()
            .filter(|p| !p.is_rest && p.default_value.is_none())
            .count();
        let max_pos = if has_rest {
            usize::MAX
        } else {
            method.parameters.len()
        };
        if arg_vals.len() < required || arg_vals.len() > max_pos {
            let message = format!(
                "Method '{}::{}' expects {} arguments, got {}",
                parent_name,
                dot_call.method,
                method.parameters.len(),
                arg_vals.len(),
            );
            return self.rt_err_kind("TypeError", message);
        }

        if let Some(error) = self.require_call_capacity() {
            return error;
        }

        let old_executing_class = self.executing_class.take();
        self.executing_class = Some(parent_name.clone());

        self.call_stack.push(CallFrame {
            name: format!("{}::{}", parent_name, dot_call.method),
            line: dot_call.span.line,
            column: dot_call.span.column,
        });
        self.scopes.push();
        self.call_depth += 1;
        self.scopes.declare("this".to_string(), this_ref);

        for (i, param) in method.parameters.iter().enumerate() {
            if param.is_rest {
                let rest_owned: Vec<OwnedValue> =
                    arg_vals[i.min(arg_vals.len())..].iter().cloned().collect();
                let rest_ref = self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: rest_owned,
                });
                self.scopes.declare(param.name.clone(), rest_ref);
                break;
            }
            let arg_ref = if i < arg_vals.len() {
                self.plant(arg_vals[i].clone())
            } else if let Some(default_expr) = &param.default_value {
                let default_expr = default_expr.clone();
                match self.eval_default_argument(&default_expr) {
                    DefaultArgumentResult::Value(value) => value,
                    DefaultArgumentResult::Throw(owned) => {
                        self.call_depth -= 1;
                        self.scopes.pop();
                        self.call_stack.pop();
                        self.executing_class = old_executing_class;
                        return Ok(ExecutionFlow::Throw(self.plant(owned)));
                    }
                    DefaultArgumentResult::Error => {
                        self.call_depth -= 1;
                        self.scopes.pop();
                        self.call_stack.pop();
                        self.executing_class = old_executing_class;
                        return Err(RuntimeFailure);
                    }
                }
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
                Ok(ExecutionFlow::Value(_)) => {}
                Ok(ExecutionFlow::Return(v)) => {
                    result_ref = v;
                    break;
                }
                Ok(ExecutionFlow::Throw(v)) => {
                    method_throw = Some(v);
                    break;
                }
                Err(RuntimeFailure) => {
                    error = true;
                    break;
                }
                Ok(ExecutionFlow::Break)
                | Ok(ExecutionFlow::Continue)
                | Ok(ExecutionFlow::BreakLabel(_))
                | Ok(ExecutionFlow::ContinueLabel(_)) => {
                    let _ = self.rt_err("break/continue used outside a loop");
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

        if error {
            return Err(RuntimeFailure);
        }
        if let Some(t) = throw_owned {
            return Ok(ExecutionFlow::Throw(self.plant(t)));
        }
        Ok(ExecutionFlow::Value(self.plant(owned)))
    }

    pub(super) fn eval_object_patch(
        &mut self,
        var_name: &str,
        patch: Vec<(String, ast::Expression)>,
    ) -> EvalResult {
        let obj_ref = match self.lookup_var(var_name) {
            Some(r) => r,
            None => {
                let message = format!("Undeclared variable '{var_name}' in object patch");
                return self.rt_err_kind("ReferenceError", message);
            }
        };

        if let Some(ObjectData::Instance {
            class_name,
            mut fields,
        }) = self.resolve(obj_ref).cloned()
        {
            // Validate against interface schema if it's an interface
            let schema = self.interface_registry.get(&class_name).cloned();

            for (field_name, expr) in patch {
                let val_ref = match self.eval_expression(&expr) {
                    Ok(ExecutionFlow::Value(r)) => r,
                    other => return other,
                };
                if let Some(ref schema_fields) = schema {
                    if let Some(iface_field) = schema_fields.iter().find(|f| f.name == field_name) {
                        // Classify before raising: `resolve` holds an immutable
                        // borrow that must end before a diagnostic is recorded.
                        let mismatch = match self.resolve(val_ref) {
                            Some(actual) if type_matches(&iface_field.type_name, actual) => None,
                            Some(actual) => Some(actual.type_name().to_string()),
                            None => None,
                        };
                        if let Some(actual) = mismatch {
                            let expected = iface_field.type_name.clone();
                            let message = format!(
                                "Field '{field_name}' expects '{expected}' but got '{actual}'"
                            );
                            return self.rt_err_kind("TypeError", message);
                        }
                    }
                }
                let owned = self.extract(val_ref);
                if let Some(f) = fields.iter_mut().find(|(n, _)| n == &field_name) {
                    f.1 = owned;
                } else {
                    fields.push((field_name, owned));
                }
            }

            match obj_ref.region {
                RegionId::Global => self
                    .global_arena
                    .update(obj_ref.index, ObjectData::Instance { class_name, fields }),
                RegionId::Scoped => self
                    .scopes
                    .arena
                    .update(obj_ref.index, ObjectData::Instance { class_name, fields }),
            }
            Ok(ExecutionFlow::Value(self.null_ref))
        } else {
            let message =
                format!("'{var_name}' is not an interface instance — cannot use patch syntax");
            self.rt_err_kind("TypeError", message)
        }
    }

    /// Dispatched from expr.rs against the arena slot: the receiver is NOT
    /// cloned. Field values are pulled one at a time via `field_value`, so a
    /// method call on an instance carrying a big collection no longer copies
    /// that collection. Reading a field still copies that field's value —
    /// that is the language's value semantics, unchanged.
    pub(super) fn eval_instance_dot(
        &mut self,
        obj_ref: ObjectRef,
        class_name: String,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        let method_name = &dot_call.method;

        // Field read: no parens and no args and field exists → return value (not call)
        if !dot_call.has_parens && dot_call.arguments.is_empty() {
            if let Some(owned) = self.field_value(obj_ref, method_name) {
                return Ok(ExecutionFlow::Value(self.plant(owned)));
            }
            // Getter: no parens, no field → look for `get prop()`
            if let Some(getter) = self.find_getter(&class_name, method_name) {
                return self.invoke_method(
                    obj_ref,
                    &class_name,
                    &getter,
                    vec![],
                    dot_call.span.line,
                    dot_call.span.column,
                );
            }
            // Referencia a método: no hay campo ni getter, pero sí un método con ese
            // nombre → `obj.metodo` VALE la función ligada a obj, no su ejecución.
            // Antes caía al despacho de abajo y lo invocaba con cero argumentos, así que
            // pasar un handler como dato (`onClick={this.handler}`) lo ejecutaba en cada
            // lectura y guardaba su valor de retorno en lugar de la función.
            if let Some(m) = self.find_method(&class_name, method_name) {
                if !m.is_public && self.executing_class.as_deref() != Some(class_name.as_str()) {
                    let message = format!(
                        "Method '{}' is private and cannot be referenced externally",
                        method_name
                    );
                    return self.rt_err_kind("TypeError", message);
                }
                return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Function {
                    return_type: m.return_type.clone(),
                    parameters: Rc::new(m.parameters.clone()),
                    body: Rc::new(m.body.clone()),
                    captured: Rc::new(vec![("this".to_string(), obj_ref)]),
                    is_generator: false,
                    bound_class: Some(class_name.clone()),
                })));
            }
        }

        // Method dispatch: walk inheritance chain
        let method = self.find_method(&class_name, method_name);
        match method {
            Some(m) => {
                let args_exprs = dot_call.arguments.clone();
                let mut arg_vals: Vec<OwnedValue> = Vec::new();
                for expr in &args_exprs {
                    match self.eval_expression(expr) {
                        Ok(ExecutionFlow::Value(r)) => arg_vals.push(self.extract(r)),
                        other => return other,
                    }
                }
                self.invoke_method(
                    obj_ref,
                    &class_name,
                    &m,
                    arg_vals,
                    dot_call.span.line,
                    dot_call.span.column,
                )
            }
            None => {
                // Fallback: toString() is available on all instance types
                if method_name == "toString" {
                    let s = self.display(obj_ref);
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(s))));
                }
                // Fallback: field holds a callable function (this.fn_field(args)).
                // The field is snapshotted BEFORE the arguments are evaluated —
                // same order as when the whole receiver was cloned up front, so an
                // argument that reassigns the field still calls the old one.
                if let Some(owned) = self.field_value(obj_ref, method_name) {
                    let fn_ref = self.plant(owned);
                    let mut arg_vals = Vec::new();
                    for arg_expr in &dot_call.arguments {
                        match self.eval_expression(arg_expr) {
                            Ok(ExecutionFlow::Value(r)) => arg_vals.push(self.extract(r)),
                            other => return other,
                        }
                    }
                    return self.call_function(fn_ref, arg_vals);
                }
                let message = format!(
                    "'{}' has no field or method named '{}'",
                    class_name, method_name
                );
                self.rt_err_kind("ReferenceError", message)
            }
        }
    }

    /// One field's value, copied out of the instance living in the arena slot.
    /// Copies that field alone — never the whole instance.
    pub(super) fn field_value(&self, obj_ref: ObjectRef, name: &str) -> Option<OwnedValue> {
        match self.resolve(obj_ref) {
            Some(ObjectData::Instance { fields, .. }) => fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone()),
            _ => None,
        }
    }

    // Walk the inheritance chain to find a method
    pub(super) fn find_method(
        &self,
        class_name: &str,
        method_name: &str,
    ) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        for _ in 0..self.class_registry.len() {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.methods.get(method_name) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
        None
    }

    pub(super) fn find_getter(
        &self,
        class_name: &str,
        prop_name: &str,
    ) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        for _ in 0..self.class_registry.len() {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.getters.get(prop_name) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
        None
    }

    pub(super) fn find_setter(
        &self,
        class_name: &str,
        prop_name: &str,
    ) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        for _ in 0..self.class_registry.len() {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.setters.get(prop_name) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
        None
    }

    // Shared helper: invoke a ClassMethod on an instance with pre-evaluated arg values.
    pub(super) fn invoke_method(
        &mut self,
        obj_ref: ObjectRef,
        class_name: &str,
        m: &ast::ClassMethod,
        arg_vals: Vec<OwnedValue>,
        call_line: usize,
        call_column: usize,
    ) -> EvalResult {
        let method_name = &m.name;

        // arity check — account for default parameter values and rest params
        let has_rest_m = m.parameters.last().map(|p| p.is_rest).unwrap_or(false);
        let required_count = m
            .parameters
            .iter()
            .filter(|p| !p.is_rest && p.default_value.is_none())
            .count();
        let max_count = if has_rest_m {
            usize::MAX
        } else {
            m.parameters.len()
        };
        if arg_vals.len() < required_count || arg_vals.len() > max_count {
            let expected_str = if has_rest_m {
                format!("at least {}", required_count)
            } else if required_count == max_count {
                format!("{}", required_count)
            } else {
                format!("{}-{}", required_count, max_count)
            };
            let message = format!(
                "Method '{}' expects {} argument(s), got {}",
                method_name,
                expected_str,
                arg_vals.len()
            );
            return self.rt_err_kind("TypeError", message);
        }

        if !m.is_public && self.executing_class.as_deref() != Some(class_name) {
            let message = format!(
                "Method '{}' is private and cannot be called externally",
                method_name
            );
            return self.rt_err_kind("TypeError", message);
        }

        if let Some(error) = self.require_call_capacity() {
            return error;
        }

        let old_executing_class = self.executing_class.take();
        self.executing_class = Some(class_name.to_string());

        self.call_stack.push(CallFrame {
            name: format!("{}::{}", class_name, method_name),
            line: call_line,
            column: call_column,
        });
        self.scopes.push();
        self.call_depth += 1;
        self.scopes.declare("this".to_string(), obj_ref);

        for (i, param) in m.parameters.iter().enumerate() {
            if param.is_rest {
                let rest_owned: Vec<OwnedValue> =
                    arg_vals[i.min(arg_vals.len())..].iter().cloned().collect();
                let rest_ref = self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: rest_owned,
                });
                self.scopes.declare(param.name.clone(), rest_ref);
                break;
            }
            let arg_ref = if i < arg_vals.len() {
                self.plant(arg_vals[i].clone())
            } else if let Some(default_expr) = &param.default_value {
                let default_expr = default_expr.clone();
                match self.eval_default_argument(&default_expr) {
                    DefaultArgumentResult::Value(value) => value,
                    DefaultArgumentResult::Throw(owned) => {
                        self.call_depth -= 1;
                        self.scopes.pop();
                        self.call_stack.pop();
                        self.executing_class = old_executing_class;
                        return Ok(ExecutionFlow::Throw(self.plant(owned)));
                    }
                    DefaultArgumentResult::Error => {
                        self.call_depth -= 1;
                        self.scopes.pop();
                        self.call_stack.pop();
                        self.executing_class = old_executing_class;
                        return Err(RuntimeFailure);
                    }
                }
            } else {
                self.null_ref
            };
            self.scopes.declare(param.name.clone(), arg_ref);
        }

        let mut result_ref = self.null_ref;
        let mut error = false;
        let mut method_throw: Option<ObjectRef> = None;
        for stmt in &m.body.statements {
            match self.eval_statement(stmt) {
                Ok(ExecutionFlow::Value(_)) => {}
                Ok(ExecutionFlow::Return(v)) => {
                    result_ref = v;
                    break;
                }
                Ok(ExecutionFlow::Throw(v)) => {
                    method_throw = Some(v);
                    break;
                }
                Err(RuntimeFailure) => {
                    error = true;
                    break;
                }
                Ok(ExecutionFlow::Break)
                | Ok(ExecutionFlow::Continue)
                | Ok(ExecutionFlow::BreakLabel(_))
                | Ok(ExecutionFlow::ContinueLabel(_)) => {
                    let _ = self.rt_err("break/continue used outside a loop");
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

        if error {
            return Err(RuntimeFailure);
        }
        if let Some(t) = throw_owned {
            return Ok(ExecutionFlow::Throw(self.plant(t)));
        }

        let result = self.plant(owned);

        if let Some(ref rt) = m.return_type {
            let actual = self.resolve(result).unwrap();
            if !type_matches(rt, actual) {
                let message = format!(
                    "Method '{}' declared return '{}' but returned '{}'",
                    method_name,
                    rt,
                    actual.type_name()
                );
                return self.rt_err_kind("TypeError", message);
            }
        }

        Ok(ExecutionFlow::Value(result))
    }

    // ── Array methods ─────────────────────────────────────────────────────────
}

#[cfg(test)]
mod inheritance_graph_tests {
    use super::super::{Evaluator, StoredClass};
    use std::collections::HashMap;

    fn stored_class(parent: &str) -> StoredClass {
        StoredClass {
            parent: Some(parent.to_string()),
            constructor: None,
            methods: HashMap::new(),
            static_methods: HashMap::new(),
            getters: HashMap::new(),
            setters: HashMap::new(),
            is_abstract: false,
            is_sealed: false,
            fields: Vec::new(),
        }
    }

    #[test]
    fn legacy_cyclic_registry_lookups_are_bounded() {
        let mut evaluator = Evaluator::new();
        evaluator
            .class_registry
            .insert("CycleA".to_string(), stored_class("CycleB"));
        evaluator
            .class_registry
            .insert("CycleB".to_string(), stored_class("CycleA"));

        assert!(evaluator.find_method("CycleA", "missing").is_none());
        assert!(evaluator.find_getter("CycleA", "missing").is_none());
        assert!(evaluator.find_setter("CycleA", "missing").is_none());
        assert!(evaluator.inheritance_would_cycle("NewChild", "CycleA"));
    }
}
