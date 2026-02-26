use crate::cache::CacheManager;
use crate::image_item::{ImageSlot, LoadedAsset};
use rayon::prelude::*;
use std::sync::Arc;
use tiny_skia::PixmapMut;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<(u8, u8, u8)> for Rgb {
    #[inline]
    fn from(tuple: (u8, u8, u8)) -> Self {
        Self {
            r: tuple.0,
            g: tuple.1,
            b: tuple.2,
        }
    }
}

pub struct GridColors {
    pub bg: Rgb,
    pub accent: Rgb,
    pub mark: Rgb,
    pub loading: Rgb,
    pub error: Rgb,
}

pub struct DrawImageParams<'a> {
    pub asset: &'a LoadedAsset,
    pub frame_idx: usize,
    pub scale: f64,
    pub off_x: i32,
    pub off_y: i32,
    pub show_alpha: bool,
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    #[inline]
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
}

struct DrawCmd {
    y_min: i32,
    y_max: i32,
    rect: Rect,
    thumb_data: Option<Arc<(u32, u32, Vec<u8>)>>,
    is_selected: bool,
    is_marked: bool,
    placeholder_color: Rgb,
}

struct GridContext<'a> {
    buf_h: i32,
    cols: u32,
    margin_x: u32,
    cell_size: u32,
    padding: u32,
    scroll_y: i32,
    selected_idx: usize,
    colors: &'a GridColors,
    marked_paths: &'a std::collections::HashSet<String>,
    cache: &'a CacheManager,
    thumb_size: u32,
    border_gap: i32,
    border_thickness: i32,
    mark_size: i32,
}

#[inline(always)]
fn blend_swar(src: [u8; 4], dst: [u8; 4], alpha: u32) -> [u8; 4] {
    let inv_a = 255 - alpha;

    // Pack RB (0x00RR00BB)
    let src_rb = (src[0] as u32) << 16 | (src[2] as u32);
    let dst_rb = (dst[0] as u32) << 16 | (dst[2] as u32);

    // Pack GA (0x00GG00AA)
    let src_ga = (src[1] as u32) << 16 | (src[3] as u32);
    let dst_ga = (dst[1] as u32) << 16 | (dst[3] as u32);

    // Blend RB
    let res_rb = src_rb * alpha + dst_rb * inv_a;
    let out_rb = res_rb + 0x00800080; // Rounding bias
    let out_rb = (out_rb + ((out_rb >> 8) & 0x00FF00FF)) >> 8;
    let out_rb = out_rb & 0x00FF00FF;

    // Blend GA
    let res_ga = src_ga * alpha + dst_ga * inv_a;
    let out_ga = res_ga + 0x00800080;
    let out_ga = (out_ga + ((out_ga >> 8) & 0x00FF00FF)) >> 8;
    let out_ga = out_ga & 0x00FF00FF;

    [
        (out_rb >> 16) as u8,
        (out_ga >> 16) as u8,
        out_rb as u8,
        255, // Resulting alpha
    ]
}

pub fn clear(frame: &mut [u8], color: Rgb) {
    let pixel = [color.r, color.g, color.b, 255];
    frame.par_chunks_mut(1024).for_each(|chunk| {
        for p in chunk.chunks_exact_mut(4) {
            p.copy_from_slice(&pixel);
        }
    });
}

#[inline(always)]
fn get_checkerboard_bg_fast(
    x: i32,
    y_check_state: i32,
    check_size: i32,
    color_1: Rgb,
    color_2: Rgb,
) -> Rgb {
    let x_state = x / check_size;
    let is_dark = (x_state ^ y_check_state) & 1;
    if is_dark != 0 { color_2 } else { color_1 }
}

