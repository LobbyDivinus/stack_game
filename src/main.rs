#![no_std]
#![no_main]
#![feature(compiler_builtins_lib)]
#![feature(alloc)]
#![feature(fnbox)]
#![feature(unboxed_closures)]

extern crate alloc;
extern crate compiler_builtins;
extern crate r0;
extern crate stm32f7_discovery as stm32f7;

use alloc::String;
use alloc::string::ToString;
use alloc::boxed::{Box,FnBox};
use core::{ptr, fmt::Write};
use stm32f7::lcd::font::FontRenderer;
use stm32f7::{board, embedded, lcd, sdram, system_clock, touch, i2c};

const WIDTH: i32 = 480;
const HEIGHT: i32 = 272;
const PIXEL_BUFFER_SIZE: usize = 4000;
const FPS: i32 = 60;

static TTF: &[u8] = include_bytes!("../RobotoMono-Bold.ttf");

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

struct Renderer {
    bg_color: lcd::Color,
    pixel_markers: [bool; (WIDTH * HEIGHT) as usize],
    drawn_pixels: [Point; 2 * PIXEL_BUFFER_SIZE],
    drawn_pixel_count: [usize; 2],
    current_buffer: u8,
    layer: lcd::Layer<lcd::FramebufferArgb8888>,
    direct: bool,
    frame_counter: i32,
}

