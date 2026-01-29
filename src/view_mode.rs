#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    FitToWindow,
    BestFit,
    FitWidth,
    FitHeight,
    Absolute,
    Zoom(f64),
}
