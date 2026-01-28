#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    FitToWindow, // 'f'
    BestFit,     // 'F' (Fit to window, but don't upscale)
    FitWidth,    // 'W'
    FitHeight,   // 'H'
    Absolute,    // '='
    Zoom(f64),
}
