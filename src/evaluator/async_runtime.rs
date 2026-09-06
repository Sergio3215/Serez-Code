//! Central asynchronous runtime for Serez Code v11.1.0 (DEC-ASYNC-001).
//!
//! Replaces unconstrained `thread::spawn` and busy polling (`recv_timeout` + `yield_now`)
//! with a bounded I/O worker pool, notification-driven completion via `Condvar`,
//! absolute shared deadlines across redirects, and atomic cancellation tokens.

use super::builtins::FetchResponse;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Default ceiling on concurrent pending asynchronous operations across the runtime.
pub const DEFAULT_MAX_PENDING_ASYNC_OPERATIONS: usize = 256;
pub const ASYNC_MAX_PENDING_OPERATIONS: usize = DEFAULT_MAX_PENDING_ASYNC_OPERATIONS;

/// Maximum number of background I/O worker threads in the shared pool.
pub const ASYNC_IO_MAX_WORKERS: usize = 4;

/// Default operation timeout in seconds if unspecified.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Absolute host maximum timeout ceiling (5 minutes).
pub const MAX_HOST_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AsyncOperationId(pub u64);

#[derive(Debug)]
pub enum OperationStatus {
    Pending,
    Completed(FetchResponse),
    Failed(String),
    TimedOut,
    Cancelled,
}

/// Internal handle for a pending asynchronous operation.
pub struct PendingOperation {
    pub id: AsyncOperationId,
    pub start_time: Instant,
    pub deadline: Instant,
    pub cancellation_token: Arc<AtomicBool>,
    pub status: Arc<Mutex<OperationStatus>>,
    pub notifier: Arc<(Mutex<bool>, Condvar)>,
}

type BoxedTask = Box<dyn FnOnce() + Send + 'static>;

struct WorkerPool {
    queue: Mutex<VecDeque<BoxedTask>>,
    condvar: Condvar,
    active_workers: AtomicUsize,
    shutdown: AtomicBool,
}

impl WorkerPool {
    fn new(max_workers: usize) -> Arc<Self> {
        let pool = Arc::new(Self {
            queue: Mutex::new(VecDeque::new()),
            condvar: Condvar::new(),
            active_workers: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
        });

        for id in 0..max_workers {
            let p = Arc::clone(&pool);
            let _ = std::thread::Builder::new()
                .name(format!("sz-async-io-{}", id))
                .spawn(move || {
                    ACTIVE_WORKER_THREADS.fetch_add(1, Ordering::SeqCst);
                    loop {
                        let task = {
                            let mut queue = p.queue.lock().unwrap_or_else(|e| e.into_inner());
                            while queue.is_empty() && !p.shutdown.load(Ordering::SeqCst) {
                                queue = p.condvar.wait(queue).unwrap_or_else(|e| e.into_inner());
                            }
                            if p.shutdown.load(Ordering::SeqCst) && queue.is_empty() {
                                break;
                            }
                            queue.pop_front()
                        };

                        if let Some(task) = task {
                            task();
                        }
                    }
                    ACTIVE_WORKER_THREADS.fetch_sub(1, Ordering::SeqCst);
                });
        }

        pool
    }

    fn execute(&self, task: BoxedTask) {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.push_back(task);
        self.condvar.notify_one();
    }
}

/// Test observability metrics.
pub static PENDING_OPERATIONS_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static COMPLETED_OPERATIONS_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static TIMED_OUT_OPERATIONS_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static CANCELLED_OPERATIONS_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static ACTIVE_WORKER_THREADS: AtomicUsize = AtomicUsize::new(0);

#[cfg(any(test, debug_assertions))]
pub fn test_reset_async_metrics() {
    PENDING_OPERATIONS_COUNT.store(0, Ordering::SeqCst);
    COMPLETED_OPERATIONS_COUNT.store(0, Ordering::SeqCst);
    TIMED_OUT_OPERATIONS_COUNT.store(0, Ordering::SeqCst);
    CANCELLED_OPERATIONS_COUNT.store(0, Ordering::SeqCst);
}

/// Global shared async runtime.
pub struct AsyncRuntime {
    next_id: AtomicU64,
    pool: Arc<WorkerPool>,
    operations: Mutex<HashMap<AsyncOperationId, Arc<PendingOperationEntry>>>,
    max_pending: usize,
}

pub struct PendingOperationEntry {
    pub id: AsyncOperationId,
    pub deadline: Instant,
    pub cancellation_token: Arc<AtomicBool>,
    pub status: Arc<Mutex<OperationStatus>>,
    pub notifier: Arc<(Mutex<bool>, Condvar)>,
}

static RUNTIME_INSTANCE: std::sync::OnceLock<Arc<AsyncRuntime>> = std::sync::OnceLock::new();

pub fn get_runtime() -> Arc<AsyncRuntime> {
    RUNTIME_INSTANCE
        .get_or_init(|| {
            Arc::new(AsyncRuntime {
                next_id: AtomicU64::new(1),
                pool: WorkerPool::new(ASYNC_IO_MAX_WORKERS),
                operations: Mutex::new(HashMap::new()),
                max_pending: DEFAULT_MAX_PENDING_ASYNC_OPERATIONS,
            })
        })
        .clone()
}