// Optimized bilinear sampling using pre-calculated weights and row pointers
#[inline(always)]
fn sample_bilinear_fast(
    row0: &[u8],
    row1: &[u8],
    width: i32,
    src_x_f: f64,
    fy: u32,
    inv_fy: u32,
) -> (u32, u32, u32, u32) {
    let x = src_x_f.floor() as i32;
    let fx = ((src_x_f - x as f64) * 256.0) as u32;
    let inv_fx = 256 - fx;

    let x0 = x.clamp(0, width - 1) as usize * 4;
    let x1 = (x + 1).clamp(0, width - 1) as usize * 4;

    let r00 = row0[x0] as u32;
    let g00 = row0[x0 + 1] as u32;
    let b00 = row0[x0 + 2] as u32;
    let a00 = row0[x0 + 3] as u32;

    let r10 = row0[x1] as u32;
    let g10 = row0[x1 + 1] as u32;
    let b10 = row0[x1 + 2] as u32;
    let a10 = row0[x1 + 3] as u32;

    let r01 = row1[x0] as u32;
    let g01 = row1[x0 + 1] as u32;
    let b01 = row1[x0 + 2] as u32;
    let a01 = row1[x0 + 3] as u32;

    let r11 = row1[x1] as u32;
    let g11 = row1[x1 + 1] as u32;
    let b11 = row1[x1 + 2] as u32;
    let a11 = row1[x1 + 3] as u32;

    let w00 = inv_fx * inv_fy;
    let w10 = fx * inv_fy;
    let w01 = inv_fx * fy;
    let w11 = fx * fy;

    let r = (r00 * w00 + r10 * w10 + r01 * w01 + r11 * w11) >> 16;
    let g = (g00 * w00 + g10 * w10 + g01 * w01 + g11 * w11) >> 16;
    let b = (b00 * w00 + b10 * w10 + b01 * w01 + b11 * w11) >> 16;
    let a = (a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11) >> 16;

    (r, g, b, a)
}

fn render_raster_scanline(
    row_pixels: &mut [u8],
    y: i32,
    tl_x: f64,
    tl_y: f64,
    inv_scale: f64,
    start_x: i32,
    end_x: i32,
    src_width: i32,
    src_height: i32,
    current_pixels: &[u8],
    show_alpha: bool,
    check_size: i32,
    check_color_1: Rgb,
    check_color_2: Rgb,
) {
    let src_y_f = (y as f64 - tl_y) * inv_scale;

    if src_y_f < 0.0 || src_y_f >= src_height as f64 {
        return;
    }

    let y_int = src_y_f.floor() as i32;
    let fy = ((src_y_f - y_int as f64) * 256.0) as u32;
    let inv_fy = 256 - fy;

    let y0 = y_int.clamp(0, src_height - 1) as usize;
    let y1 = (y_int + 1).clamp(0, src_height - 1) as usize;
    let row0 = &current_pixels[y0 * src_width as usize * 4..];
    let row1 = &current_pixels[y1 * src_width as usize * 4..];

    let mut src_x_f = (start_x as f64 - tl_x) * inv_scale;

    let y_check_state = y / check_size;

    let draw_slice_start = (start_x as usize) * 4;
    let draw_slice_end = (end_x as usize) * 4;

    if draw_slice_end > row_pixels.len() {
        return;
    }

    let dest_slice = &mut row_pixels[draw_slice_start..draw_slice_end];
    assert!(dest_slice.len() % 4 == 0);

    for (i, dest_pixel) in dest_slice.chunks_exact_mut(4).enumerate() {
        let x = start_x + i as i32;

        if src_x_f >= 0.0 && src_x_f < src_width as f64 {
            let (r, g, b, a) = sample_bilinear_fast(row0, row1, src_width, src_x_f, fy, inv_fy);

            let bg = if show_alpha {
                get_checkerboard_bg_fast(x, y_check_state, check_size, check_color_1, check_color_2)
            } else {
                Rgb {
                    r: dest_pixel[0],
                    g: dest_pixel[1],
                    b: dest_pixel[2],
                }
            };

            let src_px = [r as u8, g as u8, b as u8, a as u8];
            let bg_px = [bg.r, bg.g, bg.b, 255];

            if a == 255 {
                dest_pixel.copy_from_slice(&src_px);
            } else if a > 0 {
                let blended = blend_swar(src_px, bg_px, a);
                dest_pixel.copy_from_slice(&blended);
            } else {
                dest_pixel.copy_from_slice(&bg_px);
            }
        }
        src_x_f += inv_scale;
    }
}

