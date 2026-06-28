use crate::ast;
use crate::region::ObjectData;
use super::EvalResult;
use std::sync::{Mutex, OnceLock};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) enum TaskState {
    Running,
    Finished { result: String },
    Failed { error: String },
}

pub(crate) static TASK_REGISTRY: OnceLock<Mutex<HashMap<i64, TaskState>>> = OnceLock::new();
static NEXT_TASK_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

fn registry() -> &'static Mutex<HashMap<i64, TaskState>> {
    TASK_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

impl super::Evaluator {
    pub(super) fn eval_task_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        // En Serez el namespace es "Task"
        if !self.permissions.contains("Task") {
            eprintln!(
                "❌ ERROR: 'Task' requires permission 'Task' — declare it in serez.json \
                 (\"permissions\": [\"Task\", ...]) or with `use permissions {{ Task }}`"
            );
            return EvalResult::Error;
        }

        match dot_call.method.as_str() {
            "run" => {
                // Task.run(script_path, arg_string) -> int (taskId)
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Task.run(script_path, arg_string) requires 2 arguments");
                    return EvalResult::Error;
                }
                let path_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    _ => return EvalResult::Error,
                };
                let arg_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(v) => v,
                    _ => return EvalResult::Error,
                };

                let script_path = match self.resolve(path_ref).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => {
                        eprintln!("❌ ERROR: Task.run: script_path must be a string");
                        return EvalResult::Error;
                    }
                };

                let arg_string = match self.resolve(arg_ref).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => {
                        eprintln!("❌ ERROR: Task.run: arg_string must be a string");
                        return EvalResult::Error;
                    }
                };

                // Obtener un nuevo ID de tarea
                let task_id = NEXT_TASK_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                // Insertar en el registro como Running
                {
                    let mut reg = registry().lock().unwrap();
                    reg.insert(task_id, TaskState::Running);
                }

                // Lanzar el hilo en segundo plano
                let script_path_clone = script_path.clone();
                let arg_string_clone = arg_string.clone();
                
                let builder = std::thread::Builder::new()
                    .name(format!("task-worker-{}", task_id))
                    .stack_size(16 * 1024 * 1024); // 16 MB para sub-tareas
                
                let handle_res = builder.spawn(move || {
                    // Cargar y evaluar el script
                    let input = match std::fs::read_to_string(&script_path_clone) {
                        Ok(content) => content,
                        Err(e) => {
                            let mut reg = registry().lock().unwrap();
                            reg.insert(task_id, TaskState::Failed { error: format!("Error reading file '{}': {}", script_path_clone, e) });
                            return;
                        }
                    };

                    let source_lines: Vec<String> = input.lines().map(|l| l.to_string()).collect();
                    let lexer = crate::lexer::Lexer::new(input);
                    let mut parser = crate::parser::Parser::new(lexer);
                    parser.set_source(source_lines.clone());
                    let program = parser.parse_program();

                    let mut checker = crate::type_checker::TypeChecker::new(&program);
                    checker.check();

                    let mut evaluator = crate::evaluator::Evaluator::new();
                    evaluator.set_source(source_lines);
                    evaluator.set_task_context(task_id, arg_string_clone);
                    
                    // Heredar el archivo actual
                    let file_path_obj = std::path::Path::new(&script_path_clone);
                    evaluator.set_current_file(file_path_obj);

                    // Cargar permisos locales si existen (de serez.json)
                    if let Some(dir) = file_path_obj.parent() {
                        let dir = if dir == std::path::Path::new("") { std::path::Path::new(".") } else { dir };
                        if let Ok(manifest) = crate::package_manager::SerezManifest::load(dir) {
                            evaluator.set_permissions(manifest.permissions);
                        }
                    }
                    
                    let run_res = evaluator.eval_program(&program);

                    // Si al terminar no se llamó a Task.reply, completamos
                    let mut reg = registry().lock().unwrap();
                    if let Some(TaskState::Running) = reg.get(&task_id) {
                        match run_res {
                            Some(_) => {
                                reg.insert(task_id, TaskState::Finished { result: "".to_string() });
                            }
                            None => {
                                reg.insert(task_id, TaskState::Failed { error: "Runtime execution failed".to_string() });
                            }
                        }
                    }
                });

                if handle_res.is_err() {
                    eprintln!("❌ ERROR: Task.run: failed to spawn thread");
                    let mut reg = registry().lock().unwrap();
                    reg.insert(task_id, TaskState::Failed { error: "Thread spawn failed".to_string() });
                    return EvalResult::Error;
                }

                EvalResult::Value(self.int_ref(task_id))
            }

            "message" => {
                // Task.message() -> string
                if dot_call.arguments.len() != 0 {
                    eprintln!("❌ ERROR: Task.message() requires 0 arguments");
                    return EvalResult::Error;
                }
                let msg = self.task_arg.clone().unwrap_or_default();
                EvalResult::Value(self.alloc(ObjectData::Str(msg)))
            }

            "reply" => {
                // Task.reply(result_string)
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Task.reply(result_string) requires 1 argument");
                    return EvalResult::Error;
                }
                let result_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    _ => return EvalResult::Error,
                };
                let result_str = match self.resolve(result_ref).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => {
                        eprintln!("❌ ERROR: Task.reply result must be a string");
                        return EvalResult::Error;
                    }
                };

                if let Some(task_id) = self.task_id {
                    let mut reg = registry().lock().unwrap();
                    reg.insert(task_id, TaskState::Finished { result: result_str });
                } else {
                    eprintln!("⚠️ WARNING: Task.reply called outside of a background task");
                }
                EvalResult::Value(self.null_ref)
            }

            "poll" => {
                // Task.poll(taskId) -> string | null
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Task.poll(taskId) requires 1 argument");
                    return EvalResult::Error;
                }
                let id_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    _ => return EvalResult::Error,
                };
                let task_id = match self.resolve(id_ref).cloned() {
                    Some(ObjectData::Integer(id)) => id,
                    _ => {
                        eprintln!("❌ ERROR: Task.poll: taskId must be an integer");
                        return EvalResult::Error;
                    }
                };

                let reg = registry().lock().unwrap();
                match reg.get(&task_id) {
                    Some(TaskState::Running) => EvalResult::Value(self.null_ref),
                    Some(TaskState::Finished { result }) => {
                        EvalResult::Value(self.alloc(ObjectData::Str(result.clone())))
                    }
                    Some(TaskState::Failed { error }) => {
                        eprintln!("❌ ERROR: Task {} failed: {}", task_id, error);
                        EvalResult::Value(self.alloc(ObjectData::Str(format!("ERROR: {}", error))))
                    }
                    None => {
                        eprintln!("❌ ERROR: Task.poll: task {} not found", task_id);
                        EvalResult::Error
                    }
                }
            }

            "isDone" => {
                // Task.isDone(taskId) -> bool
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Task.isDone(taskId) requires 1 argument");
                    return EvalResult::Error;
                }
                let id_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    _ => return EvalResult::Error,
                };
                let task_id = match self.resolve(id_ref).cloned() {
                    Some(ObjectData::Integer(id)) => id,
                    _ => {
                        eprintln!("❌ ERROR: Task.isDone: taskId must be an integer");
                        return EvalResult::Error;
                    }
                };

                let reg = registry().lock().unwrap();
                let done = match reg.get(&task_id) {
                    Some(TaskState::Running) => false,
                    _ => true,
                };
                EvalResult::Value(if done { self.true_ref } else { self.false_ref })
            }

            _ => {
                eprintln!("❌ ERROR: Unknown Task method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }
}
