use crate::image_item::ImageItem;

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
        item: &ImageItem,
        frame_idx: usize,
        scale: f64,
        off_x: i32,
        off_y: i32,
    ) {
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
}
