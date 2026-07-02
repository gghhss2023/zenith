use crate::font::{FontContext, RasterizedGlyph};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct GlyphKey {
    pub c: char,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}

pub struct GlyphAtlas {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    entries: HashMap<GlyphKey, AtlasEntry>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    pub dirty: bool,
}

impl GlyphAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 4) as usize],
            width,
            height,
            entries: HashMap::new(),
            cursor_x: 1,
            cursor_y: 1,
            row_height: 0,
            dirty: true,
        }
    }

    pub fn get_or_insert(
        &mut self,
        key: GlyphKey,
        font_ctx: &mut FontContext,
    ) -> Option<AtlasEntry> {
        if let Some(&entry) = self.entries.get(&key) {
            return Some(entry);
        }

        let glyph = font_ctx.rasterize_glyph(key.c, key.bold, key.italic)?;
        self.insert(key, &glyph)
    }

    fn insert(&mut self, key: GlyphKey, glyph: &RasterizedGlyph) -> Option<AtlasEntry> {
        if glyph.width == 0 || glyph.height == 0 {
            return None;
        }

        if self.cursor_x + glyph.width + 1 > self.width {
            self.cursor_x = 1;
            self.cursor_y += self.row_height + 1;
            self.row_height = 0;
        }

        if self.cursor_y + glyph.height + 1 > self.height {
            return None;
        }

        for row in 0..glyph.height {
            let src_offset = (row * glyph.width * 4) as usize;
            let dst_offset =
                ((self.cursor_y + row) * self.width * 4 + self.cursor_x * 4) as usize;
            let len = (glyph.width * 4) as usize;
            if src_offset + len <= glyph.data.len() && dst_offset + len <= self.data.len() {
                self.data[dst_offset..dst_offset + len]
                    .copy_from_slice(&glyph.data[src_offset..src_offset + len]);
            }
        }

        let entry = AtlasEntry {
            x: self.cursor_x,
            y: self.cursor_y,
            width: glyph.width,
            height: glyph.height,
            bearing_x: glyph.bearing_x,
            bearing_y: glyph.bearing_y,
        };

        self.entries.insert(key, entry);
        self.cursor_x += glyph.width + 1;
        self.row_height = self.row_height.max(glyph.height);
        self.dirty = true;

        Some(entry)
    }

    pub fn warm_ascii(&mut self, font_ctx: &mut FontContext) {
        for c in ' '..='~' {
            let key = GlyphKey {
                c,
                bold: false,
                italic: false,
            };
            self.get_or_insert(key, font_ctx);
        }
    }
}
