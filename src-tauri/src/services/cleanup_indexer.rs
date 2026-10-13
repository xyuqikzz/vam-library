//! Keeps the cleanup reference index current in the background, so that a
//! cleanup preview only has to read packages that changed since the last run.
//!
//! The scheduler is independent of Tauri: the app supplies an [`IndexHost`]
//! that knows how to run one refresh and whether it may run right now.
//! Rules:
//! - requests are coalesced and only acted on after a quiet period;
//! - nothing runs while a cleanup preview/plan/execution is in progress, and a
//!   running refresh yields (is cancelled) when one starts;
//! - nothing runs while the game is running, and a refresh stops when the game
//!   starts, resuming after it exits;
//! - file reads happen on two low-priority worker threads ([`background_pool`]).
//!
//! Cancelling a refresh never loses work: the reference cache keeps every
//! entry completed so far.
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct IndexStatus {
    /// `idle` | `waiting` | `paused_game` | `running` | `ready` | `disabled` | `error`
    pub state: String,
    /// Catalog stage while running (`checking_files`, `building_index`, ...).
    pub stage: String,
    pub current: usize,
    pub total: usize,
    pub message: Option<String>,
    /// RFC 3339 time of the last completed refresh.
    pub finished_at: Option<String>,
}

pub trait IndexHost: Send + Sync + 'static {
    /// The user has background indexing switched on.
    fn enabled(&self) -> bool;
    /// A cleanup preview, pending plan or execution is in progress.
    fn busy(&self) -> bool;
    /// The game is (or may be) running for the current instance.
    fn game_running(&self) -> bool;
    /// Bring the reference index up to date. Must stop promptly once `cancel`
    /// is set and return an error in that case.
    fn run(
        &self,
        cancel: &AtomicBool,
        report: &mut dyn FnMut(&str, usize, usize),
    ) -> Result<(), String>;
    fn publish(&self, status: &IndexStatus);
}

#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// Quiet period after the last request before a refresh starts.
    pub debounce: Duration,
    /// How often to re-check while a cleanup is in progress.
    pub poll: Duration,
    /// How often to check whether the game is running.
    pub game_poll: Duration,
    /// Stop waiting for a cleanup that never finishes (for example an
    /// abandoned preview); running then only risks a "preview again" prompt.
    pub max_busy_wait: Duration,
    /// Minimum interval between progress updates.
    pub report_interval: Duration,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            debounce: Duration::from_secs(15),
            poll: Duration::from_secs(5),
            game_poll: Duration::from_secs(60),
            max_busy_wait: Duration::from_secs(30 * 60),
            report_interval: Duration::from_millis(300),
        }
    }
}

enum Outcome {
    Done,
    GameStarted,
    Preempted,
}

