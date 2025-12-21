use std::sync::OnceLock;

static VERBOSITY: OnceLock<u8> = OnceLock::new();

pub fn init(level: u8) {
    VERBOSITY.set(level).ok();
}

pub fn verbosity() -> u8 {
    *VERBOSITY.get().unwrap_or(&0)
}

#[macro_export]
macro_rules! vlog {
    ($level:expr, $($arg:tt)*) => {
        if $crate::log::verbosity() >= $level {
            eprintln!("[v{}] {}", $level, format!($($arg)*));
        }
    };
}

pub fn redact_sensitive(s: &str, show_prefix_len: usize) -> String {
    if s.len() <= show_prefix_len {
        "*".repeat(s.len())
    } else {
        format!("{}***", &s[..show_prefix_len])
    }
}
