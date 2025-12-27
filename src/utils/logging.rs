use colored::Colorize;
use log::{Level, LevelFilter};
use std::io::Write;

/// Initialize the logging system
pub fn init_logging(log_level: Option<&str>) {
    let mut builder = env_logger::Builder::new();

    // Set log level with precedence: CLI flag > environment variable > default
    let level = if let Some(level_str) = log_level {
        match level_str.parse::<LevelFilter>() {
            Ok(level) => level,
            Err(_) => LevelFilter::Warn,
        }
    } else {
        match std::env::var("DOTFILES_LOG")
            .ok()
            .and_then(|log_var| log_var.parse::<LevelFilter>().ok())
        {
            Some(level) => level,
            None => LevelFilter::Warn,
        }
    };

    builder.filter_level(level);

    // Custom format: [LEVEL] message
    builder.format(|buf, record| {
        let level_string = match record.level() {
            Level::Error => record.level().to_string().red().bold().to_string(),
            Level::Warn => record.level().to_string().yellow().bold().to_string(),
            Level::Info => record.level().to_string().cyan().bold().to_string(),
            Level::Debug => record.level().to_string().blue().bold().to_string(),
            Level::Trace => record.level().to_string().normal().to_string(),
        };
        writeln!(buf, "[{}] {}", level_string, record.args())
    });

    builder.init();
}

/// Log operation with context
#[macro_export]
macro_rules! log_op {
    ($msg:expr) => {
        log::info!($msg);
    };
    ($fmt:expr, $($arg:tt)*) => {
        log::info!($fmt, $($arg)*);
    };
}

/// Log a file operation
#[macro_export]
macro_rules! log_file_op {
    ($op:expr, $file:expr) => {
        log::debug!("{}: {}", $op, $file.display());
    };
}

/// Log a sync operation
#[macro_export]
macro_rules! log_sync {
    ($from:expr, $to:expr) => {
        log::debug!("Sync: {} -> {}", $from.display(), $to.display());
    };
}
