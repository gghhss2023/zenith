use bitflags::bitflags;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_f32_array(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            1.0,
        ]
    }
}

pub const DEFAULT_FG: Color = Color::rgb(200, 200, 200);
pub const DEFAULT_BG: Color = Color::rgb(26, 27, 38);

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Default)]
    pub struct CellAttrs: u16 {
        const BOLD          = 0b0000_0001;
        const ITALIC        = 0b0000_0010;
        const UNDERLINE     = 0b0000_0100;
        const INVERSE       = 0b0000_1000;
        const DIM           = 0b0001_0000;
        const HIDDEN        = 0b0010_0000;
        const STRIKETHROUGH = 0b0100_0000;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttrs,
    pub width: u8,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            attrs: CellAttrs::empty(),
            width: 1,
        }
    }
}

pub const ANSI_COLORS: [Color; 16] = [
    Color::rgb(0x15, 0x16, 0x1e),
    Color::rgb(0xf7, 0x76, 0x8e),
    Color::rgb(0x9e, 0xce, 0x6a),
    Color::rgb(0xe0, 0xaf, 0x68),
    Color::rgb(0x7a, 0xa2, 0xf7),
    Color::rgb(0xbb, 0x9a, 0xf7),
    Color::rgb(0x7d, 0xcf, 0xff),
    Color::rgb(0xa9, 0xb1, 0xd6),
    Color::rgb(0x41, 0x4d, 0x68),
    Color::rgb(0xf7, 0x76, 0x8e),
    Color::rgb(0x9e, 0xce, 0x6a),
    Color::rgb(0xe0, 0xaf, 0x68),
    Color::rgb(0x7a, 0xa2, 0xf7),
    Color::rgb(0xbb, 0x9a, 0xf7),
    Color::rgb(0x7d, 0xcf, 0xff),
    Color::rgb(0xc0, 0xca, 0xf5),
];