impl AsyncRuntime {
    pub fn new_with_capacity(max_pending: usize) -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pool: WorkerPool::new(ASYNC_IO_MAX_WORKERS),
            operations: Mutex::new(HashMap::new()),
            max_pending,
        }
    }

    /// Allocates an async operation handle enforcing the pending operation limit.
    pub fn allocate_operation(
        &self,
        timeout_secs: u64,
    ) -> Result<Arc<PendingOperationEntry>, String> {
        let mut ops = self.operations.lock().unwrap_or_else(|e| e.into_inner());
        if ops.len() >= self.max_pending {
            return Err(format!(
                "ResourceError: maximum pending async operations ({}) exceeded",
                self.max_pending
            ));
        }

        let id = AsyncOperationId(self.next_id.fetch_add(1, Ordering::SeqCst));
        let effective_timeout = timeout_secs.clamp(1, MAX_HOST_TIMEOUT_SECS);
        let deadline = Instant::now() + Duration::from_secs(effective_timeout);
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(OperationStatus::Pending));
        let notifier = Arc::new((Mutex::new(false), Condvar::new()));

        let entry = Arc::new(PendingOperationEntry {
            id,
            deadline,
            cancellation_token,
            status,
            notifier,
        });

        ops.insert(id, Arc::clone(&entry));
        PENDING_OPERATIONS_COUNT.fetch_add(1, Ordering::SeqCst);
        Ok(entry)
    }

    /// Retrieves a pending operation entry by its id.
    pub fn get_operation(&self, id: AsyncOperationId) -> Option<Arc<PendingOperationEntry>> {
        let ops = self.operations.lock().unwrap_or_else(|e| e.into_inner());
        ops.get(&id).cloned()
    }

    /// Submits background execution of an I/O task.
    pub fn submit_io_task<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.pool.execute(Box::new(task));
    }

    /// Marks an operation as completed with exactly-once guarantee.
    pub fn complete(&self, id: AsyncOperationId, result: Result<FetchResponse, String>) {
        let entry = {
            let ops = self.operations.lock().unwrap_or_else(|e| e.into_inner());
            ops.get(&id).cloned()
        };

        if let Some(entry) = entry {
            let mut status = entry.status.lock().unwrap_or_else(|e| e.into_inner());
            // Exactly-once: only transition from Pending
            if matches!(*status, OperationStatus::Pending) {
                match result {
                    Ok(resp) => *status = OperationStatus::Completed(resp),
                    Err(err) => *status = OperationStatus::Failed(err),
                }
                COMPLETED_OPERATIONS_COUNT.fetch_add(1, Ordering::SeqCst);

                // Notify scheduler waiting on condvar
                let (lock, cvar) = &*entry.notifier;
                let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());
                *done = true;
                cvar.notify_all();
            }
            // Late completion: if already TimedOut or Cancelled, discard safely.
        }
    }

    /// Waits for completion or deadline of an operation without busy polling.
    pub fn wait_for_completion(
        &self,
        entry: &PendingOperationEntry,
    ) -> Result<FetchResponse, String> {
        let (lock, cvar) = &*entry.notifier;
        let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());

        while !*done {
            let now = Instant::now();
            if now >= entry.deadline {
                break;
            }
            let remaining = entry.deadline.duration_since(now);
            let wait_res = cvar
                .wait_timeout(done, remaining)
                .unwrap_or_else(|e| e.into_inner());
            done = wait_res.0;
            if wait_res.1.timed_out() {
                break;
            }
        }

        // Check or transition state
        let mut status = entry.status.lock().unwrap_or_else(|e| e.into_inner());
        match &*status {
            OperationStatus::Completed(resp) => Ok(FetchResponse {
                status: resp.status,
                status_text: resp.status_text.clone(),
                headers: resp.headers.clone(),
                body: resp.body.clone(),
            }),
            OperationStatus::Failed(msg) => Err(msg.clone()),
            OperationStatus::Pending => {
                // Deadline expired: mark TimedOut and trigger cancellation token
                *status = OperationStatus::TimedOut;
                entry.cancellation_token.store(true, Ordering::SeqCst);
                TIMED_OUT_OPERATIONS_COUNT.fetch_add(1, Ordering::SeqCst);
                Err("request timed out".to_string())
            }
            OperationStatus::TimedOut => Err("request timed out".to_string()),
            OperationStatus::Cancelled => Err("operation cancelled".to_string()),
        }
    }

    /// Removes an operation from the registry and cleans up resources.
    pub fn remove_operation(&self, id: AsyncOperationId) {
        let mut ops = self.operations.lock().unwrap_or_else(|e| e.into_inner());
        if ops.remove(&id).is_some() {
            PENDING_OPERATIONS_COUNT.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Cancels an operation if still pending.
    pub fn cancel(&self, id: AsyncOperationId) {
        let entry = {
            let ops = self.operations.lock().unwrap_or_else(|e| e.into_inner());
            ops.get(&id).cloned()
        };
        if let Some(entry) = entry {
            entry.cancellation_token.store(true, Ordering::SeqCst);
            let mut status = entry.status.lock().unwrap_or_else(|e| e.into_inner());
            if matches!(*status, OperationStatus::Pending) {
                *status = OperationStatus::Cancelled;
                CANCELLED_OPERATIONS_COUNT.fetch_add(1, Ordering::SeqCst);
                let (lock, cvar) = &*entry.notifier;
                let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());
                *done = true;
                cvar.notify_all();
            }
        }
    }
}