impl Renderer {
    fn new(l: lcd::Layer<lcd::FramebufferArgb8888>, background: lcd::Color) -> Renderer {
        Renderer {
            bg_color: background,
            pixel_markers: [false; (WIDTH * HEIGHT) as usize],
            drawn_pixels: [Point { x: 0, y: 0 }; 2 * PIXEL_BUFFER_SIZE],
            drawn_pixel_count: [0; 2],
            current_buffer: 0,
            layer: l,
            direct: true,
            frame_counter: 0,
        }
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: lcd::Color) {
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

    fn begin_frame(&mut self) {
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

    fn end_frame(&mut self) {
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

    fn flush(&mut self) {
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

    fn clear(&mut self) {
        self.flush();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let color = self.get_background(x, y);
                self.layer
                    .print_point_color_at(x as usize, y as usize, color);
            }
        }
    }

    fn clear_area(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.flush();
        for py in y..y + h {
            for px in x..x + w {
                let color = self.get_background(px, py);
                self.layer
                    .print_point_color_at(px as usize, py as usize, color);
            }
        }
    }

    fn get_background(&self, x: i32, y: i32) -> lcd::Color {
        lcd::Color::rgb((WIDTH / 8 - x / 8) as u8, 0, (x / 8) as u8)
    }
}

#[no_mangle]
pub unsafe extern "C" fn reset() -> ! {
    extern "C" {
        static __DATA_LOAD: u32;
        static __DATA_END: u32;
        static mut __DATA_START: u32;
        static mut __BSS_START: u32;
        static mut __BSS_END: u32;
    }

    let data_load = &__DATA_LOAD;
    let data_start = &mut __DATA_START;
    let data_end = &__DATA_END;
    let bss_start = &mut __BSS_START;
    let bss_end = &__BSS_END;

    // initializes the .data section
    // (copy the data segment initializers from flash to RAM)
    r0::init_data(data_start, data_end, data_load);
    // zeroes the .bss section
    r0::zero_bss(bss_start, bss_end);

    stm32f7::heap::init();

    // Initialize the floating point unit
    let scb = stm32f7::cortex_m::peripheral::scb_mut();
    scb.cpacr.modify(|v| v | 0b1111 << 20);

    main(board::hw());
}

fn main(hw: board::Hardware) -> ! {
    use embedded::interfaces::gpio::{self, Gpio};

    let board::Hardware {
        rcc,
        pwr,
        flash,
        fmc,
        ltdc,
        gpio_a,
        gpio_b,
        gpio_c,
        gpio_d,
        gpio_e,
        gpio_f,
        gpio_g,
        gpio_h,
        gpio_i,
        gpio_j,
        gpio_k,
        i2c_3,
        sai_2,
        syscfg,
        ethernet_mac,
        ethernet_dma,
        nvic,
        exti,
        ..
    } = hw;

    let mut gpio = Gpio::new(
        gpio_a,
        gpio_b,
        gpio_c,
        gpio_d,
        gpio_e,
        gpio_f,
        gpio_g,
        gpio_h,
        gpio_i,
        gpio_j,
        gpio_k,
    );

    system_clock::init(rcc, pwr, flash);

    // enable all gpio ports
    rcc.ahb1enr.update(|r| {
        r.set_gpioaen(true);
        r.set_gpioben(true);
        r.set_gpiocen(true);
        r.set_gpioden(true);
        r.set_gpioeen(true);
        r.set_gpiofen(true);
        r.set_gpiogen(true);
        r.set_gpiohen(true);
        r.set_gpioien(true);
        r.set_gpiojen(true);
        r.set_gpioken(true);
    });

    // init sdram (needed for display buffer)
    sdram::init(rcc, fmc, &mut gpio);

    // lcd controller
    let mut lcd = lcd::init(ltdc, rcc, &mut gpio);
    let bg_color = lcd::Color::from_hex(0x000000);
    //cd.set_background_color(bg_color);
    let mut layer_1 = lcd.layer_1().unwrap();
    let mut layer_2 = lcd.layer_2().unwrap();

    //layer_1.clear();
    layer_2.clear();
    //lcd::init_stdout(layer_2);

    // i2c
    i2c::init_pins_and_clocks(rcc, &mut gpio);
    let mut i2c_3 = i2c::init(i2c_3);
    i2c_3.test_1();
    i2c_3.test_2();

    touch::check_family_id(&mut i2c_3).unwrap();

    let mut renderer = Renderer::new(layer_1, bg_color);

    loop {
        game(&mut renderer, &mut i2c_3);
    }
}

fn game(renderer: &mut Renderer, i2c_3: &mut i2c::I2C) {
    renderer.clear();

    let white_color = lcd::Color::from_hex(0xffffff);

    let block_height = 15;
    let mut cur_stack_height = block_height;
    let mut block_width = 150;
    let mut last_block_width = block_width;
    let mut last_block_start = (HEIGHT - last_block_width) / 2;
    let mut last_tapped = false;
    let mut last_ms = system_clock::ticks();
    let mut block_color = lcd::Color::rgb(255, 255, 255);
    let mut ms = system_clock::ticks();
    let mut h = 0f32;
    let mut s = 0.8f32;
    let mut v = 1f32;
    block_color = hsv_color(h, s, v);

    draw_block(
        renderer,
        0,
        last_block_start - 1,
        block_height,
        last_block_width,
        block_color,
    );

    block_color = hsv_color(h, s, v);

    let font_renderer = FontRenderer::new(TTF, 20.0);

    loop {
        ms = system_clock::ticks();
        renderer.begin_frame();

        let range_start = last_block_start - 5 * last_block_width / 4;
        let range_width = 10 * last_block_width / 4;

        let mut p_time = 15 * block_width + 500;
        let mut p = ((ms - last_ms) as i32 % p_time) as f32 / p_time as f32 * 2f32;
        if p > 1f32 {
            p = 2f32 - p;
        }
        p = -2f32 * p * p * p + 3f32 * p * p;
        let mut block_start = range_start + (p * range_width as f32) as i32;

        draw_block(
            renderer,
            cur_stack_height,
            block_start,
            block_height,
            block_width,
            block_color,
        );

        // poll for new touch data
        let mut tapped = false;
        for touch in &touch::touches(i2c_3).unwrap() {
            tapped = true;
        }

        renderer.end_frame();

        if tapped && !last_tapped {
            renderer.flush();
            if block_start < last_block_start {
                block_width -= last_block_start - block_start;
                block_start = last_block_start;
            }
            if block_start + block_width > last_block_start + last_block_width {
                block_width -= block_start + block_width - last_block_start - last_block_width;
            }
            draw_block(
                renderer,
                cur_stack_height - 1,
                block_start,
                block_height + 1,
                block_width,
                block_color,
            );

            last_block_start = block_start;
            last_block_width = block_width;
            cur_stack_height += block_height;
            last_ms = ms - (ms as i32 % 100) as usize;
            h += 15f32;
            block_color = hsv_color(h, s, v);

            if block_width < 3 {
                return;
            }

            renderer.clear_area(WIDTH - 20, 0, 20, HEIGHT);
            let mut text = String::new();
            text.push_str("Current score: ");
            text.push_str(&cur_stack_height.to_string());
            //let f = get_font_drawer(renderer, 0, 0);
            //font_renderer.render(&text, f);
        }
        last_tapped = tapped;

        // Timer
        let ms_per_frame = (1000 / FPS) as usize;
        loop {
            let cur_ms = system_clock::ticks();
            if cur_ms - ms >= ms_per_frame {
                break;
            }
        }
    }
}

/*fn get_font_drawer(renderer: &'static mut Renderer, px: &'static i32, py: &'static i32) -> Box<Fn(usize, usize, f32)> {
    Box::new(|x,y,v| {
        let i = (255f32 * v) as u8;
        if i > 128 {
            renderer.set_pixel(WIDTH - y as i32 + py, x as i32 + px, lcd::Color::rgb(i, i, i));
        }
    })
}*/

fn abs(x: f32) -> f32 {
    if x < 0f32 {
        -x
    } else {
        x
    }
}

fn hsv_color(hue: f32, s: f32, v: f32) -> lcd::Color {
    let h = (hue as i32 % 360) as f32;

    let c = v * s;
    let x = c * (1f32 - abs((h as i32 % 120) as f32 / 60f32 - 1f32));
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

fn draw_block(d: &mut Renderer, x: i32, y: i32, w: i32, h: i32, color: lcd::Color) {
    draw_rect(d, x, y, w, h, color);
    for i in 0..h / 15 {
        draw_rect(d, x + w / 2 - 3, y + i * 15 + 5, 9, 7, color);
        draw_line(
            d,
            x + w / 2 + 1,
            y + i * 15 + 5,
            x + w / 2 + 1,
            y + i * 15 + 11,
            color,
        );
        draw_line(
            d,
            x + w / 2 - 3,
            y + i * 15 + 8,
            x + w / 2 + 5,
            y + i * 15 + 8,
            color,
        );
    }
}

fn draw_line(d: &mut Renderer, x0: i32, y0: i32, x1: i32, y1: i32, color: lcd::Color) {
    if y0 == y1 {
        for px in x0..=x1 {
            d.set_pixel(px, y0, color);
        }
    } else if x0 == x1 {
        for py in y0..=y1 {
            d.set_pixel(x0, py, color)
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
            d.set_pixel(x, y, color);
            error = error + d_err;
            while error >= 0.5f32 {
                if d_y > 0 {
                    y += 1;
                } else if d_y < 0 {
                    y -= 1;
                }
                if error >= 1.5f32 {
                    d.set_pixel(x, y, color);
                }
                error = error - 1f32;
            }
        }
    }
}

fn draw_rect_solid(d: &mut Renderer, x: i32, y: i32, w: i32, h: i32, color: lcd::Color) {
    for py in y..y + h {
        for px in x..x + w {
            d.set_pixel(px, py, color);
        }
    }
}

fn draw_rect(d: &mut Renderer, x: i32, y: i32, w: i32, h: i32, color: lcd::Color) {
    for px in x..x + w {
        d.set_pixel(px, y, color);
    }
    for py in y + 1..y + h - 1 {
        d.set_pixel(x, py, color);
    }
    for py in y + 1..y + h - 1 {
        d.set_pixel((x + w - 1), py, color);
    }
    for px in x..x + w {
        d.set_pixel(px, (y + h - 1), color);
    }
}
