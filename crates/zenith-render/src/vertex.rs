use crate::atlas::{GlyphAtlas, GlyphKey};
use crate::font::FontContext;
use zenith_core::cell::{CellAttrs, DEFAULT_BG};
use zenith_core::grid::Grid;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GlyphInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub tex_offset: [f32; 2],
    pub tex_size: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BgInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CursorInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

pub struct RenderOutput {
    pub bg_instances: Vec<BgInstance>,
    pub glyph_instances: Vec<GlyphInstance>,
    pub cursor: Option<CursorInstance>,
}

pub fn generate_render_data(
    grid: &Grid,
    font_ctx: &mut FontContext,
    atlas: &mut GlyphAtlas,
    cursor: (usize, usize),
    show_cursor: bool,
    _viewport_width: f32,
    _viewport_height: f32,
) -> RenderOutput {
    let cell_w = font_ctx.cell_width;
    let cell_h = font_ctx.cell_height;
    let atlas_w = atlas.width as f32;
    let atlas_h = atlas.height as f32;

    let mut bg_instances = Vec::with_capacity(grid.cols() * grid.rows());
    let mut glyph_instances = Vec::with_capacity(grid.cols() * grid.rows());

    for row in 0..grid.rows() {
        for col in 0..grid.cols() {
            let cell = grid.display_cell(col, row);
            let x = col as f32 * cell_w;
            let y = row as f32 * cell_h;

            if cell.width == 0 {
                continue;
            }

            let (fg, bg) = if cell.attrs.contains(CellAttrs::INVERSE) {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };

            if bg != DEFAULT_BG {
                bg_instances.push(BgInstance {
                    position: [x, y],
                    size: [cell_w * cell.width as f32, cell_h],
                    color: bg.to_f32_array(),
                });
            }

            if cell.c != ' ' && cell.c != '\0' {
                let key = GlyphKey {
                    c: cell.c,
                    bold: cell.attrs.contains(CellAttrs::BOLD),
                    italic: cell.attrs.contains(CellAttrs::ITALIC),
                };

                if let Some(entry) = atlas.get_or_insert(key, font_ctx) {
                    let gx = x + entry.bearing_x;
                    let gy = y + (font_ctx.baseline - entry.bearing_y);

                    glyph_instances.push(GlyphInstance {
                        position: [gx, gy],
                        size: [entry.width as f32, entry.height as f32],
                        tex_offset: [entry.x as f32 / atlas_w, entry.y as f32 / atlas_h],
                        tex_size: [entry.width as f32 / atlas_w, entry.height as f32 / atlas_h],
                        color: fg.to_f32_array(),
                    });
                }
            }
        }
    }

    let cursor_inst = if show_cursor && grid.display_offset() == 0 {
        let cx = cursor.0 as f32 * cell_w;
        let cy = cursor.1 as f32 * cell_h;
        Some(CursorInstance {
            position: [cx, cy],
            size: [cell_w, cell_h],
            color: [0.97, 0.47, 0.56, 0.8],
        })
    } else {
        None
    };

    RenderOutput {
        bg_instances,
        glyph_instances,
        cursor: cursor_inst,
    }
}
