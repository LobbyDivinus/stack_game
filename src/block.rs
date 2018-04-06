
extern crate stm32f7_discovery as stm32f7;

use stm32f7::lcd::Color;

use renderer::Renderer;

pub struct Block {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
}

impl Block {
    pub fn new(pos_x: i32, pos_y: i32, pos_z: i32, w: i32, h: i32, d: i32) -> Block {
        Block {
            x: pos_x,
            y: pos_y,
            z: pos_z,
            width: w,
            height: h,
            depth: d
        }
    }

    pub fn draw(&self, renderer: &mut Renderer, base_x: i32, base_y: i32, color: Color) {
        renderer.draw_block_3d(
            base_x + self.x + self.z,
            base_y + self.y + self.x / 2 - self.z / 2,
            self.width,
            self.height,
            self.depth,
            color,
        );
    }
}