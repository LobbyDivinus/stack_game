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
use core::{ptr, fmt::Write};
use stm32f7::lcd::font::FontRenderer;
use stm32f7::{board, embedded, lcd, sdram, system_clock, touch, i2c};


const FPS: i32 = 60;

static TTF: &[u8] = include_bytes!("../RobotoMono-Bold.ttf");

mod renderer;
use renderer::{Renderer, WIDTH, HEIGHT};


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
    let highscore: &mut i32 = &mut 0;

    loop {
        game(&mut renderer, &mut i2c_3, highscore);
    }
}

fn game(renderer: &mut Renderer, i2c_3: &mut i2c::I2C, highscore: &mut i32) {
    renderer.clear();

    let white_color = lcd::Color::from_hex(0xffffff);

    let block_height = 15;
    let mut score = 0;
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
    block_color = Renderer::hsv_color(h, s, v);

    draw_block(
        renderer,
        0,
        last_block_start - 1,
        block_height,
        last_block_width,
        block_color,
    );

    h += 20f32;
    block_color = Renderer::hsv_color(h, s, v);

    let font_renderer = FontRenderer::new(TTF, 20.0);
    font_renderer.render("Current Score", get_font_drawer_portrait(renderer, 0, 0));
    font_renderer.render("Highscore", get_font_drawer_portrait(renderer, HEIGHT - 90, 0));

    let mut redraw_score = true;
    let mut redraw_highscore = true;

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
            block_color = Renderer::hsv_color(h, s, v);

            if block_width < 3 {
                return;
            }

            score += 1;
            if score > *highscore {
                *highscore = score;
                redraw_highscore = true;
            }

            redraw_score = true;
        }
        last_tapped = tapped;

        if redraw_score {
            renderer.clear_area(WIDTH - 40, 0, 20, 40);
            font_renderer.render(&score.to_string(), get_font_drawer_portrait(renderer, 0, 20));
            redraw_score = false;
        }
        if redraw_highscore {
            renderer.clear_area(WIDTH - 40, HEIGHT - 40, 20, 40);
            font_renderer.render(&(*highscore).to_string(), get_font_drawer_portrait(renderer, HEIGHT - 40, 20));
            redraw_highscore = false;
        }

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

fn get_font_drawer_portrait<'a>(renderer: &'a mut Renderer, px: i32, py: i32) -> impl FnMut(usize, usize, f32) + 'a {
    move |x,y,v| {
        let i = (255f32 * v) as u8;
        if i > 128 {
            renderer.set_pixel(WIDTH - y as i32 - py, x as i32 + px, lcd::Color::rgb(i, i, i));
        }
    }
}


fn draw_block(d: &mut Renderer, x: i32, y: i32, w: i32, h: i32, color: lcd::Color) {
    d.draw_rect(x, y, w, h, color);
    for i in 0..h / 15 {
        d.draw_rect(x + w / 2 - 3, y + i * 15 + 5, 9, 7, color);
        d.draw_line(
            x + w / 2 + 1,
            y + i * 15 + 5,
            x + w / 2 + 1,
            y + i * 15 + 11,
            color,
        );
        d.draw_line(
            x + w / 2 - 3,
            y + i * 15 + 8,
            x + w / 2 + 5,
            y + i * 15 + 8,
            color,
        );
    }
}