fn draw_raster_image(
    frame: &mut [u8],
    buf_w: i32,
    buf_h: i32,
    params: &DrawImageParams,
    width: u32,
    height: u32,
    frames: &[crate::image_item::FrameData],
    check_size: i32,
    check_color_1: Rgb,
    check_color_2: Rgb,
) {
    let scale = params.scale;
    let img_w = width as f64;
    let img_h = height as f64;

    let scaled_w = img_w * scale;
    let scaled_h = img_h * scale;

    let tl_x = (buf_w as f64 / 2.0) - (scaled_w / 2.0) + params.off_x as f64;
    let tl_y = (buf_h as f64 / 2.0) - (scaled_h / 2.0) + params.off_y as f64;

    let start_x = tl_x.max(0.0) as i32;
    let start_y = tl_y.max(0.0) as i32;
    let end_x = (tl_x + scaled_w).min(buf_w as f64) as i32;
    let end_y = (tl_y + scaled_h).min(buf_h as f64) as i32;

    if end_x <= start_x || end_y <= start_y || frames.is_empty() {
        return;
    }

    let inv_scale = 1.0 / scale;
    let src_width = width as i32;
    let src_height = height as i32;

    let safe_frame_idx = params.frame_idx % frames.len();
    let current_pixels = &frames[safe_frame_idx].pixels;

    frame
        .par_chunks_exact_mut((buf_w * 4) as usize)
        .enumerate()
        .for_each(|(y, row_pixels)| {
            let y = y as i32;
            if y >= start_y && y < end_y {
                render_raster_scanline(
                    row_pixels,
                    y,
                    tl_x,
                    tl_y,
                    inv_scale,
                    start_x,
                    end_x,
                    src_width,
                    src_height,
                    current_pixels,
                    params.show_alpha,
                    check_size,
                    check_color_1,
                    check_color_2,
                );
            }
        });
}

fn render_checkerboard_bg_scanline(
    row_pixels: &mut [u8],
    y: i32,
    start_x: i32,
    end_x: i32,
    check_size: i32,
    check_color_1: Rgb,
    check_color_2: Rgb,
) {
    let draw_slice_start = (start_x as usize) * 4;
    let draw_slice_end = (end_x as usize) * 4;
    let dest_slice = &mut row_pixels[draw_slice_start..draw_slice_end];
    assert!(dest_slice.len() % 4 == 0);

    let y_check_state = y / check_size;

    for (i, dest_pixel) in dest_slice.chunks_exact_mut(4).enumerate() {
        let x = start_x + i as i32;
        let bg =
            get_checkerboard_bg_fast(x, y_check_state, check_size, check_color_1, check_color_2);
        dest_pixel[0] = bg.r;
        dest_pixel[1] = bg.g;
        dest_pixel[2] = bg.b;
        dest_pixel[3] = 255;
    }
}

fn draw_vector_image(
    frame: &mut [u8],
    buf_w: i32,
    buf_h: i32,
    params: &DrawImageParams,
    tree: &resvg::usvg::Tree,
    base_width: u32,
    base_height: u32,
    internal_transform: tiny_skia::Transform,
    check_size: i32,
    check_color_1: Rgb,
    check_color_2: Rgb,
) {
    let scaled_w = base_width as f64 * params.scale;
    let scaled_h = base_height as f64 * params.scale;

    let tl_x = (buf_w as f64 / 2.0) - (scaled_w / 2.0) + params.off_x as f64;
    let tl_y = (buf_h as f64 / 2.0) - (scaled_h / 2.0) + params.off_y as f64;

    if params.show_alpha {
        let start_x = tl_x.max(0.0) as i32;
        let start_y = tl_y.max(0.0) as i32;
        let end_x = (tl_x + scaled_w).min(buf_w as f64) as i32;
        let end_y = (tl_y + scaled_h).min(buf_h as f64) as i32;

        if end_x > start_x && end_y > start_y {
            frame
                .par_chunks_exact_mut((buf_w * 4) as usize)
                .enumerate()
                .for_each(|(y, row_pixels)| {
                    let y = y as i32;
                    if y >= start_y && y < end_y {
                        render_checkerboard_bg_scanline(
                            row_pixels,
                            y,
                            start_x,
                            end_x,
                            check_size,
                            check_color_1,
                            check_color_2,
                        );
                    }
                });
        }
    }

    if let Some(mut pixmap) = PixmapMut::from_bytes(frame, buf_w as u32, buf_h as u32) {
        let mut ts = internal_transform;
        ts = ts.post_scale(params.scale as f32, params.scale as f32);
        ts = ts.post_translate(tl_x as f32, tl_y as f32);

        resvg::render(tree, ts, &mut pixmap);
    }
}

pub fn draw_image(frame: &mut [u8], buf_w: i32, buf_h: i32, params: &DrawImageParams) {
    let config = crate::config::AppConfig::get();
    let check_size = config.ui.checkerboard_size.max(1) as i32;
    let check_color_1 = Rgb::from(crate::utils::parse_color(&config.ui.checkerboard_color_1));
    let check_color_2 = Rgb::from(crate::utils::parse_color(&config.ui.checkerboard_color_2));

    match params.asset {
        LoadedAsset::Raster {
            width,
            height,
            frames,
        } => draw_raster_image(
            frame,
            buf_w,
            buf_h,
            params,
            *width,
            *height,
            frames,
            check_size,
            check_color_1,
            check_color_2,
        ),
        LoadedAsset::Vector {
            tree,
            base_width,
            base_height,
            internal_transform,
        } => draw_vector_image(
            frame,
            buf_w,
            buf_h,
            params,
            tree,
            *base_width,
            *base_height,
            *internal_transform,
            check_size,
            check_color_1,
            check_color_2,
        ),
    }
}

