use crate::image_item::{ImageItem, ImageSlot};

pub struct GridColors {
    pub bg: (u8, u8, u8),
    pub accent: (u8, u8, u8),
    pub mark: (u8, u8, u8),
    pub loading: (u8, u8, u8),
    pub error: (u8, u8, u8),
}

pub struct DrawImageParams<'a> {
    pub item: &'a ImageItem,
    pub frame_idx: usize,
    pub scale: f64,
    pub off_x: i32,
    pub off_y: i32,
}

struct Rect(i32, i32, i32, i32);

pub struct Renderer;

impl Renderer {
    pub fn clear(frame: &mut [u8], color: (u8, u8, u8)) {
        for chunk in frame.chunks_exact_mut(4) {
            chunk[0] = color.0;
            chunk[1] = color.1;
            chunk[2] = color.2;
            chunk[3] = 255;
        }
    }

    pub fn draw_image(
        frame: &mut [u8],
        buf_w: i32,
        available_h: i32,
        params: &DrawImageParams,
    ) {
        let item = params.item;
        let frame_idx = params.frame_idx;
        let scale = params.scale;
        let off_x = params.off_x;
        let off_y = params.off_y;

        let img_w = item.width as f64;
        let img_h = item.height as f64;

        let scaled_w = img_w * scale;
        let scaled_h = img_h * scale;

        let tl_x = (buf_w as f64 / 2.0) - (scaled_w / 2.0) + off_x as f64;
        let tl_y = (available_h as f64 / 2.0) - (scaled_h / 2.0) + off_y as f64;

        let start_x = tl_x.max(0.0) as i32;
        let start_y = tl_y.max(0.0) as i32;
        let end_x = (tl_x + scaled_w).min(buf_w as f64) as i32;
        let end_y = (tl_y + scaled_h).min(available_h as f64) as i32;

        if end_x > start_x && end_y > start_y {
            let inv_scale = 1.0 / scale;
            let src_width = item.width as i32;
            let src_height = item.height as i32;

            let current_pixels = &item.frames[frame_idx].pixels;
            let src_x_start_f = (start_x as f64 - tl_x) * inv_scale;

            for y in start_y..end_y {
                let src_y = ((y as f64 - tl_y) * inv_scale) as i32;

                if src_y >= 0 && src_y < src_height {
                    let dest_row_start = (y * buf_w + start_x) as usize * 4;
                    let src_row_start = (src_y * src_width) as usize * 4;

                    let mut src_x_f = src_x_start_f;
                    let mut dest_idx = dest_row_start;

                    for _x in start_x..end_x {
                        let src_x = src_x_f as i32;

                        if src_x >= 0 && src_x < src_width {
                            let src_idx = src_row_start + (src_x as usize * 4);
                            if src_idx + 4 <= current_pixels.len() && dest_idx + 4 <= frame.len() {
                                let src_pixel = &current_pixels[src_idx..src_idx + 4];
                                let src_a = src_pixel[3] as u32;

                                if src_a == 255 {
                                    frame[dest_idx..dest_idx + 4].copy_from_slice(src_pixel);
                                } else if src_a > 0 {
                                    let dst_pixel = &mut frame[dest_idx..dest_idx + 4];
                                    let inv_a = 255 - src_a;

                                    dst_pixel[0] = ((src_pixel[0] as u32 * src_a
                                        + dst_pixel[0] as u32 * inv_a)
                                        / 255)
                                        as u8;
                                    dst_pixel[1] = ((src_pixel[1] as u32 * src_a
                                        + dst_pixel[1] as u32 * inv_a)
                                        / 255)
                                        as u8;
                                    dst_pixel[2] = ((src_pixel[2] as u32 * src_a
                                        + dst_pixel[2] as u32 * inv_a)
                                        / 255)
                                        as u8;
                                    dst_pixel[3] = 255;
                                }
                            }
                        }
                        src_x_f += inv_scale;
                        dest_idx += 4;
                    }
                }
            }
        }
    }

