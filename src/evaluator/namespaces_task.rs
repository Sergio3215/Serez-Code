use super::ExecutionFlow;
use super::{EvalResult, ProgramOutcome};
use crate::ast;
use crate::region::ObjectData;
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub(crate) const MAX_CONCURRENT_TASKS: usize = 32;
pub(crate) const MAX_TASK_RECORDS: usize = 256;
pub(crate) const MAX_TASK_MESSAGE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_TASK_SOURCE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
enum TaskState {
    Running { reply: Option<String> },
    Finished { result: String },
    Failed { error: String },
}

/// Runtime service shared by a top-level evaluator and its worker descendants.
///
/// It deliberately is not process-global: task IDs and replies from one embedder
/// must not be observable by another evaluator in the same host process.
pub(crate) struct TaskRuntime {
    registry: Mutex<HashMap<i64, TaskState>>,
    next_id: AtomicI64,
}

impl Default for TaskRuntime {
    fn default() -> Self {
        Self {
            registry: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
        }
    }
}

impl TaskRuntime {
    fn lock_registry(&self) -> MutexGuard<'_, HashMap<i64, TaskState>> {
        // No user evaluation runs while this lock is held, so recovering the
        // structurally valid map is safer than turning poison into a host panic.
        self.registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn allocate_id(&self) -> Option<i64> {
        self.next_id
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1))
            .ok()
    }
}

fn running_count(registry: &HashMap<i64, TaskState>) -> usize {
    registry
        .values()
        .filter(|state| matches!(state, TaskState::Running { .. }))
        .count()
}

fn evict_oldest_terminal_if_full(registry: &mut HashMap<i64, TaskState>) {
    if registry.len() < MAX_TASK_RECORDS {
        return;
    }
    let oldest_terminal = registry
        .iter()
        .filter(|(_, state)| !matches!(state, TaskState::Running { .. }))
        .map(|(id, _)| *id)
        .min();
    if let Some(id) = oldest_terminal {
        registry.remove(&id);
    }
}

fn worker_outcome(outcome: ProgramOutcome) -> Result<(), String> {
    match outcome {
        ProgramOutcome::Value(_) => Ok(()),
        ProgramOutcome::RuntimeError(error) => Err(format!(
            "[{}] {}: {}",
            error.code,
            error.kind.as_deref().unwrap_or_default(),
            error.message
        )),
        ProgramOutcome::UncaughtException { message } => {
            Err(format!("Uncaught exception: {}", message))
        }
        ProgramOutcome::InvalidControlFlow(flow) => {
            Err(format!("Invalid top-level control flow: {:?}", flow))
        }
        ProgramOutcome::UnstructuredError => Err("Unstructured runtime failure".to_string()),
    }
}

fn bounded_worker_error(error: String) -> String {
    if error.len() <= MAX_TASK_MESSAGE_BYTES {
        error
    } else {
        format!(
            "[SZ6002] ResourceError: Task worker error exceeds the {} MiB message limit",
            MAX_TASK_MESSAGE_BYTES / (1024 * 1024)
        )
    }
}

/// The task this evaluator *is*, and the registry it shares.
///
/// Three fields of `Evaluator` until M6.3. `runtime` is shared by a top-level
/// evaluator and every worker it creates - keeping it here rather than in a
/// global isolates unrelated evaluators while still letting tasks nest. `id` and
/// `arg` are `None` in the parent and `Some` in a worker, so together they answer
/// "am I a worker, and what was I given?" - one question, previously asked in
/// three places.
#[derive(Default)]
pub struct TaskContext {
    pub runtime: Arc<TaskRuntime>,
    /// `None` at the top level; `Some` inside a worker.
    pub id: Option<i64>,
    /// The message the parent passed to this worker.
    pub arg: Option<String>,
}

impl super::Evaluator {
    pub(super) fn eval_task_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        if let Some(error) = self.require_permission("Task", "Task") {
            return error;
        }

        match dot_call.method.as_str() {
            "run" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind(
                        "TypeError",
                        "Task.run(script_path, arg_string) requires 2 arguments",
                    );
                }