fn create_draw_cmd(i: usize, slot: &ImageSlot, ctx: &GridContext) -> Option<DrawCmd> {
    let col = (i as u32) % ctx.cols;
    let row = (i as u32) / ctx.cols;

    let x_cell = (ctx.margin_x + col * ctx.cell_size) as i32;
    let y_cell = (row * ctx.cell_size + ctx.padding / 2) as i32 - ctx.scroll_y;

    if y_cell + (ctx.cell_size as i32) < 0 || y_cell > ctx.buf_h {
        return None;
    }

    let is_selected = i == ctx.selected_idx;
    let mut is_marked = false;
    let mut thumb_data = None;
    let placeholder_color = match slot {
        ImageSlot::Error(_) => ctx.colors.error,
        _ => ctx.colors.loading,
    };

    let (target_w, target_h) = if let ImageSlot::MetadataLoaded(item) = slot {
        is_marked = ctx
            .marked_paths
            .contains(&item.path.to_string_lossy().to_string());
        thumb_data = ctx.cache.get_thumbnail(&item.path);

        if let Some(ref data) = thumb_data {
            (data.0 as i32, data.1 as i32)
        } else {
            let aspect = item.width as f64 / item.height as f64;
            if aspect >= 1.0 {
                (
                    ctx.thumb_size as i32,
                    (ctx.thumb_size as f64 / aspect) as i32,
                )
            } else {
                (
                    (ctx.thumb_size as f64 * aspect) as i32,
                    ctx.thumb_size as i32,
                )
            }
        }
    } else {
        (ctx.thumb_size as i32, ctx.thumb_size as i32)
    };

    let target_x = x_cell + (ctx.thumb_size as i32 - target_w) / 2;
    let target_y = y_cell + (ctx.thumb_size as i32 - target_h) / 2;
    let rect = Rect::new(target_x, target_y, target_w, target_h);

    let padding = ctx.border_gap + ctx.border_thickness + ctx.mark_size;
    let y_min = target_y - padding;
    let y_max = target_y + target_h + padding;

    Some(DrawCmd {
        y_min,
        y_max,
        rect,
        thumb_data,
        is_selected,
        is_marked,
        placeholder_color,
    })
}

fn gather_grid_commands(
    buf_w: i32,
    buf_h: i32,
    images: &[ImageSlot],
    cache: &CacheManager,
    selected_idx: usize,
    colors: &GridColors,
    marked_paths: &std::collections::HashSet<String>,
) -> Vec<DrawCmd> {
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

    let ctx = GridContext {
        buf_h,
        cols,
        margin_x,
        cell_size,
        padding,
        scroll_y,
        selected_idx,
        colors,
        marked_paths,
        cache,
        thumb_size,
        border_gap: config.ui.selected_border_padding as i32,
        border_thickness: config.ui.selected_border_width as i32,
        mark_size: config.ui.mark_indicator_size as i32,
    };

    images
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| create_draw_cmd(i, slot, &ctx))
        .collect()
}

fn render_grid_scanline(
    row_pixels: &mut [u8],
    y: i32,
    buf_w: i32,
    draw_commands: &[DrawCmd],
    colors: &GridColors,
    border_gap: i32,
    border_thickness: i32,
    mark_size: i32,
) {
    for cmd in draw_commands.iter().filter(|c| y >= c.y_min && y < c.y_max) {
        if let Some(data) = &cmd.thumb_data {
            draw_thumbnail_scanline(row_pixels, y, buf_w, cmd.rect, &data.2);
        } else {
            draw_border_scanline(
                row_pixels,
                y,
                buf_w,
                cmd.rect,
                cmd.placeholder_color,
                border_thickness,
            );
        }

        if cmd.is_selected {
            let offset = border_gap + border_thickness;
            let sel_rect = Rect::new(
                cmd.rect.x - offset,
                cmd.rect.y - offset,
                cmd.rect.w + offset * 2,
                cmd.rect.h + offset * 2,
            );
            draw_border_scanline(
                row_pixels,
                y,
                buf_w,
                sel_rect,
                colors.accent,
                border_thickness,
            );
        }

        if cmd.is_marked {
            draw_mark_scanline(
                row_pixels,
                y,
                buf_w,
                cmd.rect,
                border_gap,
                border_thickness,
                mark_size,
                colors.mark,
            );
        }
    }
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
    let border_gap = config.ui.selected_border_padding as i32;
    let border_thickness = config.ui.selected_border_width as i32;
    let mark_size = config.ui.mark_indicator_size as i32;

    let draw_commands = gather_grid_commands(
        buf_w,
        buf_h,
        images,
        cache,
        selected_idx,
        colors,
        marked_paths,
    );

    clear(frame, colors.bg);

    frame
        .par_chunks_exact_mut((buf_w * 4) as usize)
        .enumerate()
        .for_each(|(y, row_pixels)| {
            render_grid_scanline(
                row_pixels,
                y as i32,
                buf_w,
                &draw_commands,
                colors,
                border_gap,
                border_thickness,
                mark_size,
            );
        });
}

