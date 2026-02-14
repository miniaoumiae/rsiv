use std::sync::OnceLock;

pub static SVG_FONT_DB: OnceLock<resvg::usvg::fontdb::Database> = OnceLock::new();

pub fn get_svg_font_db() -> &'static resvg::usvg::fontdb::Database {
    SVG_FONT_DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

pub fn parse_color(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r, g, b)
    } else {
        (0, 0, 0)
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

pub static QUIET_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_quiet_mode(quiet: bool) {
    QUIET_MODE.store(quiet, Ordering::Relaxed);
}

#[macro_export]
macro_rules! rsiv_err {
    ($($arg:tt)*) => {{
        if !$crate::utils::QUIET_MODE.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("\x1b[1;31m[error]\x1b[0m rsiv: {}", format_args!($($arg)*));
        }
    }};
}

#[macro_export]
macro_rules! rsiv_warn {
    ($($arg:tt)*) => {{
        if !$crate::utils::QUIET_MODE.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("\x1b[1;33m[warning]\x1b[0m rsiv: {}", format_args!($($arg)*));
        }
    }};
}