pub struct BackgroundIndexer {
    timing: Timing,
    requests: AtomicU64,
    running: AtomicBool,
    job: Mutex<Option<Arc<AtomicBool>>>,
    preempted: AtomicBool,
    status: Mutex<IndexStatus>,
}
impl Default for BackgroundIndexer {
    fn default() -> Self {
        Self::new(Timing::default())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
fn state(name: &str) -> IndexStatus {
    IndexStatus {
        state: name.into(),
        ..IndexStatus::default()
    }
}

impl BackgroundIndexer {
    pub fn new(timing: Timing) -> Self {
        Self {
            timing,
            requests: AtomicU64::new(0),
            running: AtomicBool::new(false),
            job: Mutex::new(None),
            preempted: AtomicBool::new(false),
            status: Mutex::new(state("idle")),
        }
    }

    pub fn status(&self) -> IndexStatus {
        lock(&self.status).clone()
    }

    fn set(&self, host: &dyn IndexHost, mut status: IndexStatus) {
        let mut current = lock(&self.status);
        if status.finished_at.is_none() {
            status.finished_at = current.finished_at.clone();
        }
        if *current != status {
            *current = status.clone();
            drop(current);
            host.publish(&status);
        }
    }

    /// Ask for a refresh once `delay` has passed without further requests.
    /// Requests made while a refresh runs cause one more refresh afterwards.
    pub fn schedule(self: &Arc<Self>, host: Arc<dyn IndexHost>, delay: Duration) {
        self.requests.fetch_add(1, Ordering::SeqCst);
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let this = Arc::clone(self);
        let spawned = thread::Builder::new()
            .name("cleanup-index".into())
            .spawn(move || this.worker(host, delay));
        if let Err(e) = spawned {
            log::warn!("无法启动后台引用索引: {e}");
            self.running.store(false, Ordering::SeqCst);
        }
    }

    /// Cancel a running refresh so a foreground cleanup has the disk to
    /// itself. The refresh resumes once the cleanup is over.
    pub fn preempt(&self) {
        if let Some(cancel) = lock(&self.job).clone() {
            self.preempted.store(true, Ordering::SeqCst);
            cancel.store(true, Ordering::SeqCst);
        }
    }

    fn worker(self: Arc<Self>, host: Arc<dyn IndexHost>, mut delay: Duration) {
        loop {
            let mut seen = self.requests.load(Ordering::SeqCst);
            loop {
                thread::sleep(delay);
                let now = self.requests.load(Ordering::SeqCst);
                if now == seen {
                    break;
                }
                seen = now;
                delay = self.timing.debounce;
            }
            self.refresh(&host);
            self.running.store(false, Ordering::SeqCst);
            if self.requests.load(Ordering::SeqCst) == seen
                || self
                    .running
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
            {
                return;
            }
            delay = self.timing.debounce;
        }
    }

    fn refresh(&self, host: &Arc<dyn IndexHost>) {
        let mut busy_since: Option<Instant> = None;
        loop {
            if !host.enabled() {
                self.set(&**host, state("disabled"));
                return;
            }
            if host.busy() {
                let since = *busy_since.get_or_insert_with(Instant::now);
                if since.elapsed() < self.timing.max_busy_wait {
                    self.set(&**host, state("waiting"));
                    thread::sleep(self.timing.poll);
                    continue;
                }
            } else {
                busy_since = None;
            }
            if host.game_running() {
                self.set(&**host, state("paused_game"));
                thread::sleep(self.timing.game_poll);
                continue;
            }
            match self.run_job(host) {
                Outcome::Done => return,
                Outcome::GameStarted => self.set(&**host, state("paused_game")),
                Outcome::Preempted => self.set(&**host, state("waiting")),
            }
        }
    }

    fn run_job(&self, host: &Arc<dyn IndexHost>) -> Outcome {
        let cancel = Arc::new(AtomicBool::new(false));
        self.preempted.store(false, Ordering::SeqCst);
        *lock(&self.job) = Some(Arc::clone(&cancel));
        // A host may become busy between the check and this point.
        if host.busy() {
            self.preempted.store(true, Ordering::SeqCst);
            cancel.store(true, Ordering::SeqCst);
        }
        self.set(&**host, state("running"));
        let game_started = AtomicBool::new(false);
        let finished = (Mutex::new(false), Condvar::new());
        let result = thread::scope(|scope| {
            scope.spawn(|| loop {
                let guard = lock(&finished.0);
                let (guard, _) = finished
                    .1
                    .wait_timeout_while(guard, self.timing.game_poll, |done| !*done)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if *guard {
                    return;
                }
                drop(guard);
                if host.game_running() {
                    game_started.store(true, Ordering::SeqCst);
                    cancel.store(true, Ordering::SeqCst);
                    return;
                }
            });
            let mut last_report: Option<Instant> = None;
            let mut last_stage = String::new();
            let result = if cancel.load(Ordering::SeqCst) {
                Err("操作已取消".to_string())
            } else {
                host.run(&cancel, &mut |stage, current, total| {
                    let due =
                        last_report.map_or(true, |at| at.elapsed() >= self.timing.report_interval);
                    if stage != last_stage || due || (total > 0 && current == total) {
                        last_report = Some(Instant::now());
                        last_stage = stage.to_string();
                        self.set(
                            &**host,
                            IndexStatus {
                                state: "running".into(),
                                stage: stage.into(),
                                current,
                                total,
                                ..IndexStatus::default()
                            },
                        );
                    }
                })
            };
            *lock(&finished.0) = true;
            finished.1.notify_all();
            result
        });
        *lock(&self.job) = None;
        match result {
            Ok(()) => {
                self.set(
                    &**host,
                    IndexStatus {
                        state: "ready".into(),
                        finished_at: Some(chrono::Utc::now().to_rfc3339()),
                        ..IndexStatus::default()
                    },
                );
                Outcome::Done
            }
            Err(_) if game_started.load(Ordering::SeqCst) => Outcome::GameStarted,
            Err(_) if self.preempted.load(Ordering::SeqCst) => Outcome::Preempted,
            Err(message) => {
                log::warn!("后台引用索引未完成: {message}");
                self.set(
                    &**host,
                    IndexStatus {
                        state: "error".into(),
                        message: Some(message),
                        ..IndexStatus::default()
                    },
                );
                Outcome::Done
            }
        }
    }
}

/// Two low-priority workers for background file reads. On Windows they run in
/// background processing mode, which lowers both CPU and I/O priority so the
/// app and other programs keep the disk. Database writes are not done on
/// these threads (see `resource_cleanup::with_worker_pool`), so a slow
/// background thread never holds the shared database lock.
pub fn background_pool() -> Result<Arc<rayon::ThreadPool>, String> {
    static POOL: OnceLock<Arc<rayon::ThreadPool>> = OnceLock::new();
    if let Some(pool) = POOL.get() {
        return Ok(Arc::clone(pool));
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .thread_name(|i| format!("cleanup-index-{i}"))
        .start_handler(|_| lower_current_thread_priority())
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Arc::clone(POOL.get_or_init(|| Arc::new(pool))))
}

#[cfg(windows)]
fn lower_current_thread_priority() {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
    }
    const THREAD_MODE_BACKGROUND_BEGIN: i32 = 0x0001_0000;
    // SAFETY: GetCurrentThread returns a pseudo handle for the calling thread
    // that needs no closing; background mode may only be set on that thread.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}
#[cfg(not(windows))]
fn lower_current_thread_priority() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct FakeHost {
        disabled: AtomicBool,
        busy: AtomicBool,
        game: AtomicBool,
        fail: AtomicBool,
        runs: AtomicUsize,
        completed: AtomicUsize,
        /// Steps of 2 ms per run.
        steps: AtomicUsize,
        statuses: Mutex<Vec<IndexStatus>>,
    }
    impl IndexHost for FakeHost {
        fn enabled(&self) -> bool {
            !self.disabled.load(Ordering::SeqCst)
        }
        fn busy(&self) -> bool {
            self.busy.load(Ordering::SeqCst)
        }
        fn game_running(&self) -> bool {
            self.game.load(Ordering::SeqCst)
        }
        fn run(
            &self,
            cancel: &AtomicBool,
            report: &mut dyn FnMut(&str, usize, usize),
        ) -> Result<(), String> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            let steps = self.steps.load(Ordering::SeqCst).max(1);
            for step in 0..steps {
                if cancel.load(Ordering::SeqCst) {
                    return Err("操作已取消".into());
                }
                report("building_index", step + 1, steps);
                thread::sleep(Duration::from_millis(2));
            }
            if self.fail.load(Ordering::SeqCst) {
                return Err("boom".into());
            }
            self.completed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn publish(&self, status: &IndexStatus) {
            self.statuses.lock().unwrap().push(status.clone());
        }
    }
    fn fast() -> Timing {
        Timing {
            debounce: Duration::from_millis(20),
            poll: Duration::from_millis(5),
            game_poll: Duration::from_millis(5),
            max_busy_wait: Duration::from_secs(60),
            report_interval: Duration::from_millis(1),
        }
    }
    fn setup(steps: usize) -> (Arc<BackgroundIndexer>, Arc<FakeHost>) {
        let host = Arc::new(FakeHost::default());
        host.steps.store(steps, Ordering::SeqCst);
        (Arc::new(BackgroundIndexer::new(fast())), host)
    }
    fn schedule(indexer: &Arc<BackgroundIndexer>, host: &Arc<FakeHost>) {
        indexer.schedule(
            Arc::clone(host) as Arc<dyn IndexHost>,
            Duration::from_millis(20),
        );
    }
    fn wait_for(what: &str, condition: impl Fn() -> bool) {
        let started = Instant::now();
        while !condition() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "timed out: {what}"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }
    fn idle(indexer: &BackgroundIndexer) -> bool {
        !indexer.running.load(Ordering::SeqCst)
    }

    #[test]
    fn coalesces_requests_and_reports_ready() {
        let (indexer, host) = setup(3);
        for _ in 0..5 {
            schedule(&indexer, &host);
        }
        wait_for("ready", || {
            indexer.status().state == "ready" && idle(&indexer)
        });
        assert_eq!(host.runs.load(Ordering::SeqCst), 1);
        let statuses = host.statuses.lock().unwrap();
        assert!(statuses
            .iter()
            .any(|s| s.state == "running" && s.current == 3 && s.total == 3));
        assert!(statuses.last().unwrap().finished_at.is_some());
    }

    #[test]
    fn a_request_during_a_refresh_runs_one_more_refresh() {
        let (indexer, host) = setup(40);
        schedule(&indexer, &host);
        wait_for("started", || host.runs.load(Ordering::SeqCst) == 1);
        schedule(&indexer, &host);
        schedule(&indexer, &host);
        wait_for("second run", || {
            host.completed.load(Ordering::SeqCst) == 2 && idle(&indexer)
        });
        assert_eq!(host.runs.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn waits_while_a_cleanup_is_in_progress() {
        let (indexer, host) = setup(1);
        host.busy.store(true, Ordering::SeqCst);
        schedule(&indexer, &host);
        wait_for("waiting", || indexer.status().state == "waiting");
        thread::sleep(Duration::from_millis(50));
        assert_eq!(host.runs.load(Ordering::SeqCst), 0);
        host.busy.store(false, Ordering::SeqCst);
        wait_for("ready", || indexer.status().state == "ready");
    }

    #[test]
    fn pauses_while_the_game_runs_and_resumes_after_it_exits() {
        let (indexer, host) = setup(1);
        host.game.store(true, Ordering::SeqCst);
        schedule(&indexer, &host);
        wait_for("paused", || indexer.status().state == "paused_game");
        thread::sleep(Duration::from_millis(30));
        assert_eq!(host.runs.load(Ordering::SeqCst), 0);
        host.game.store(false, Ordering::SeqCst);
        wait_for("ready", || indexer.status().state == "ready");
    }

    #[test]
    fn stops_when_the_game_starts_and_finishes_after_it_exits() {
        let (indexer, host) = setup(5_000);
        schedule(&indexer, &host);
        wait_for("running", || indexer.status().state == "running");
        host.game.store(true, Ordering::SeqCst);
        wait_for("paused", || indexer.status().state == "paused_game");
        assert_eq!(host.completed.load(Ordering::SeqCst), 0);
        host.steps.store(2, Ordering::SeqCst);
        host.game.store(false, Ordering::SeqCst);
        wait_for("ready", || {
            indexer.status().state == "ready" && idle(&indexer)
        });
        assert_eq!(host.runs.load(Ordering::SeqCst), 2);
        assert_eq!(host.completed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn yields_to_a_foreground_cleanup_and_resumes_afterwards() {
        let (indexer, host) = setup(5_000);
        schedule(&indexer, &host);
        wait_for("running", || indexer.status().state == "running");
        host.busy.store(true, Ordering::SeqCst);
        indexer.preempt();
        wait_for("waiting", || indexer.status().state == "waiting");
        assert_eq!(host.completed.load(Ordering::SeqCst), 0);
        host.steps.store(2, Ordering::SeqCst);
        host.busy.store(false, Ordering::SeqCst);
        wait_for("ready", || {
            indexer.status().state == "ready" && idle(&indexer)
        });
        assert_eq!(host.completed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn disabled_indexing_never_runs_and_failures_are_reported() {
        let (indexer, host) = setup(1);
        host.disabled.store(true, Ordering::SeqCst);
        schedule(&indexer, &host);
        wait_for("disabled", || {
            indexer.status().state == "disabled" && idle(&indexer)
        });
        assert_eq!(host.runs.load(Ordering::SeqCst), 0);
        host.disabled.store(false, Ordering::SeqCst);
        host.fail.store(true, Ordering::SeqCst);
        schedule(&indexer, &host);
        wait_for("error", || {
            indexer.status().state == "error" && idle(&indexer)
        });
        assert_eq!(indexer.status().message.as_deref(), Some("boom"));
    }

    #[test]
    fn background_pool_runs_work_off_the_calling_thread() {
        let pool = background_pool().unwrap();
        let caller = thread::current().id();
        let worker = pool.install(|| thread::current().id());
        assert_ne!(caller, worker);
        assert!(Arc::ptr_eq(&pool, &background_pool().unwrap()));
    }
}
