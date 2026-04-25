use crate::app::InputMode;
use std::sync::Arc;
use std::time::Instant;
use winit::window::{CursorIcon, Window};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CursorZone {
    None,
    Prev,
    Next,
    Dragging,
}

pub struct CursorState {
    pub pos: Option<(f64, f64)>,
    pub zone: CursorZone,
    pub is_dragging: bool,
    pub drag_start_pos: Option<(f64, f64)>,
    pub camera_start_pos: Option<(i32, i32)>,
    pub last_left_click_time: Option<Instant>,
    pub last_left_click_pos: Option<(f64, f64)>,
    pub last_moved: Instant,
    pub is_visible: bool,
}

impl CursorState {
    pub fn is_in_next_zone(
        &self,
        x: f64,
        window_width: f64,
        grid_mode: bool,
        input_mode: &InputMode,
    ) -> bool {
        if grid_mode || *input_mode != InputMode::Normal || window_width <= 0.0 {
            return false;
        }
        x >= window_width * 0.8
    }

    pub fn is_in_prev_zone(
        &self,
        x: f64,
        window_width: f64,
        grid_mode: bool,
        input_mode: &InputMode,
    ) -> bool {
        if grid_mode || *input_mode != InputMode::Normal || window_width <= 0.0 {
            return false;
        }
        x <= window_width * 0.2
    }

    pub fn set_zone(&mut self, zone: CursorZone, window: Option<&Arc<Window>>) {
        if self.zone == zone {
            return;
        }
        self.zone = zone;
        if let Some(w) = window {
            match zone {
                // macOS abstracts the single limit arrows, so we use the Pointer (hand)
                CursorZone::Next => w.set_cursor(CursorIcon::Pointer),
                CursorZone::Prev => w.set_cursor(CursorIcon::Pointer),
                CursorZone::Dragging => w.set_cursor(CursorIcon::Grabbing),
                CursorZone::None => w.set_cursor(CursorIcon::Default),
            };
        }
    }

    pub fn refresh_icon(
        &mut self,
        window_width: f64,
        grid_mode: bool,
        input_mode: &InputMode,
        window: Option<&Arc<Window>>,
    ) {
        if self.is_dragging {
            self.set_zone(CursorZone::Dragging, window);
            return;
        }

        if let Some((x, _)) = self.pos {
            if self.is_in_next_zone(x, window_width, grid_mode, input_mode) {
                self.set_zone(CursorZone::Next, window);
            } else if self.is_in_prev_zone(x, window_width, grid_mode, input_mode) {
                self.set_zone(CursorZone::Prev, window);
            } else {
                self.set_zone(CursorZone::None, window);
            }
        } else {
            self.set_zone(CursorZone::None, window);
        }
    }
}
