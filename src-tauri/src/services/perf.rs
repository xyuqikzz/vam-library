//! Debug-only command timings; release builds neither record nor log paths/data.
pub struct CommandTimer {
    #[cfg(debug_assertions)]
    command: &'static str,
    #[cfg(debug_assertions)]
    started: std::time::Instant,
}
impl CommandTimer {
    pub fn new(command: &'static str) -> Self {
        let _ = command;
        Self {
            #[cfg(debug_assertions)] command,
            #[cfg(debug_assertions)] started: std::time::Instant::now(),
        }
    }
}
impl Drop for CommandTimer {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        eprintln!("[perf] {} {:.2}ms", self.command, self.started.elapsed().as_secs_f64() * 1000.0);
    }
}
