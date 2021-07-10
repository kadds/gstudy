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
    pub fn to_rgba(&self) -> u32 {
        let r = (self.r * 256f32) as u8;
        let g = (self.g * 256f32) as u8;
        let b = (self.b * 256f32) as u8;
        let a = (self.a * 256f32) as u8;
        (r as u32) << 24 + (g as u32) << 16 + (b as u32) << 8 + (a as u32)
    }
    pub fn to_rgba_u8(&self) -> [u8; 4] {
        let r = (self.r * 256f32) as u8;
        let g = (self.g * 256f32) as u8;
        let b = (self.b * 256f32) as u8;
        let a = (self.a * 256f32) as u8;
        [r, g, b, a]
    }
    pub fn to_bgra(&self) -> u32 {
        let r = (self.r * 256f32) as u8;
        let g = (self.g * 256f32) as u8;
        let b = (self.b * 256f32) as u8;
        let a = (self.a * 256f32) as u8;
        (b as u32) << 24 + (g as u32) << 16 + (r as u32) << 8 + (a as u32)
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
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub fn right(&self) -> i32 {
        self.x + self.width
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }
    pub fn left(&self) -> i32 {
        self.x
    }
    pub fn top(&self) -> i32 {
        self.y
    }
}
