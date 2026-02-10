use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum ViewMode {
    FitToWindow,
    BestFit,
    Cover,
    FitWidth,
    FitHeight,
    Absolute,
    Zoom(f64),
}
