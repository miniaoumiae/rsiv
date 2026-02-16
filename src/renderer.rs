use crate::cache::CacheManager;
use crate::image_item::{ImageSlot, LoadedImage};
use rayon::prelude::*;

pub struct GridColors {
    pub bg: (u8, u8, u8),
    pub accent: (u8, u8, u8),
    pub mark: (u8, u8, u8),
    pub loading: (u8, u8, u8),
    pub error: (u8, u8, u8),
}

pub struct DrawImageParams<'a> {
    pub image: &'a LoadedImage,
    pub frame_idx: usize,
    pub scale: f64,
    pub off_x: i32,
    pub off_y: i32,
    pub show_alpha: bool,
}

#[derive(Clone, Copy)]
struct Rect(i32, i32, i32, i32);

pub fn clear(frame: &mut [u8], color: (u8, u8, u8)) {
    frame.par_chunks_exact_mut(4).for_each(|chunk| {
        chunk[0] = color.0;
        chunk[1] = color.1;
        chunk[2] = color.2;
        chunk[3] = 255;
    });
}

pub fn draw_image(frame: &mut [u8], buf_w: i32, buf_h: i32, params: &DrawImageParams) {
    let image = params.image;
    let frame_idx = params.frame_idx;
    let scale = params.scale;
    let off_x = params.off_x;
    let off_y = params.off_y;
    let show_alpha = params.show_alpha;

    let img_w = image.width as f64;
    let img_h = image.height as f64;

    let scaled_w = img_w * scale;
    let scaled_h = img_h * scale;

    let tl_x = (buf_w as f64 / 2.0) - (scaled_w / 2.0) + off_x as f64;
    let tl_y = (buf_h as f64 / 2.0) - (scaled_h / 2.0) + off_y as f64;

    let start_x = tl_x.max(0.0) as i32;
    let start_y = tl_y.max(0.0) as i32;
    let end_x = (tl_x + scaled_w).min(buf_w as f64) as i32;
    let end_y = (tl_y + scaled_h).min(buf_h as f64) as i32;

    if end_x <= start_x || end_y <= start_y {
        return;
    }

    let inv_scale = 1.0 / scale;
    let src_width = image.width as i32;
    let src_height = image.height as i32;

    // Safety check for empty frames
    if image.frames.is_empty() {
        return;
    }

    // Safety check for frame index
    let safe_frame_idx = frame_idx % image.frames.len();
    let current_pixels = &image.frames[safe_frame_idx].pixels;

    let global_src_x_start_f = (start_x as f64 - tl_x) * inv_scale;

    // Ckeckboard colors
    let check_size = 16;
    let check_color_1 = 204u8; // Light gray (0xCC)
    let check_color_2 = 153u8; // Darker gray (0x99)

    frame
        .par_chunks_exact_mut((buf_w * 4) as usize)
        .enumerate()
        .for_each(|(y, row_pixels)| {
            let y = y as i32;

            if y < start_y || y >= end_y {
                return;
            }

            let src_y = ((y as f64 - tl_y) * inv_scale) as i32;

            if src_y >= 0 && src_y < src_height {
                let src_row_start = (src_y * src_width) as usize * 4;
                let mut src_x_f = global_src_x_start_f;

                let draw_slice_start = (start_x as usize) * 4;
                let draw_slice_end = (end_x as usize) * 4;

                if draw_slice_end > row_pixels.len() {
                    return;
                }

                let dest_slice = &mut row_pixels[draw_slice_start..draw_slice_end];

                for (i, dest_pixel) in dest_slice.chunks_exact_mut(4).enumerate() {
                    let current_screen_x = start_x + i as i32; // Absolute X coordinate for checkerboard
                    let src_x = src_x_f as i32;

                    if src_x >= 0 && src_x < src_width {
                        let src_idx = src_row_start + (src_x as usize * 4);
                        if src_idx + 4 <= current_pixels.len() {
                            let src_p = &current_pixels[src_idx..src_idx + 4];
                            let src_a = src_p[3] as u32;

                            if src_a == 255 {
                                // Opaque
                                dest_pixel.copy_from_slice(src_p);
                            } else if src_a > 0 {
                                // Transparent

                                // Determine background color (Checkerboard or Window BG)
                                let (bg_r, bg_g, bg_b) = if show_alpha {
                                    // Calculate checkerboard based on screen coordinates
                                    let is_dark =
                                        ((current_screen_x / check_size) + (y / check_size)) % 2
                                            == 0;
                                    let c = if is_dark {
                                        check_color_2
                                    } else {
                                        check_color_1
                                    };
                                    (c as u32, c as u32, c as u32)
                                } else {
                                    // Use existing background color
                                    (
                                        dest_pixel[0] as u32,
                                        dest_pixel[1] as u32,
                                        dest_pixel[2] as u32,
                                    )
                                };

                                let inv_a = 255 - src_a;

                                // Blend
                                dest_pixel[0] =
                                    ((src_p[0] as u32 * src_a + bg_r * inv_a) / 255) as u8;
                                dest_pixel[1] =
                                    ((src_p[1] as u32 * src_a + bg_g * inv_a) / 255) as u8;
                                dest_pixel[2] =
                                    ((src_p[2] as u32 * src_a + bg_b * inv_a) / 255) as u8;
                                dest_pixel[3] = 255;
                            }
                            // If src_a == 0, we do nothing (leave existing background),
                            // UNLESS we want to force draw the checkerboard over the cleared bg
                            else if show_alpha {
                                let is_dark =
                                    ((current_screen_x / check_size) + (y / check_size)) % 2 == 0;
                                let c = if is_dark {
                                    check_color_2
                                } else {
                                    check_color_1
                                };
                                dest_pixel[0] = c;
                                dest_pixel[1] = c;
                                dest_pixel[2] = c;
                                dest_pixel[3] = 255;
                            }
                        }
                    }
                    src_x_f += inv_scale;
                }
            }
        });
}

