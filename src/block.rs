
extern crate stm32f7_discovery as stm32f7;

use stm32f7::lcd;

use renderer::Renderer;

pub struct Block {
    x: i32,
    y: i32,
    z: i32,
    w: i32,
    h: i32,
    d: i32,
}

impl Block {
    pub fn new(pos_x: i32, pos_y: i32, pos_z: i32, width: i32, height: i32, depth: i32) -> Block {
        Block {
            x: pos_x,
            y: pos_y,
            z: pos_z,
            w: width,
            h: height,
            d: depth,
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, base_x: i32, base_y: i32) {
        renderer.draw_block_3d(
            base_x + self.x + self.z,
            base_y + self.y + self.x / 2 - self.z / 2,
            self.w,
            self.h,
            self.d,
            lcd::Color::rgb(255, 255, 255),
        );
    }
}