    pub fn draw_grid(
        frame: &mut [u8],
        buf_w: i32,
        buf_h: i32,
        images: &mut [ImageSlot],
        selected_idx: usize,
        colors: &GridColors,
        marked_paths: &std::collections::HashSet<String>,
    ) {
        let thumb_size = 160;
        let padding = 30;
        let cell_size = thumb_size + padding;

        let cols = (buf_w as u32 / cell_size).max(1);

        // Calculate margin to center the grid horizontally
        let grid_width = cols * cell_size;
        let margin_x = (buf_w as u32 - grid_width) / 2 + padding / 2;

        let current_row = (selected_idx as u32) / cols;
        // Scroll so selected row is roughly in middle
        let scroll_y = if current_row * cell_size > buf_h as u32 / 2 {
            (current_row * cell_size) as i32 - (buf_h / 2) + (cell_size as i32 / 2)
        } else {
            0
        };

        // Clear background
        Self::clear(frame, colors.bg);

        for (i, slot) in images.iter_mut().enumerate() {
            let col = (i as u32) % cols;
            let row = (i as u32) / cols;

            let x_cell = (margin_x + col * cell_size) as i32;
            let y_cell = (row * cell_size + padding / 2) as i32 - scroll_y;

            // Visibility check
            if y_cell + (cell_size as i32) < 0 || y_cell > buf_h {
                continue;
            }

            let ImageSlot::Loaded(item) = slot else {
                let p_size = thumb_size as i32;
                let t_x = x_cell + (thumb_size as i32 - p_size) / 2;
                let t_y = y_cell + (thumb_size as i32 - p_size) / 2;

                let color = match slot {
                    ImageSlot::Error(_) => colors.error,
                    _ => colors.loading,
                };

                Self::draw_border(
                    frame,
                    buf_w,
                    buf_h,
                    Rect(t_x, t_y, p_size, p_size),
                    color,
                );

                if i == selected_idx {
                    let border_gap = 1;
                    let thickness = 4;
                    let offset = border_gap + thickness;
                    Self::draw_border(
                        frame,
                        buf_w,
                        buf_h,
                        Rect(
                            t_x - offset,
                            t_y - offset,
                            p_size + offset * 2,
                            p_size + offset * 2,
                        ),
                        colors.accent,
                    );
                }
                continue;
            };

            if let Some((t_w, t_h, pixels)) = item.get_thumbnail(thumb_size) {
                // Center the thumbnail in the cell
                let t_x = x_cell + (thumb_size as i32 - t_w as i32) / 2;
                let t_y = y_cell + (thumb_size as i32 - t_h as i32) / 2;

                // Draw pixels
                for row_idx in 0..t_h {
                    let dest_y = t_y + row_idx as i32;
                    if dest_y >= 0 && dest_y < buf_h {
                        let src_row_start = (row_idx * t_w) as usize * 4;
                        let dest_row_start = (dest_y * buf_w + t_x) as usize * 4;

                        let row_len = (t_w as usize).min((buf_w - t_x).max(0) as usize) * 4;

                        if src_row_start + row_len <= pixels.len()
                            && dest_row_start + row_len <= frame.len()
                            && t_x >= 0
                        {
                            let src_slice = &pixels[src_row_start..src_row_start + row_len];
                            let dest_slice = &mut frame[dest_row_start..dest_row_start + row_len];

                            for (src_chunk, dest_chunk) in src_slice
                                .chunks_exact(4)
                                .zip(dest_slice.chunks_exact_mut(4))
                            {
                                let src_a = src_chunk[3] as u32;
                                if src_a == 255 {
                                    dest_chunk.copy_from_slice(src_chunk);
                                } else if src_a > 0 {
                                    let inv_a = 255 - src_a;
                                    dest_chunk[0] = ((src_chunk[0] as u32 * src_a
                                        + dest_chunk[0] as u32 * inv_a)
                                        / 255)
                                        as u8;
                                    dest_chunk[1] = ((src_chunk[1] as u32 * src_a
                                        + dest_chunk[1] as u32 * inv_a)
                                        / 255)
                                        as u8;
                                    dest_chunk[2] = ((src_chunk[2] as u32 * src_a
                                        + dest_chunk[2] as u32 * inv_a)
                                        / 255)
                                        as u8;
                                    dest_chunk[3] = 255;
                                }
                            }
                        }
                    }
                }

                // Draw border if selected
                if i == selected_idx {
                    let border_gap = 1;
                    let thickness = 4;
                    let offset = border_gap + thickness;
                    Self::draw_border(
                        frame,
                        buf_w,
                        buf_h,
                        Rect(
                            t_x - offset,
                            t_y - offset,
                            t_w as i32 + offset * 2,
                            t_h as i32 + offset * 2,
                        ),
                        colors.accent,
                    );
                }

                // Draw mark indicator if marked
                if marked_paths.contains(&item.path) {
                    let mark_size = 12;
                    let border_gap = 1;
                    let thickness = 4;
                    // Position at bottom-right corner, centered on the border area
                    let m_x = t_x + t_w as i32 + border_gap + thickness / 2 - mark_size / 2;
                    let m_y = t_y + t_h as i32 + border_gap + thickness / 2 - mark_size / 2;

                    // Simple filled rect for mark
                    for dy in 0..mark_size {
                        for dx in 0..mark_size {
                            let px = m_x + dx;
                            let py = m_y + dy;
                            if px >= 0 && px < buf_w && py >= 0 && py < buf_h {
                                let idx = ((py * buf_w + px) * 4) as usize;
                                if idx + 4 <= frame.len() {
                                    frame[idx] = colors.mark.0;
                                    frame[idx + 1] = colors.mark.1;
                                    frame[idx + 2] = colors.mark.2;
                                    frame[idx + 3] = 255;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn draw_border(
        frame: &mut [u8],
        buf_w: i32,
        buf_h: i32,
        rect: Rect,
        color: (u8, u8, u8),
    ) {
        let Rect(x, y, w, h) = rect;
        let thickness = 4;
        let color_alpha = [color.0, color.1, color.2, 255];

        let mut set_pixel = |x: i32, y: i32| {
            if x >= 0 && x < buf_w && y >= 0 && y < buf_h {
                let idx = ((y * buf_w + x) * 4) as usize;
                if idx + 4 <= frame.len() {
                    frame[idx..idx + 4].copy_from_slice(&color_alpha);
                }
            }
        };

        for i in 0..thickness {
            // Top & Bottom
            for bx in x..(x + w) {
                set_pixel(bx, y + i);
                set_pixel(bx, y + h - 1 - i);
            }
            // Left & Right
            for by in y..(y + h) {
                set_pixel(x + i, by);
                set_pixel(x + w - 1 - i, by);
            }
        }
    }
}

