pub struct FrameBuffer<'a> {
    pub frame: &'a mut [u8],
    pub width: u32,
    pub height: u32,
}

impl<'a> FrameBuffer<'a> {
    pub fn new(frame: &'a mut [u8], width: u32, height: u32) -> Self {
        Self {
            frame,
            width,
            height,
        }
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: (u8, u8, u8)) {
        let (r, g, b) = color;
        let start_x = x.max(0);
        let start_y = y.max(0);
        let end_x = (x + w as i32).min(self.width as i32);
        let end_y = (y + h as i32).min(self.height as i32);

        if start_x >= end_x || start_y >= end_y {
            return;
        }

        for cy in start_y..end_y {
            let row_start = (cy as u32 * self.width + start_x as u32) as usize * 4;
            let width_bytes = (end_x - start_x) as usize * 4;
            let row_end = row_start + width_bytes;

            if row_end <= self.frame.len() {
                // Optimization: could fill directly, but loop is fine for now
                for chunk in self.frame[row_start..row_end].chunks_exact_mut(4) {
                    chunk[0] = r;
                    chunk[1] = g;
                    chunk[2] = b;
                    chunk[3] = 255;
                }
            }
        }
    }
}
