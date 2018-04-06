
extern crate stm32f7_discovery as stm32f7;

use stm32f7::lcd;

pub const WIDTH: i32 = 480;
pub const HEIGHT: i32 = 272;

const PIXEL_BUFFER_SIZE: usize = 4000;


struct Point {
    x: i16,
    y: i16,
}

impl Copy for Point {}
impl Clone for Point {
    fn clone(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
}

pub struct Renderer {
    bg_color: lcd::Color,
    pixel_markers: [bool; (WIDTH * HEIGHT) as usize],
    drawn_pixels: [Point; 2 * PIXEL_BUFFER_SIZE],
    drawn_pixel_count: [usize; 2],
    current_buffer: u8,
    layer: lcd::Layer<lcd::FramebufferArgb8888>,
    direct: bool,
    frame_counter: i32,
    portrait: bool
}

impl Renderer {
    pub fn new(l: lcd::Layer<lcd::FramebufferArgb8888>, background: lcd::Color) -> Renderer {
        Renderer {
            bg_color: background,
            pixel_markers: [false; (WIDTH * HEIGHT) as usize],
            drawn_pixels: [Point { x: 0, y: 0 }; 2 * PIXEL_BUFFER_SIZE],
            drawn_pixel_count: [0; 2],
            current_buffer: 0,
            layer: l,
            direct: true,
            frame_counter: 0,
            portrait: false
        }
    }

    pub fn set_pixel(&mut self, px: i32, py: i32, color: lcd::Color) {
        let mut x = px;
        let mut y = py;

        if (self.portrait) {
            x = WIDTH - py;
            y = px;
        }

        if x < 0 || x >= WIDTH || y < 0 || y >= HEIGHT {
            return;
        }

        if self.direct {
            self.layer
                .print_point_color_at(x as usize, y as usize, color);
        } else {
            self.pixel_markers[(x + y * WIDTH) as usize] = true;
            let offset = self.current_buffer as usize * PIXEL_BUFFER_SIZE;
            let index = self.drawn_pixel_count[self.current_buffer as usize] + offset;
            self.drawn_pixels[index].x = x as i16;
            self.drawn_pixels[index].y = y as i16;

            let pixel = self.drawn_pixels[index];
            self.layer
                .print_point_color_at(pixel.x as usize, pixel.y as usize, color);

            self.drawn_pixel_count[self.current_buffer as usize] += 1;
        }
    }

    pub fn begin_frame(&mut self) {
        let last_buffer = 1 - self.current_buffer;
        let offset = last_buffer as usize * PIXEL_BUFFER_SIZE;
        let size = self.drawn_pixel_count[last_buffer as usize];
        for i in offset..size + offset {
            let pixel = self.drawn_pixels[i as usize];
            let x = pixel.x as i32;
            let y = pixel.y as i32;
            self.pixel_markers[(x + y * WIDTH) as usize] = false;
        }
        self.drawn_pixel_count[self.current_buffer as usize] = 0;
        self.direct = false;
    }

    pub fn end_frame(&mut self) {
        let last_buffer = 1 - self.current_buffer;
        let offset = last_buffer as usize * PIXEL_BUFFER_SIZE;
        let size = self.drawn_pixel_count[last_buffer as usize];
        for i in 0..size {
            let pixel = self.drawn_pixels[(i + offset) as usize];
            let x = pixel.x as i32;
            let y = pixel.y as i32;
            if !self.pixel_markers[(x + y * WIDTH) as usize] {
                let color = self.get_background(x, y);
                self.layer
                    .print_point_color_at(x as usize, y as usize, color);
            }
        }

        self.current_buffer = last_buffer;
        self.frame_counter += 1;
        self.direct = true;
    }

    pub fn flush(&mut self) {
        for buf in 0..2 {
            let offset = buf as usize * PIXEL_BUFFER_SIZE;
            let size = self.drawn_pixel_count[buf as usize];
            for i in 0..size {
                let pixel = self.drawn_pixels[(i + offset) as usize];
                let x = pixel.x as i32;
                let y = pixel.y as i32;
                let color = self.get_background(x, y);
                self.layer
                    .print_point_color_at(x as usize, y as usize, color);
                self.pixel_markers[(x + y * WIDTH) as usize] = false;
            }
            self.drawn_pixel_count[buf as usize] = 0;
        }
    }

