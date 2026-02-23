use crate::view_mode::ViewMode;

pub struct Camera {
    pub mode: ViewMode,
    pub off_x: i32,
    pub off_y: i32,
    pub grid_mode: bool,
    pub show_alpha: bool,
}

impl Camera {
    pub fn get_current_scale(&self, window_size: (f64, f64), image_size: (f64, f64)) -> f64 {
        let (buf_w, buf_h) = window_size;
        let (img_w, img_h) = image_size;

        if buf_w <= 0.0 || buf_h <= 0.0 || img_w <= 0.0 || img_h <= 0.0 {
            return 1.0;
        }

        match self.mode {
            ViewMode::Absolute => 1.0,
            ViewMode::Zoom(s) => {
                let config = crate::config::AppConfig::get();
                s.clamp(config.options.zoom_min, config.options.zoom_max)
            }
            ViewMode::FitToWindow => (buf_w / img_w).min(buf_h / img_h),
            ViewMode::BestFit => (buf_w / img_w).min(buf_h / img_h).min(1.0),
            ViewMode::Cover => (buf_w / img_w).max(buf_h / img_h),
            ViewMode::FitWidth => buf_w / img_w,
            ViewMode::FitHeight => buf_h / img_h,
        }
    }

    pub fn clamp_offsets(&mut self, window_size: (f64, f64), scaled_img_size: (f64, f64)) {
        if self.grid_mode {
            return;
        }

        let (buf_w, buf_h) = window_size;
        let (scaled_w, scaled_h) = scaled_img_size;

        if buf_w <= 0.0 || buf_h <= 0.0 {
            return;
        }

        let config = crate::config::AppConfig::get();

        if config.options.clamp_pan {
            let max_off_x = ((scaled_w - buf_w) / 2.0).max(0.0).floor() as i32;
            let max_off_y = ((scaled_h - buf_h) / 2.0).max(0.0).floor() as i32;
            self.off_x = self.off_x.clamp(-max_off_x, max_off_x);
            self.off_y = self.off_y.clamp(-max_off_y, max_off_y);
        } else {
            let keep_x = 50.0_f64.min(scaled_w);
            let keep_y = 50.0_f64.min(scaled_h);
            let max_off_x = ((buf_w + scaled_w) / 2.0 - keep_x).max(0.0).floor() as i32;
            let max_off_y = ((buf_h + scaled_h) / 2.0 - keep_y).max(0.0).floor() as i32;
            self.off_x = self.off_x.clamp(-max_off_x, max_off_x);
            self.off_y = self.off_y.clamp(-max_off_y, max_off_y);
        }
    }
}
