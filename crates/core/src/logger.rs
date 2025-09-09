use std::{
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
};

use once_cell::sync::Lazy;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    fmt::{
        format::{Format, Writer},
        MakeWriter,
    },
    EnvFilter,
};

static SHUTDOWN_IN_PROGRESS: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// A writer that adapts its behavior during application shutdown.
///
/// Uses buffered writing during normal operation for performance,
/// but switches to direct stdout writing during shutdown to ensure
/// log messages are flushed immediately.
struct ShutdownAwareWriter {
    buffer: std::io::BufWriter<std::io::Stdout>,
}

impl ShutdownAwareWriter {
    /// Creates a new shutdown-aware writer with buffered stdout.
    fn new() -> Self {
        Self { buffer: std::io::BufWriter::new(std::io::stdout()) }
    }
}

impl Write for ShutdownAwareWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if SHUTDOWN_IN_PROGRESS.load(Ordering::Relaxed) {
            // During shutdown, write directly to stdout
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write(buf)
        } else {
            self.buffer.write(buf)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if SHUTDOWN_IN_PROGRESS.load(Ordering::Relaxed) {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.flush()
        } else {
            self.buffer.flush()
        }
    }
}

/// Factory for creating shutdown-aware writers.
///
/// Implements the MakeWriter trait to provide writers that can adapt
/// their behavior during application shutdown.
struct ShutdownAwareWriterMaker;

impl<'a> MakeWriter<'a> for ShutdownAwareWriterMaker {
    type Writer = ShutdownAwareWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ShutdownAwareWriter::new()
    }
}

/// Custom timer formatter for log messages.
///
/// Provides different time formats for normal operation and shutdown:
/// - Normal: "DD Month - HH:MM:SS.microseconds"
/// - Shutdown: "HH:MM:SS"
struct CustomTimer;

impl tracing_subscriber::fmt::time::FormatTime for CustomTimer {
    fn format_time(&self, writer: &mut Writer<'_>) -> std::fmt::Result {
        // Use a simpler time format during shutdown
        if SHUTDOWN_IN_PROGRESS.load(Ordering::Relaxed) {
            let now = chrono::Local::now();
            write!(writer, "{}", now.format("%H:%M:%S"))
        } else {
            let now = chrono::Local::now();
            write!(writer, "{} - {}", now.format("%d %B"), now.format("%H:%M:%S%.6f"))
        }
    }
}

/// Sets up the global logger with the specified log level.
///
/// Configures tracing with:
/// - Custom timestamp formatting
/// - Shutdown-aware writing
/// - Environment variable override support
/// - Level and message display (no target)
///
/// # Arguments
/// * `log_level` - The minimum log level to display
///
/// # Note
/// If a global logger is already set, this function silently does nothing.
pub fn setup_logger(log_level: LevelFilter) {
    let filter = EnvFilter::from_default_env().add_directive(log_level.into());

    let format = Format::default().with_timer(CustomTimer).with_level(true).with_target(false);

    let subscriber = tracing_subscriber::fmt()
        .with_writer(ShutdownAwareWriterMaker)
        .with_env_filter(filter)
        .event_format(format)
        .finish();

    if tracing::subscriber::set_global_default(subscriber).is_err() {
        // Use println! here since logging might not be set up yet
        // println!("Logger has already been set up, continuing...");
    }
}

/// Sets up the global logger with INFO level.
///
/// Convenience function that configures logging at INFO level,
/// which is the standard level for production deployments.
///
/// Equivalent to calling `setup_logger(LevelFilter::INFO)`.
pub fn setup_info_logger() {
    setup_logger(LevelFilter::INFO);
}

/// Marks that application shutdown has started.
///
/// This changes the logging behavior to use direct stdout writes
/// instead of buffered writes, ensuring that shutdown messages
/// are immediately visible.
///
/// # Usage
/// Call this function when beginning application shutdown procedures.
pub fn mark_shutdown_started() {
    SHUTDOWN_IN_PROGRESS.store(true, Ordering::Relaxed);
}

/// RAII guard for temporarily suppressing shutdown mode.
///
/// When this guard is dropped, it resets the shutdown flag to false,
/// allowing normal buffered logging to resume. This is useful for
/// testing or temporary operations during shutdown.
pub struct LoggerGuard;

impl Drop for LoggerGuard {
    /// Resets the shutdown flag when the guard is dropped.
    fn drop(&mut self) {
        SHUTDOWN_IN_PROGRESS.store(false, Ordering::Relaxed);
    }
}
