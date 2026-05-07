use crate::ast::{Expression, Program, Statement};
use std::collections::HashMap;

// 1. Los valores resultantes que entiende la computadora
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Object {
    Integer(i64),
    String(String),
    Array(Vec<Object>),
    Boolean(bool), // Agregado para soportar true/false
    Null,          // Para sentencias que no devuelven nada, como `let`
}

// 2. Nuestro Intérprete y su Memoria (Environment / Tabla de Símbolos)
pub struct Evaluator {
    // Aquí guardaremos las variables. Ej: "ii" -> Object::Integer(1)
    env: HashMap<String, Object>,
}

impl Evaluator {
    pub fn new() -> Self {
        Evaluator {
            env: HashMap::new(),
        }
    }

    // Evalúa todo el programa línea por línea
    pub fn eval_program(&mut self, program: &Program) -> Option<Object> {
        let mut result = Object::Null;

        for statement in &program.statements {
            if let Some(evaluated) = self.eval_statement(statement) {
                result = evaluated;
            }
        }
        Some(result) // Devuelve el resultado de la última línea
    }

    // Evalúa sentencias (como let)
    fn eval_statement(&mut self, stmt: &Statement) -> Option<Object> {
        match stmt {
            Statement::Let(let_stmt) => {
                // 1. Evaluamos la expresión de la derecha del '='
                let val = self.eval_expression(&let_stmt.value)?;
                // 2. La guardamos en nuestra memoria (HashMap)
                self.env.insert(let_stmt.name.clone(), val);
                Some(Object::Null)
            }
            Statement::Assign(assign_stmt) => {
                // Reasignación: la variable DEBE existir previamente
                if !self.env.contains_key(&assign_stmt.name) {
                    println!("❌ ERROR: Variable no declarada: {}", assign_stmt.name);
                    return None;
                }
                let val = self.eval_expression(&assign_stmt.value)?;
                self.env.insert(assign_stmt.name.clone(), val.clone());
                Some(val) // Devolvemos el nuevo valor para que el REPL lo muestre
            }
            Statement::Expression(expr) => {
                // Si es una expresión suelta (como "ii" o "[1, 2]"), la evaluamos
                // y devolvemos el resultado (esto es lo que hace que el REPL imprima valores)
                self.eval_expression(expr)
            }
        }
    }

    // Evalúa expresiones (como números, textos, arrays y variables)
    fn eval_expression(&mut self, expr: &Expression) -> Option<Object> {
        match expr {
            Expression::Integer(i) => Some(Object::Integer(*i)),
            Expression::String(s) => Some(Object::String(s.clone())),
            Expression::Boolean(b) => Some(Object::Boolean(*b)), // Asegúrate de tener Object::Boolean en tu enum
            Expression::Identifier(name) => match self.env.get(name) {
                Some(val) => Some(val.clone()),
                None => {
                    println!("❌ ERROR: Variable no encontrada: {}", name);
                    None
                }
            },
            Expression::ArrayLiteral(elements) => {
                let mut arr = Vec::new();
                for el in elements {
                    if let Some(val) = self.eval_expression(el) {
                        arr.push(val);
                    }
                }
                Some(Object::Array(arr))
            }
            // ¡NUEVO! Evaluamos el prefijo (ej: -5)
            Expression::Prefix(operator, right_expr) => {
                let right = self.eval_expression(right_expr)?;
                self.eval_prefix_expression(operator, right)
            }
            // ¡NUEVO! Evaluamos operaciones matemáticas (ej: 1 + 1)
            Expression::Infix(left_expr, operator, right_expr) => {
                let left = self.eval_expression(left_expr)?;
                let right = self.eval_expression(right_expr)?;
                self.eval_infix_expression(operator, left, right)
            }
        }
    }

    fn eval_prefix_expression(&mut self, operator: &str, right: Object) -> Option<Object> {
        match operator {
            "-" => {
                if let Object::Integer(i) = right {
                    Some(Object::Integer(-i))
                } else {
                    println!("❌ ERROR EVALUADOR: Operador prefijo no soportado para este tipo");
                    None
                }
            }
            "!" => {
                // Si agregaste lógicas booleanas: !true -> false
                if let Object::Boolean(b) = right {
                    Some(Object::Boolean(!b))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn eval_infix_expression(
        &mut self,
        operator: &str,
        left: Object,
        right: Object,
    ) -> Option<Object> {
        match (left, right) {
            // ── Integer op Integer ──────────────────────────────────────────
            (Object::Integer(l), Object::Integer(r)) => match operator {
                "+" => Some(Object::Integer(l + r)),
                "-" => Some(Object::Integer(l - r)),
                "*" => Some(Object::Integer(l * r)),
                "/" => {
                    if r == 0 {
                        println!("❌ ERROR EVALUADOR: División por cero");
                        None
                    } else {
                        Some(Object::Integer(l / r))
                    }
                }
                "%" => Some(Object::Integer(l % r)),
                "<" => Some(Object::Boolean(l < r)),
                ">" => Some(Object::Boolean(l > r)),
                "==" => Some(Object::Boolean(l == r)),
                "!=" => Some(Object::Boolean(l != r)),
                _ => {
                    println!("❌ ERROR EVALUADOR: Operador desconocido: {}", operator);
                    None
                }
            },

            // ── String op String ────────────────────────────────────────────
            (Object::String(l), Object::String(r)) => match operator {
                "+" => Some(Object::String(l + &r)), // "hola" + "mundo" → "holamundo"
                "==" => Some(Object::Boolean(l == r)),
                "!=" => Some(Object::Boolean(l != r)),
                _ => {
                    println!(
                        "❌ ERROR EVALUADOR: Operador '{}' no soportado entre strings",
                        operator
                    );
                    None
                }
            },

            // ── String * Integer ────────────────────────────────────────────
            (Object::String(s), Object::Integer(n)) => match operator {
                "*" => {
                    if n < 0 {
                        println!(
                            "❌ ERROR EVALUADOR: No se puede repetir un string con número negativo"
                        );
                        None
                    } else {
                        Some(Object::String(s.repeat(n as usize)))
                    }
                }
                _ => {
                    println!(
                        "❌ ERROR EVALUADOR: Operador '{}' no soportado entre String e Integer",
                        operator
                    );
                    None
                }
            },

            // ── Tipos incompatibles ─────────────────────────────────────────
            _ => {
                println!("❌ ERROR EVALUADOR: Los tipos no coinciden para esta operación");
                None
            }
        }
    }
}
