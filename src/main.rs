#![no_std]
#![no_main]
#![feature(compiler_builtins_lib)]
#![feature(alloc)]
#![feature(fnbox)]
#![feature(unboxed_closures)]
#![feature(conservative_impl_trait)]

extern crate alloc;
extern crate compiler_builtins;
extern crate r0;
extern crate stm32f7_discovery as stm32f7;

use alloc::Vec;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use stm32f7::lcd::font::FontRenderer;
use stm32f7::{board, embedded, lcd, sdram, system_clock, touch, i2c, lcd::Color};

const FPS: i32 = 60;

static TTF: &[u8] = include_bytes!("../RobotoMono-Bold.ttf");

mod renderer;
use renderer::{hsv_color, weight_color, Renderer};

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
    use embedded::interfaces::gpio::Gpio;

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

    let black_bg = move |_x, _y| bg_color;
    let transparent_bg = |_x, _y| Color::rgba(0, 0, 0, 0);
    let mut renderer = Renderer::new(&mut layer_1, Box::new(black_bg));
    let mut top_renderer = Renderer::new(&mut layer_2, Box::new(transparent_bg));

    renderer.set_portrait(true);
    top_renderer.set_portrait(true);

    let highscore: &mut i32 = &mut 0;
    let mut round = 0;

    loop {
        let f = get_background(&mut renderer, round);
        renderer.set_bg(Box::new(f));

        game(&mut renderer, &mut top_renderer, &mut i2c_3, highscore);

        round += 1;
    }
}

fn get_background<T: lcd::Framebuffer>(
    renderer: &mut Renderer<T>,
    _round: i32,
) -> impl FnMut(i32, i32) -> Color {
    let ymax = renderer.get_height();
    let bg_color = hsv_color(system_clock::ticks() as f32, 1f32, 0.25f32);
    move |x, y| {
        let alpha = y as f32 / ymax as f32;
        let base_color = weight_color(Color::rgb(0, 0, 64), 1f32 - alpha);
        let mut color = weight_color(bg_color, alpha);
        color.red += base_color.red;
        color.green += base_color.green;
        color.blue += base_color.blue;
        if (1329 * (x ^ (y * 717)) + 971) % (ymax - x + 200) == 0 {
            let mut alpha = y as f32 / ymax as f32;
            alpha *= alpha;
            alpha = 1f32 - alpha;
            color.red = ((1f32 - alpha) * color.red as f32 + alpha * 255f32) as u8;
            color.green = ((1f32 - alpha) * color.green as f32 + alpha * 255f32) as u8;
            color.blue = ((1f32 - alpha) * color.blue as f32 + alpha * 255f32) as u8;
        }
        fix_color(color)
    }
}

