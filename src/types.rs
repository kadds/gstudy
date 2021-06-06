#[derive(Debug, Clone)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

impl Color {
    pub fn to_argb(&self) -> u32 {
        let r = (self.r * 256f32) as u8;
        let g = (self.g * 256f32) as u8;
        let b = (self.b * 256f32) as u8;
        let a = (self.a * 256f32) as u8;
        (a as u32) << 24 + (r as u32) << 16 + (g as u32) << 8 + (b as u32)
    }
    pub fn to_abgr(&self) -> u32 {
        let r = (self.r * 256f32) as u8;
        let g = (self.g * 256f32) as u8;
        let b = (self.b * 256f32) as u8;
        let a = (self.a * 256f32) as u8;
        (a as u32) << 24 + (b as u32) << 16 + (g as u32) << 8 + (r as u32)
    }
}

pub struct Position2 {
    pub x: u32,
    pub y: u32,
}

pub struct Position3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}
#[derive(Debug, Clone, PartialEq, PartialOrd, Copy, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Copy, Default)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}