    pub fn clear(&mut self) {
        self.flush();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let color = self.get_background(x, y);
                self.layer
                    .print_point_color_at(x as usize, y as usize, color);
            }
        }
    }

    pub fn clear_area(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.flush();
        for py in y..y + h {
            for px in x..x + w {
                let color = self.get_background(px, py);
                self.layer
                    .print_point_color_at(px as usize, py as usize, color);
            }
        }
    }

    pub fn get_background(&self, x: i32, y: i32) -> lcd::Color {
        let mut color = lcd::Color::rgb(0, ((WIDTH - x) / 5) as u8, 64);
        if (1329 * (x ^ (y * 717)) + 971) % (WIDTH - x + 200) == 0 {
            let mut alpha = 1f32 - (x as f32 / WIDTH as f32);
            alpha *= alpha;
            alpha = 1f32 - alpha;
            color.red = ((1f32 - alpha) * color.red as f32 + alpha * 255f32) as u8;
            color.green = ((1f32 - alpha) * color.green as f32 + alpha * 255f32) as u8;
            color.blue = ((1f32 - alpha) * color.blue as f32 + alpha * 255f32) as u8;
        }
        color
    }

    pub fn set_portrait(&mut self, state: bool) {
        self.portrait = state;
    }

    pub fn draw_block_3d(&mut self, x: i32, y: i32, width: i32, height: i32, depth: i32, color: lcd::Color) {
        self.draw_line(x, y, x + width, y + width / 2, color);
        self.draw_line(x + width, y + width / 2, x + width + depth, y + width / 2 - depth / 2, color);
        self.draw_line(x, y, x + depth, y - depth / 2, color);
        self.draw_line(x + depth, y - depth / 2, x + width + depth, y + width / 2 - depth / 2, color);

        self.draw_line(x, y + height, x + width, y + width / 2 + height, color);
        self.draw_line(x + width, y + width / 2 + height, x + width + depth, y + width / 2 - depth / 2 + height, color);

        self.draw_line(x, y, x, y + height, color);
        self.draw_line(x + width, y + width / 2, x + width, y + width / 2 + height, color);
        self.draw_line(x + width + depth, y + width / 2 - depth / 2, x + width + depth, y + width / 2 - depth / 2 + height, color);
    }

    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: lcd::Color) {
        if y0 == y1 {
            for px in x0..=x1 {
                self.set_pixel(px, y0, color);
            }
        } else if x0 == x1 {
            for py in y0..=y1 {
                self.set_pixel(x0, py, color)
            }
        } else {
            let d_x = x1 - x0;
            let d_y = y1 - y0;
            let mut d_err = d_y as f32 / d_x as f32;
            if d_err < 0f32 {
                d_err = -d_err;
            }
            let mut error = 0f32;
            let mut y = y0;
            let mut sign_d_x = 1;
            let mut abs_d_x = d_x;
            if abs_d_x < 0 {
                abs_d_x = -abs_d_x;
                sign_d_x = -1;
            }
            for i in 0..=abs_d_x {
                let x = x0 + sign_d_x * i;
                self.set_pixel(x, y, color);
                error = error + d_err;
                while error >= 0.5f32 {
                    if d_y > 0 {
                        y += 1;
                    } else if d_y < 0 {
                        y -= 1;
                    }
                    if error >= 1.5f32 {
                        self.set_pixel(x, y, color);
                    }
                    error = error - 1f32;
                }
            }
        }
    }

    pub fn draw_rect_solid(&mut self, x: i32, y: i32, w: i32, h: i32, color: lcd::Color) {
        for py in y..y + h {
            for px in x..x + w {
                self.set_pixel(px, py, color);
            }
        }
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: lcd::Color) {
        for px in x..x + w {
            self.set_pixel(px, y, color);
        }
        for py in y + 1..y + h - 1 {
            self.set_pixel(x, py, color);
        }
        for py in y + 1..y + h - 1 {
            self.set_pixel((x + w - 1), py, color);
        }
        for px in x..x + w {
            self.set_pixel(px, (y + h - 1), color);
        }
    }

    pub fn hsv_color(hue: f32, s: f32, v: f32) -> lcd::Color {
        let h = (hue as i32 % 360) as f32;

        let c = v * s;
        let x = (h as i32 % 120) as f32 / 60f32 - 1f32;
        let x = c * (1f32 - if x < 0f32 { -x } else { x });
        let m = v - c;

        let mut rgb = (0f32, 0f32, 0f32);
        if h < 60f32 {
            rgb.0 = c;
            rgb.1 = x;
        } else if h < 120f32 {
            rgb.0 = x;
            rgb.1 = c;
        } else if h < 180f32 {
            rgb.1 = c;
            rgb.2 = x;
        } else if h < 240f32 {
            rgb.1 = x;
            rgb.2 = c;
        } else if h < 300f32 {
            rgb.0 = x;
            rgb.2 = c;
        } else {
            rgb.0 = c;
            rgb.2 = x;
        }

        rgb.0 += m;
        rgb.1 += m;
        rgb.2 += m;

        lcd::Color::rgb(
            (255f32 * rgb.0) as u8,
            (255f32 * rgb.1) as u8,
            (255f32 * rgb.2) as u8,
        )
    }
}