fn game<S: lcd::Framebuffer, T: lcd::Framebuffer>(
    renderer: &mut Renderer<S>,
    top_renderer: &mut Renderer<T>,
    i2c_3: &mut i2c::I2C,
    highscore: &mut i32,
) {
    renderer.clear();

    let white_color = Color::from_hex(0xffffff);

    let xmax = renderer.get_width();
    let ymax = renderer.get_height();

    let mut score = 0;
    let mut last_tapped = false;
    let mut last_ms = system_clock::ticks();
    let mut ms;

    let base_x = xmax / 2;
    let mut base_y = ymax;

    let font = FontRenderer::new(TTF, 20.0);
    let big_font = FontRenderer::new(TTF, 32.0);

    top_renderer.draw_text(&font, "Current Score", 0, 0, white_color);
    top_renderer.draw_text(&font, "Highscore", xmax - 81, 0, white_color);

    let mut redraw_score = true;
    let mut redraw_highscore = true;

    let mut hue = (last_ms % 360) as f32;

    let block_height = 15;
    let mut blocks = Vec::new();
    let mut current_block = Block::new(-50, -60, -50, 100, 60, 100, 0f32);
    draw_block(renderer, &current_block, base_x, base_y, hue);
    blocks.push(current_block);
    current_block = Block::new(-50, -60 - block_height, -50, 100, block_height, 100, 0f32);

    loop {
        ms = system_clock::ticks();

        let mut size = current_block.width;
        if current_block.depth > size {
            size = current_block.depth;
        }
        let p_time = 30 * size + 500;
        let mut p = ((ms - last_ms) as i32 % p_time) as f32 / p_time as f32 * 2f32;
        if p > 1f32 {
            p = 2f32 - p;
        }
        p = -2f32 * p * p * p + 3f32 * p * p;

        {
            let last_block = &blocks.last().unwrap();
            if score % 2 == 0 {
                current_block.x = ((3f32 * current_block.width as f32 * (p - 0.5f32)) as i32
                    - current_block.width / 2 + last_block.x
                    + last_block.width / 2) / 2 * 2;
            } else {
                current_block.z = ((3f32 * current_block.depth as f32 * (p - 0.5f32)) as i32
                    - current_block.depth / 2 + last_block.z
                    + last_block.depth / 2) / 2 * 2;
            }
            top_renderer.begin_frame();
            current_block.draw(top_renderer, base_x, base_y, white_color);
            top_renderer.end_frame();
        }

        let tapped = !&touch::touches(i2c_3).unwrap().is_empty();
        if tapped && !last_tapped {
            {
                let last_block = &blocks.last().unwrap();
                let fit_distance = 3;

                if abs(current_block.x - last_block.x) <= fit_distance
                    && abs(current_block.z - last_block.z) <= fit_distance
                {
                    current_block.x = last_block.x;
                    current_block.z = last_block.z;

                    let in_a_row = 5;
                    if blocks.len() >= in_a_row {
                        let mut is_perfect = true;
                        for i in blocks.len() - in_a_row + 1..blocks.len() {
                            let lower_block = &blocks[i - 1];
                            let b = &blocks[i];
                            if b.x != lower_block.x || b.z != lower_block.z
                                || b.width != lower_block.width
                                || b.depth != lower_block.depth
                            {
                                is_perfect = false;
                                break;
                            }
                        }
                        if is_perfect {
                            let bonus = 4;
                            current_block.x -= bonus;
                            current_block.z -= bonus;
                            current_block.width += 2 * bonus;
                            current_block.depth += 2 * bonus;
                        }
                    }
                } else {
                    if current_block.x < last_block.x {
                        current_block.width -= last_block.x - current_block.x;
                        current_block.x = last_block.x;
                    }
                    if current_block.x + current_block.width > last_block.x + last_block.width {
                        current_block.width -=
                            current_block.x + current_block.width - last_block.x - last_block.width;
                    }
                    if current_block.z < last_block.z {
                        current_block.depth -= last_block.z - current_block.z;
                        current_block.z = last_block.z;
                    }
                    if current_block.z + current_block.depth > last_block.z + last_block.depth {
                        current_block.depth -=
                            current_block.z + current_block.depth - last_block.z - last_block.depth;
                    }
                }
            }

            if current_block.width < 4 || current_block.depth < 4 {
                let text = "Game Over";
                let mut score_text = String::from("Your score is ");
                score_text.push_str(&score.to_string());

                top_renderer.set_immediate(true);
                top_renderer.begin_frame();
                top_renderer.draw_text(
                    &big_font,
                    text,
                    (xmax - text.len() as i32 * 14) / 2,
                    ymax / 2 - 32,
                    white_color,
                );
                top_renderer.draw_text(
                    &font,
                    &score_text,
                    (xmax - score_text.len() as i32 * 9) / 2,
                    ymax / 2,
                    white_color,
                );
                top_renderer.end_frame();
                top_renderer.set_immediate(false);
                return;
            }

            if base_y + current_block.y < ymax / 3 {
                let min_x = blocks.first().unwrap().min_x(base_x, base_y) - 5;
                let min_y = blocks.last().unwrap().min_y(base_x, base_y) - 5;
                let max_x = blocks.first().unwrap().max_x(base_x, base_y) + 5;
                renderer.clear_area(min_x, min_y, max_x - min_x + 1, ymax - min_y);
                base_y += ymax / 3;
                for b in blocks.iter() {
                    if b.min_y(base_x, base_y) < ymax {
                        draw_block(renderer, b, base_x, base_y, b.hue);
                    }
                }
            }

            hue = ((hue as i32 + 10) % 360) as f32;
            draw_block(renderer, &current_block, base_x, base_y, hue);
            blocks.push(current_block);
            let last_block = &blocks.last().unwrap();
            current_block = Block::new(
                last_block.x,
                last_block.y - block_height,
                last_block.z,
                last_block.width,
                block_height,
                last_block.depth,
                hue,
            );

            last_ms = ms - (ms as i32 % 100) as usize;

            score += 1;
            if score > *highscore {
                *highscore = score;
                redraw_highscore = true;
            }

            redraw_score = true;
        }
        last_tapped = tapped;

        if redraw_score {
            top_renderer.clear_area(0, 20, 40, 20);
            top_renderer.draw_text(&font, &score.to_string(), 0, 20, white_color);
            redraw_score = false;
        }
        if redraw_highscore {
            top_renderer.clear_area(xmax - 40, 20, 40, 20);
            let text = (*highscore).to_string();
            top_renderer.draw_text(
                &font,
                &text,
                xmax - 9 * text.chars().count() as i32,
                20,
                white_color,
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

fn abs(value: i32) -> i32 {
    if value < 0 {
        -value
    } else {
        value
    }
}

fn draw_block<T: lcd::Framebuffer>(
    renderer: &mut Renderer<T>,
    block: &Block,
    base_x: i32,
    base_y: i32,
    hue: f32,
) {
    let base_color = hsv_color(hue, 0.5f32, 1f32);

    let _outline_color = fix_color(weight_color(base_color, 1f32));
    let left_color = fix_color(weight_color(base_color, 1f32));
    let right_color = fix_color(weight_color(base_color, 0.6f32));
    let top_color = fix_color(weight_color(base_color, 0.8f32));

    block.draw_solid(renderer, base_x, base_y, left_color, right_color, top_color);
    //block.draw(renderer, base_x, base_y, outline_color);
}

fn fix_color(color: Color) -> Color {
    Color::from_hex(swap_bits(color.to_rgb(), 1, 4))
}

fn swap_bits(value: u32, pos0: u8, pos1: u8) -> u32 {
    let a = (value >> pos0) & 1;
    let b = (value >> pos1) & 1;
    value & !(1 << pos0) & !(1 << pos1) | (a << pos1) | (b << pos0)
}