                let path_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(value)) => value,
                    other => return other,
                };
                let script_path = match self.resolve(path_ref).cloned() {
                    Some(ObjectData::Str(path)) => path,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Task.run: script_path must be a string");
                    }
                };

                let arg_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    Ok(ExecutionFlow::Value(value)) => value,
                    other => return other,
                };
                let arg_string = match self.resolve(arg_ref).cloned() {
                    Some(ObjectData::Str(arg)) => arg,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Task.run: arg_string must be a string");
                    }
                };

                if arg_string.len() > MAX_TASK_MESSAGE_BYTES {
                    return self.fatal_err_kind(
                        "ResourceError",
                        format!(
                            "Task.run argument exceeds the {} MiB message limit",
                            MAX_TASK_MESSAGE_BYTES / (1024 * 1024)
                        ),
                    );
                }

                let runtime = Arc::clone(&self.task.runtime);
                let task_id = {
                    let mut reg = runtime.lock_registry();
                    if running_count(&reg) >= MAX_CONCURRENT_TASKS {
                        drop(reg);
                        return self.fatal_err_kind(
                            "ResourceError",
                            format!(
                                "Task.run reached the limit of {} concurrent workers",
                                MAX_CONCURRENT_TASKS
                            ),
                        );
                    }
                    evict_oldest_terminal_if_full(&mut reg);
                    let Some(task_id) = runtime.allocate_id() else {
                        drop(reg);
                        return self.fatal_err_kind(
                            "ResourceError",
                            "Task ID space is exhausted for this runtime",
                        );
                    };
                    reg.insert(task_id, TaskState::Running { reply: None });
                    task_id
                };

                let worker_lockdown = self.security.lockdown;
                let inherited_permissions: Vec<String> = if worker_lockdown {
                    self.security.granted.iter().cloned().collect()
                } else {
                    Vec::new()
                };
                let worker_runtime = Arc::clone(&runtime);
                let worker_path = script_path.clone();
                let worker_arg = arg_string.clone();

                let builder = std::thread::Builder::new()
                    .name(format!("task-worker-{}", task_id))
                    .stack_size(16 * 1024 * 1024);

                let handle_res = builder.spawn(move || {
                    let run_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let file = std::fs::File::open(&worker_path).map_err(|error| {
                            format!("Error reading file '{}': {}", worker_path, error)
                        })?;
                        let mut input = String::new();
                        file.take((MAX_TASK_SOURCE_BYTES + 1) as u64)
                            .read_to_string(&mut input)
                            .map_err(|error| {
                                format!("Error reading file '{}': {}", worker_path, error)
                            })?;
                        if input.len() > MAX_TASK_SOURCE_BYTES {
                            return Err(format!(
                                "[SZ6002] ResourceError: Task worker source exceeds {} MiB",
                                MAX_TASK_SOURCE_BYTES / (1024 * 1024)
                            ));
                        }

                        let source_lines: Vec<String> = input.lines().map(str::to_string).collect();
                        let lexer = crate::lexer::Lexer::new(input);
                        let mut parser = crate::parser::Parser::new(lexer);
                        parser.set_source(source_lines.clone());
                        parser.set_source_name(&worker_path);
                        let program = parser.parse_program();
                        if parser.has_errors() {
                            return Err("Syntax/Parsing error in worker script".to_string());
                        }

                        let mut checker = crate::type_checker::TypeChecker::new(&program);
                        checker.check();

                        let mut evaluator = crate::evaluator::Evaluator::new();
                        evaluator.set_source(source_lines);
                        evaluator.set_task_runtime_context(
                            task_id,
                            worker_arg,
                            Arc::clone(&worker_runtime),
                        );
                        let file_path = std::path::Path::new(&worker_path);
                        evaluator.set_current_file(file_path);

                        if worker_lockdown {
                            evaluator.set_permissions(inherited_permissions);
                            evaluator.set_lockdown(true);
                        } else if let Some(dir) = file_path.parent() {
                            let dir = if dir == std::path::Path::new("") {
                                std::path::Path::new(".")
                            } else {
                                dir
                            };
                            if let Ok(manifest) = crate::package_manager::SerezManifest::load(dir) {
                                evaluator.set_permissions(manifest.permissions);
                            }
                        }

                        worker_outcome(evaluator.eval_program_outcome(&program))
                    }));

                    let mut reg = worker_runtime.lock_registry();
                    let reply = match reg.get(&task_id) {
                        Some(TaskState::Running { reply }) => reply.clone(),
                        _ => None,
                    };
                    let final_state = match run_res {
                        Ok(Ok(())) => TaskState::Finished {
                            result: reply.unwrap_or_default(),
                        },
                        Ok(Err(error)) => TaskState::Failed {
                            error: bounded_worker_error(error),
                        },
                        Err(_) => TaskState::Failed {
                            error: "Worker thread panicked".to_string(),
                        },
                    };
                    reg.insert(task_id, final_state);
                });

                if let Err(error) = handle_res {
                    runtime.lock_registry().remove(&task_id);
                    return self.fatal_err_kind(
                        "ResourceError",
                        format!("Task.run failed to spawn a worker thread: {}", error),
                    );
                }

                Ok(ExecutionFlow::Value(self.int_ref(task_id)))
            }

            "message" => {
                if !dot_call.arguments.is_empty() {
                    return self.rt_err_kind("TypeError", "Task.message() requires 0 arguments");
                }
                let message = self.task.arg.clone().unwrap_or_default();
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(message))))
            }

            "reply" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Task.reply(result_string) requires 1 argument");
                }
                let result_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(value)) => value,
                    other => return other,
                };
                let result = match self.resolve(result_ref).cloned() {
                    Some(ObjectData::Str(result)) => result,
                    _ => {
                        return self.rt_err_kind("TypeError", "Task.reply result must be a string");
                    }
                };
                if result.len() > MAX_TASK_MESSAGE_BYTES {
                    return self.fatal_err_kind(
                        "ResourceError",
                        format!(
                            "Task.reply result exceeds the {} MiB message limit",
                            MAX_TASK_MESSAGE_BYTES / (1024 * 1024)
                        ),
                    );
                }

                if let Some(task_id) = self.task.id {
                    let runtime = Arc::clone(&self.task.runtime);
                    let mut reg = runtime.lock_registry();
                    if let Some(TaskState::Running { reply }) = reg.get_mut(&task_id) {
                        *reply = Some(result);
                    }
                } else {
                    eprintln!("⚠️ WARNING: Task.reply called outside of a background task");
                }
                Ok(ExecutionFlow::Value(self.null_ref))
            }

            "poll" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Task.poll(taskId) requires 1 argument");
                }
                let id_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(value)) => value,
                    other => return other,
                };
                let task_id = match self.resolve(id_ref).cloned() {
                    Some(ObjectData::Integer(id)) => id,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Task.poll: taskId must be an integer");
                    }
                };

                let runtime = Arc::clone(&self.task.runtime);
                let reg = runtime.lock_registry();
                match reg.get(&task_id) {
                    Some(TaskState::Running { .. }) => Ok(ExecutionFlow::Value(self.null_ref)),
                    Some(TaskState::Finished { result }) => Ok(ExecutionFlow::Value(
                        self.alloc(ObjectData::Str(result.clone())),
                    )),
                    Some(TaskState::Failed { error }) => Ok(ExecutionFlow::Value(
                        self.alloc(ObjectData::Str(format!("ERROR: {}", error))),
                    )),
                    None => {
                        drop(reg);
                        self.rt_err_kind(
                            "ReferenceError",
                            format!("Task.poll: task {} not found", task_id),
                        )
                    }
                }
            }

            "isDone" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Task.isDone(taskId) requires 1 argument");
                }
                let id_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    Ok(ExecutionFlow::Value(value)) => value,
                    other => return other,
                };
                let task_id = match self.resolve(id_ref).cloned() {
                    Some(ObjectData::Integer(id)) => id,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Task.isDone: taskId must be an integer");
                    }
                };

                let runtime = Arc::clone(&self.task.runtime);
                let reg = runtime.lock_registry();
                let done = match reg.get(&task_id) {
                    Some(TaskState::Running { .. }) => false,
                    Some(TaskState::Finished { .. } | TaskState::Failed { .. }) => true,
                    None => {
                        drop(reg);
                        return self.rt_err_kind(
                            "ReferenceError",
                            format!("Task.isDone: task {} not found", task_id),
                        );
                    }
                };
                Ok(ExecutionFlow::Value(if done {
                    self.true_ref
                } else {
                    self.false_ref
                }))
            }

            method => self.rt_err_kind(
                "ReferenceError",
                format!("Unknown Task method '{}'", method),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisoned_registry_is_recovered_without_panicking() {
        let runtime = Arc::new(TaskRuntime::default());
        let worker_runtime = Arc::clone(&runtime);
        let _ = std::thread::spawn(move || {
            let _guard = worker_runtime.registry.lock().unwrap();
            panic!("poison task registry for regression coverage");
        })
        .join();

        assert!(runtime.registry.is_poisoned());
        runtime
            .lock_registry()
            .insert(1, TaskState::Running { reply: None });
        assert_eq!(running_count(&runtime.lock_registry()), 1);
    }

    #[test]
    fn evaluator_task_runtimes_are_observably_isolated() {
        let first = super::super::Evaluator::new();
        let mut second = super::super::Evaluator::new();
        assert!(!Arc::ptr_eq(&first.task.runtime, &second.task.runtime));

        first.task.runtime.lock_registry().insert(
            1,
            TaskState::Finished {
                result: "private-result".to_string(),
            },
        );
        second.set_permissions(vec!["Task".to_string()]);
        let lexer = crate::lexer::Lexer::new("Task.poll(1);".to_string());
        let mut parser = crate::parser::Parser::new(lexer);
        let program = parser.parse_program();
        assert!(!parser.has_errors());
        match second.eval_program_outcome(&program) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4001");
                assert_eq!(error.kind.as_deref(), Some("ReferenceError"));
            }
            other => panic!("second evaluator observed first task: {other:?}"),
        }
    }

    #[test]
    fn concurrent_limit_counts_only_live_workers() {
        let mut registry = HashMap::new();
        for id in 0..MAX_CONCURRENT_TASKS as i64 {
            registry.insert(id, TaskState::Running { reply: None });
        }
        registry.insert(
            100,
            TaskState::Finished {
                result: String::new(),
            },
        );
        registry.insert(
            101,
            TaskState::Failed {
                error: String::new(),
            },
        );
        assert_eq!(running_count(&registry), MAX_CONCURRENT_TASKS);
    }

    #[test]
    fn record_limit_evicts_only_the_oldest_terminal_task() {
        let mut registry = HashMap::new();
        registry.insert(0, TaskState::Running { reply: None });
        for id in 1..MAX_TASK_RECORDS as i64 {
            registry.insert(
                id,
                TaskState::Finished {
                    result: String::new(),
                },
            );
        }

        evict_oldest_terminal_if_full(&mut registry);

        assert_eq!(registry.len(), MAX_TASK_RECORDS - 1);
        assert!(registry.contains_key(&0));
        assert!(!registry.contains_key(&1));
    }

    #[test]
    fn concurrent_limit_returns_a_fatal_resource_error_without_spawning() {
        let src = r#"
            try { Task.run("unused.sz", ""); }
            catch (_) { throw "resource limit became catchable"; }
        "#;
        let lexer = crate::lexer::Lexer::new(src.to_string());
        let mut parser = crate::parser::Parser::new(lexer);
        let program = parser.parse_program();
        assert!(!parser.has_errors());

        let mut evaluator = super::super::Evaluator::new();
        evaluator.set_permissions(vec!["Task".to_string()]);
        {
            let mut registry = evaluator.task.runtime.lock_registry();
            for id in 0..MAX_CONCURRENT_TASKS as i64 {
                registry.insert(id, TaskState::Running { reply: None });
            }
        }

        match evaluator.eval_program_outcome(&program) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ6002");
                assert_eq!(error.kind.as_deref(), Some("ResourceError"));
            }
            other => panic!("expected fatal Task resource error, got {other:?}"),
        }
    }
}
