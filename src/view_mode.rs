use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum ViewMode {
    FitToWindow,
    BestFit,
    FitWidth,
    FitHeight,
    Absolute,
    Zoom(f64),
}
