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
