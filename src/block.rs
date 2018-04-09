#![allow(dead_code)]

extern crate stm32f7_discovery as stm32f7;

use stm32f7::lcd::{Color, Framebuffer};

use renderer::Renderer;

pub struct Block {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub hue: f32
}

impl Block {
    pub fn new(pos_x: i32, pos_y: i32, pos_z: i32, w: i32, h: i32, d: i32, hue: f32) -> Block {
        Block {
            x: pos_x,
            y: pos_y,
            z: pos_z,
            width: w,
            height: h,
            depth: d,
            hue: hue
        }
    }

    pub fn min_x(&self, base_x: i32, _base_y: i32) -> i32 {
        base_x + self.x + self.z
    }

    pub fn min_y(&self,_base_x: i32, base_y: i32) -> i32 {
        base_y + self.y + self.x / 2 - self.z / 2 - self.depth / 2
    }

    pub fn max_x(&self,base_x: i32, _base_y: i32) -> i32 {
        base_x + self.x + self.z + self.width + self.depth
    }

    pub fn max_y(&self,_base_x: i32, base_y: i32) -> i32 {
        base_y + self.y + self.x / 2 - self.z / 2 + self.width / 2 + self.height
    }

    pub fn draw<T: Framebuffer>(&self, renderer: &mut Renderer<T>, base_x: i32, base_y: i32, color: Color) {
        renderer.draw_block_3d(
            base_x + self.x + self.z,
            base_y + self.y + self.x / 2 - self.z / 2,
            self.width,
            self.height,
            self.depth,
            color,
        );
    }

    pub fn draw_solid<T: Framebuffer>(&self, renderer: &mut Renderer<T>, base_x: i32, base_y: i32, left_color: Color, right_color: Color, top_color: Color) {
        renderer.draw_block_3d_solid(
            base_x + self.x + self.z,
            base_y + self.y + self.x / 2 - self.z / 2,
            self.width,
            self.height,
            self.depth,
            left_color,
            right_color,
            top_color,
        );
    }
}