fn draw_thumbnail_scanline(row_pixels: &mut [u8], y: i32, buf_w: i32, rect: Rect, pixels: &[u8]) {
    if y < rect.y || y >= rect.y + rect.h {
        return;
    }

    let row_idx = y - rect.y;
    let src_row_start = (row_idx * rect.w) as usize * 4;
    let dest_x_start = rect.x.max(0);
    let dest_x_end = (rect.x + rect.w).min(buf_w);

    if dest_x_end <= dest_x_start {
        return;
    }

    let src_offset_x = (dest_x_start - rect.x) as usize * 4;
    let copy_len = (dest_x_end - dest_x_start) as usize * 4;
    let dest_row_start = (dest_x_start as usize) * 4;

    if src_row_start + src_offset_x + copy_len > pixels.len()
        || dest_row_start + copy_len > row_pixels.len()
    {
        return;
    }

    let src_slice = &pixels[src_row_start + src_offset_x..src_row_start + src_offset_x + copy_len];
    let dest_slice = &mut row_pixels[dest_row_start..dest_row_start + copy_len];
    assert!(dest_slice.len() % 4 == 0);

    for (src_chunk, dest_chunk) in src_slice
        .chunks_exact(4)
        .zip(dest_slice.chunks_exact_mut(4))
    {
        let src_a = src_chunk[3] as u32;
        if src_a == 255 {
            dest_chunk.copy_from_slice(src_chunk);
        } else if src_a > 0 {
            let src_px = [src_chunk[0], src_chunk[1], src_chunk[2], src_chunk[3]];
            let bg_px = [dest_chunk[0], dest_chunk[1], dest_chunk[2], 255];
            let blended = blend_swar(src_px, bg_px, src_a);
            dest_chunk.copy_from_slice(&blended);
        }
    }
}

fn draw_mark_scanline(
    row_pixels: &mut [u8],
    y: i32,
    buf_w: i32,
    rect: Rect,
    border_gap: i32,
    border_thickness: i32,
    mark_size: i32,
    color: Rgb,
) {
    if mark_size <= 0 {
        return;
    }

    let m_x = rect.x + rect.w + border_gap + border_thickness / 2 - mark_size / 2;
    let m_y = rect.y + rect.h + border_gap + border_thickness / 2 - mark_size / 2;

    if y >= m_y && y < m_y + mark_size {
        let start_draw_x = m_x.max(0);
        let end_draw_x = (m_x + mark_size).min(buf_w);

        for x in start_draw_x..end_draw_x {
            let idx = (x as usize) * 4;
            if idx + 4 <= row_pixels.len() {
                row_pixels[idx] = color.r;
                row_pixels[idx + 1] = color.g;
                row_pixels[idx + 2] = color.b;
                row_pixels[idx + 3] = 255;
            }
        }
    }
}

fn draw_border_scanline(
    row_pixels: &mut [u8],
    y: i32,
    buf_w: i32,
    rect: Rect,
    color: Rgb,
    thickness: i32,
) {
    let thickness = thickness.max(0);
    if thickness == 0 {
        return;
    }

    let in_vertical_range = y >= rect.y && y < rect.y + rect.h;
    if !in_vertical_range {
        return;
    }

    let in_top = y >= rect.y && y < rect.y + thickness;
    let in_bottom = y >= rect.y + rect.h - thickness && y < rect.y + rect.h;

    let color_alpha = [color.r, color.g, color.b, 255];

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
        draw_span(rect.x, rect.x + rect.w, row_pixels);
    } else {
        draw_span(rect.x, rect.x + thickness, row_pixels);
        draw_span(rect.x + rect.w - thickness, rect.x + rect.w, row_pixels);
    }
}
