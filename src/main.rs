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
use alloc::Vec;
use alloc::string::ToString;
use core::{ptr, fmt::Write};
use stm32f7::lcd::font::FontRenderer;
use stm32f7::{board, embedded, lcd, lcd::Color, sdram, system_clock, touch, i2c};

const FPS: i32 = 60;

static TTF: &[u8] = include_bytes!("../RobotoMono-Bold.ttf");

mod renderer;
use renderer::Renderer;

mod block;
use block::Block;

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
    let bg_color = Color::from_hex(0x000000);
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
    renderer.set_portrait(true);

    let highscore: &mut i32 = &mut 0;

    loop {
        game(&mut renderer, &mut i2c_3, highscore);
    }
}

fn game(renderer: &mut Renderer, i2c_3: &mut i2c::I2C, highscore: &mut i32) {
    renderer.clear();

    let white_color = Color::from_hex(0xffffff);

    let xmax = renderer.get_width();
    let ymax = renderer.get_height();

    let block_height = 15;
    let mut score = 0;
    let mut cur_stack_height = block_height;
    let mut block_width = 150;
    let mut last_block_width = block_width;
    let mut last_block_start = (xmax - last_block_width) / 2;
    let mut last_tapped = false;
    let mut last_ms = system_clock::ticks();
    let mut block_color = Color::rgb(255, 255, 255);
    let mut ms = system_clock::ticks();

    let base_x = xmax / 2;
    let mut base_y = ymax;


    let font_renderer = FontRenderer::new(TTF, 20.0);
    font_renderer.render("Current Score", get_font_drawer(renderer, 0, 0));
    font_renderer.render("Highscore", get_font_drawer(renderer, xmax - 81, 0));

    let mut redraw_score = true;
    let mut redraw_highscore = true;

    let mut blocks = Vec::new();
    let mut current_block = Block::new(-50, -60, -50, 100, 60, 100);
    blocks.push(current_block);
    current_block = Block::new(-50, -60 - block_height, -50, 100, block_height, 100);

    loop {
        ms = system_clock::ticks();
        renderer.begin_frame();

        let range_start = last_block_start - 5 * last_block_width / 4;
        let range_width = 10 * last_block_width / 4;

        let mut size = current_block.width;
        if current_block.depth > size {
            size = current_block.depth;
        }
        let mut p_time = 15 * size + 500;
        let mut p = ((ms - last_ms) as i32 % p_time) as f32 / p_time as f32 * 2f32;
        if p > 1f32 {
            p = 2f32 - p;
        }
        p = -2f32 * p * p * p + 3f32 * p * p;
        let mut block_start = range_start + (p * range_width as f32) as i32;

        for (i, b) in blocks.iter().enumerate() {
            let mut color = Color::rgb(180, 180, 180);
            if i == blocks.len() - 1 {
                color = Color::rgb(255, 0, 0);
            }
            b.draw(renderer, base_x, base_y, color);
        }

        {
            let last_block = &blocks.last().unwrap();
            if score % 2 == 0 {
                current_block.x =
                    ((3f32 * current_block.width as f32 * (p - 0.5f32)) as i32 - current_block.width / 2 + last_block.x + last_block.width / 2) / 2 * 2;
            } else {
                current_block.z =
                    ((3f32 * current_block.depth as f32 * (p - 0.5f32)) as i32 - current_block.depth / 2 + last_block.y + last_block.depth / 2) / 2 * 2;
            }
            current_block.draw(renderer, base_x, base_y, white_color);
        }


        // poll for new touch data
        let mut tapped = false;
        tapped = !&touch::touches(i2c_3).unwrap().is_empty();

        renderer.end_frame();

        if tapped && !last_tapped {
            {
                let last_block = &blocks.last().unwrap();
                if current_block.x < last_block.x {
                    current_block.width -= last_block.x - current_block.x;
                    current_block.x = last_block.x;
                }
                if current_block.x + current_block.width > last_block.x + last_block.width {
                    current_block.width -= current_block.x + current_block.width - last_block.x - last_block.width;
                }
                if current_block.z < last_block.z {
                    current_block.depth -= last_block.z - current_block.z;
                    current_block.z = last_block.z;
                }
                if current_block.z + current_block.depth > last_block.z + last_block.depth {
                    current_block.depth -= current_block.z + current_block.depth - last_block.z - last_block.depth;
                }
            }

            blocks.push(current_block);
            let last_block = &blocks.last().unwrap();
            current_block = Block::new(last_block.x, last_block.y - block_height, last_block.z, last_block.width, block_height, last_block.depth);


            last_ms = ms - (ms as i32 % 100) as usize;

            if current_block.width < 4 || current_block.height < 4 {
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
            renderer.clear_area(0, 20, 20, 40);
            font_renderer.render(&score.to_string(), get_font_drawer(renderer, 0, 20));
            redraw_score = false;
        }
        if redraw_highscore {
            renderer.clear_area(xmax - 40, 20, 40, 20);
            let text = (*highscore).to_string();
            font_renderer.render(
                &text,
                get_font_drawer(renderer, xmax - 9 * text.chars().count() as i32, 20),
            );
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

fn get_font_drawer<'a>(
    renderer: &'a mut Renderer,
    px: i32,
    py: i32,
) -> impl FnMut(usize, usize, f32) + 'a {
    move |x, y, v| {
        let i = (255f32 * v) as u8;
        if i > 128 {
            renderer.set_pixel(x as i32 + px, y as i32 + py, Color::rgb(i, i, i));
        }
    }
}