pub fn draw_grid(
    frame: &mut [u8],
    buf_w: i32,
    buf_h: i32,
    images: &[ImageSlot],
    cache: &CacheManager,
    selected_idx: usize,
    colors: &GridColors,
    marked_paths: &std::collections::HashSet<String>,
) {
    let config = crate::config::AppConfig::get();
    let thumb_size = config.options.thumbnail_size;
    let padding = config.options.grid_padding;
    let cell_size = thumb_size + padding;

    let cols = (buf_w as u32 / cell_size).max(1);

    let grid_width = cols * cell_size;
    let margin_x = (buf_w as u32 - grid_width) / 2 + padding / 2;

    let current_row = (selected_idx as u32) / cols;
    let scroll_y = if current_row * cell_size > buf_h as u32 / 2 {
        (current_row * cell_size) as i32 - (buf_h / 2) + (cell_size as i32 / 2)
    } else {
        0
    };

    clear(frame, colors.bg);

    // GATHER: Collect all draw commands sequentially.
    let draw_commands: Vec<_> = images
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| {
            let col = (i as u32) % cols;
            let row = (i as u32) / cols;

            let x_cell = (margin_x + col * cell_size) as i32;
            let y_cell = (row * cell_size + padding / 2) as i32 - scroll_y;

            if y_cell + (cell_size as i32) < 0 || y_cell > buf_h {
                return None;
            }

            let is_selected = i == selected_idx;

            // Check cache (mut access here is safe because we are single-threaded in this phase)
            if let ImageSlot::MetadataLoaded(item) = slot {
                let is_marked = marked_paths.contains(&item.path.to_string_lossy().to_string());
                let thumb_data = cache.get_thumbnail(&item.path);

                // Calculate correct aspect ratio for the placeholder box even if not loaded
                let (p_w, p_h) = {
                    let aspect = item.width as f64 / item.height as f64;
                    if aspect >= 1.0 {
                        (thumb_size, (thumb_size as f64 / aspect) as u32)
                    } else {
                        ((thumb_size as f64 * aspect) as u32, thumb_size)
                    }
                };
                let p_w = p_w as i32;
                let p_h = p_h as i32;

                let t_x = x_cell + (thumb_size as i32 - p_w) / 2;
                let t_y = y_cell + (thumb_size as i32 - p_h) / 2;

                let y_min = t_y - 10;
                let y_max = t_y + p_h + 10;

                Some((
                    y_min,
                    y_max,
                    x_cell,
                    y_cell,
                    t_x,
                    t_y,
                    thumb_data,
                    is_selected,
                    is_marked,
                    ImageSlot::MetadataLoaded(item.clone()),
                ))
            } else {
                // For pending/error slots
                let p_size = thumb_size as i32;
                let t_x = x_cell + (thumb_size as i32 - p_size) / 2;
                let t_y = y_cell + (thumb_size as i32 - p_size) / 2;
                let y_min = t_y - 10;
                let y_max = t_y + p_size + 10;
                Some((
                    y_min,
                    y_max,
                    x_cell,
                    y_cell,
                    t_x,
                    t_y,
                    None,
                    is_selected,
                    false,
                    slot.clone(),
                ))
            }
        })
        .collect();

    // DRAW: Execute commands in parallel
    let thumb_size_i32 = thumb_size as i32;

    frame
        .par_chunks_exact_mut((buf_w * 4) as usize)
        .enumerate()
        .for_each(|(y, row_pixels)| {
            let y = y as i32;

            for (
                _ymin,
                _ymax,
                x_cell,
                y_cell,
                base_t_x,
                base_t_y,
                thumb_data,
                is_selected,
                is_marked,
                slot,
            ) in draw_commands
                .iter()
                .filter(|(ymin, ymax, ..)| y >= *ymin && y < *ymax)
            {
                // Draw Thumbnail Pixels
                if let Some(data) = thumb_data {
                    let (t_w, t_h, pixels) = &**data;
                    let t_w = *t_w as i32;
                    let t_h = *t_h as i32;

                    let t_x = x_cell + (thumb_size_i32 - t_w) / 2;
                    let t_y = y_cell + (thumb_size_i32 - t_h) / 2;

                    if y >= t_y && y < t_y + t_h {
                        let row_idx = y - t_y;
                        let src_row_start = (row_idx * t_w) as usize * 4;
                        let dest_x_start = t_x.max(0);
                        let dest_x_end = (t_x + t_w).min(buf_w);

                        if dest_x_end > dest_x_start {
                            let src_offset_x = (dest_x_start - t_x) as usize * 4;
                            let copy_len = (dest_x_end - dest_x_start) as usize * 4;
                            let dest_row_start = (dest_x_start as usize) * 4;

                            if src_row_start + src_offset_x + copy_len <= pixels.len()
                                && dest_row_start + copy_len <= row_pixels.len()
                            {
                                let src_slice = &pixels[src_row_start + src_offset_x
                                    ..src_row_start + src_offset_x + copy_len];
                                let dest_slice =
                                    &mut row_pixels[dest_row_start..dest_row_start + copy_len];

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
                } else {
                    // Draw placeholder
                    let color = match slot {
                        ImageSlot::Error(_) => colors.error,
                        _ => colors.loading,
                    };

                    // We use the coordinates calculated in gather step (base_t_x/y which are already centered)
                    // If it's MetadataLoaded, it's aspect-correct. If Pending, it's square.
                    let (p_w, p_h) = if let ImageSlot::MetadataLoaded(m) = slot {
                        let aspect = m.width as f64 / m.height as f64;
                        if aspect >= 1.0 {
                            (thumb_size_i32, (thumb_size as f64 / aspect) as i32)
                        } else {
                            ((thumb_size as f64 * aspect) as i32, thumb_size_i32)
                        }
                    } else {
                        (thumb_size_i32, thumb_size_i32)
                    };

                    draw_border_scanline(
                        row_pixels,
                        y,
                        buf_w,
                        Rect(*base_t_x, *base_t_y, p_w, p_h),
                        color,
                    );
                }

                // Draw Selection Border
                if *is_selected {
                    let border_gap = 1;
                    let thickness = 4;
                    let offset = border_gap + thickness;

                    let (target_w, target_h, target_x, target_y) = if let Some(data) = thumb_data {
                        let (t_w, t_h, _) = &**data;
                        let t_x = x_cell + (thumb_size_i32 - *t_w as i32) / 2;
                        let t_y = y_cell + (thumb_size_i32 - *t_h as i32) / 2;
                        (*t_w as i32, *t_h as i32, t_x, t_y)
                    } else if let ImageSlot::MetadataLoaded(m) = slot {
                        let aspect = m.width as f64 / m.height as f64;
                        let (p_w, p_h) = if aspect >= 1.0 {
                            (thumb_size_i32, (thumb_size as f64 / aspect) as i32)
                        } else {
                            ((thumb_size as f64 * aspect) as i32, thumb_size_i32)
                        };
                        (p_w, p_h, *base_t_x, *base_t_y)
                    } else {
                        (thumb_size_i32, thumb_size_i32, *base_t_x, *base_t_y)
                    };

                    draw_border_scanline(
                        row_pixels,
                        y,
                        buf_w,
                        Rect(
                            target_x - offset,
                            target_y - offset,
                            target_w + offset * 2,
                            target_h + offset * 2,
                        ),
                        colors.accent,
                    );
                }

                // Draw Mark
                if *is_marked {
                    let (target_w, target_h, target_x, target_y) = if let Some(data) = thumb_data {
                        let (t_w, t_h, _) = &**data;
                        let t_x = x_cell + (thumb_size_i32 - *t_w as i32) / 2;
                        let t_y = y_cell + (thumb_size_i32 - *t_h as i32) / 2;
                        (*t_w as i32, *t_h as i32, t_x, t_y)
                    } else {
                        // Use base coordinates from gather step
                        let (p_w, p_h) = if let ImageSlot::MetadataLoaded(m) = slot {
                            let aspect = m.width as f64 / m.height as f64;
                            if aspect >= 1.0 {
                                (thumb_size_i32, (thumb_size as f64 / aspect) as i32)
                            } else {
                                ((thumb_size as f64 * aspect) as i32, thumb_size_i32)
                            }
                        } else {
                            (thumb_size_i32, thumb_size_i32)
                        };
                        (p_w, p_h, *base_t_x, *base_t_y)
                    };

                    let mark_size = 12;
                    let border_gap = 1;
                    let thickness = 4;
                    let m_x = target_x + target_w + border_gap + thickness / 2 - mark_size / 2;
                    let m_y = target_y + target_h + border_gap + thickness / 2 - mark_size / 2;

                    if y >= m_y && y < m_y + mark_size {
                        let start_draw_x = m_x.max(0);
                        let end_draw_x = (m_x + mark_size).min(buf_w);

                        for x in start_draw_x..end_draw_x {
                            let idx = (x as usize) * 4;
                            if idx + 4 <= row_pixels.len() {
                                row_pixels[idx] = colors.mark.0;
                                row_pixels[idx + 1] = colors.mark.1;
                                row_pixels[idx + 2] = colors.mark.2;
                                row_pixels[idx + 3] = 255;
                            }
                        }
                    }
                }
            }
        });
}

fn draw_border_scanline(
    row_pixels: &mut [u8],
    y: i32,
    buf_w: i32,
    rect: Rect,
    color: (u8, u8, u8),
) {
    let Rect(rx, ry, rw, rh) = rect;
    let thickness = 4;

    let in_vertical_range = y >= ry && y < ry + rh;
    if !in_vertical_range {
        return;
    }

    let in_top = y >= ry && y < ry + thickness;
    let in_bottom = y >= ry + rh - thickness && y < ry + rh;

    let color_alpha = [color.0, color.1, color.2, 255];

    let draw_span = |start_x: i32, end_x: i32, pixels: &mut [u8]| {
        let sx = start_x.max(0);
        let ex = end_x.min(buf_w);
        if ex > sx {
            for x in sx..ex {
                let idx = (x as usize) * 4;
                if idx + 4 <= pixels.len() {
                    pixels[idx..idx + 4].copy_from_slice(&color_alpha);
                }
            }
        }
    };

    if in_top || in_bottom {
        draw_span(rx, rx + rw, row_pixels);
    } else {
        draw_span(rx, rx + thickness, row_pixels);
        draw_span(rx + rw - thickness, rx + rw, row_pixels);
    }
}
