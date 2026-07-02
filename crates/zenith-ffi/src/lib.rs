use std::ffi::{c_char, CString};

use zenith_core::pty::Pty;
use zenith_core::term::Terminal;
use zenith_render::atlas::GlyphAtlas;
use zenith_render::font::FontContext;
use zenith_render::vertex::generate_render_data;

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

pub struct ZenithTerminal {
    term: Terminal,
    pty: Pty,
    font_ctx: FontContext,
    atlas: GlyphAtlas,
}

#[no_mangle]
pub extern "C" fn zn_init() {
    let _ = env_logger::try_init();
}

#[no_mangle]
pub extern "C" fn zn_terminal_new(
    cols: u32,
    rows: u32,
    font_family: *const c_char,
    font_size: f32,
) -> *mut ZenithTerminal {
    let family = if font_family.is_null() {
        "Menlo"
    } else {
        unsafe { std::ffi::CStr::from_ptr(font_family) }
            .to_str()
            .unwrap_or("Menlo")
    };

    let pty = match Pty::spawn(cols as u16, rows as u16, None) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to spawn PTY: {}", e);
            return std::ptr::null_mut();
        }
    };

    let mut font_ctx = FontContext::new(family, font_size);
    let mut atlas = GlyphAtlas::new(2048, 2048);
    atlas.warm_ascii(&mut font_ctx);

    let terminal = Box::new(ZenithTerminal {
        term: Terminal::new(cols as usize, rows as usize),
        pty,
        font_ctx,
        atlas,
    });

    Box::into_raw(terminal)
}

#[no_mangle]
pub extern "C" fn zn_terminal_destroy(term: *mut ZenithTerminal) {
    if !term.is_null() {
        unsafe {
            drop(Box::from_raw(term));
        }
    }
}

#[no_mangle]
pub extern "C" fn zn_terminal_read(term: *mut ZenithTerminal) -> bool {
    let term = unsafe { &mut *term };
    let mut buf = [0u8; 65536];
    match term.pty.read(&mut buf) {
        Ok(0) => false,
        Ok(n) => {
            term.term.feed(&buf[..n]);
            true
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn zn_terminal_write(term: *mut ZenithTerminal, data: *const u8, len: u32) {
    let term = unsafe { &mut *term };
    term.term.reset_display_offset();
    let slice = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let _ = term.pty.write_all(slice);
}

#[no_mangle]
pub extern "C" fn zn_terminal_scroll_display(term: *mut ZenithTerminal, delta: i32) {
    let term = unsafe { &mut *term };
    term.term.scroll_display(delta);
}

#[no_mangle]
pub extern "C" fn zn_terminal_resize(term: *mut ZenithTerminal, cols: u32, rows: u32) {
    let term = unsafe { &mut *term };
    term.term.resize(cols as usize, rows as usize);
    let _ = term.pty.resize(cols as u16, rows as u16);
}

#[no_mangle]
pub extern "C" fn zn_terminal_pty_fd(term: *mut ZenithTerminal) -> i32 {
    let term = unsafe { &*term };
    term.pty.fd()
}

#[repr(C)]
pub struct ZNRenderData {
    pub bg_instances: *const BgInstance,
    pub bg_count: u32,
    pub glyph_instances: *const GlyphInstance,
    pub glyph_count: u32,
    pub has_cursor: bool,
    pub cursor: CursorInstance,
    pub atlas_data: *const u8,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub atlas_dirty: bool,
    _bg_vec: *mut Vec<BgInstance>,
    _glyph_vec: *mut Vec<GlyphInstance>,
}

#[no_mangle]
pub extern "C" fn zn_terminal_render(
    term: *mut ZenithTerminal,
    viewport_width: f32,
    viewport_height: f32,
) -> *mut ZNRenderData {
    let term = unsafe { &mut *term };
    let cursor = term.term.cursor();
    let show_cursor = term.term.show_cursor();

    let output = generate_render_data(
        term.term.grid(),
        &mut term.font_ctx,
        &mut term.atlas,
        cursor,
        show_cursor,
        viewport_width,
        viewport_height,
    );

    // Safety: BgInstance/GlyphInstance/CursorInstance in FFI and render crates
    // have identical #[repr(C)] layouts
    let bg_vec: Vec<BgInstance> = unsafe { std::mem::transmute(output.bg_instances) };
    let glyph_vec: Vec<GlyphInstance> = unsafe { std::mem::transmute(output.glyph_instances) };
    let cursor_opt: Option<CursorInstance> = unsafe { std::mem::transmute(output.cursor) };

    let bg_box = Box::new(bg_vec);
    let glyph_box = Box::new(glyph_vec);

    let atlas_dirty = term.atlas.dirty;
    if atlas_dirty {
        term.atlas.dirty = false;
    }

    let render_data = Box::new(ZNRenderData {
        bg_instances: bg_box.as_ptr(),
        bg_count: bg_box.len() as u32,
        glyph_instances: glyph_box.as_ptr(),
        glyph_count: glyph_box.len() as u32,
        has_cursor: cursor_opt.is_some(),
        cursor: cursor_opt.unwrap_or(CursorInstance {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
        }),
        atlas_data: term.atlas.data.as_ptr(),
        atlas_width: term.atlas.width,
        atlas_height: term.atlas.height,
        atlas_dirty,
        _bg_vec: Box::into_raw(bg_box),
        _glyph_vec: Box::into_raw(glyph_box),
    });

    Box::into_raw(render_data)
}

#[no_mangle]
pub extern "C" fn zn_render_data_free(data: *mut ZNRenderData) {
    if !data.is_null() {
        unsafe {
            let data = Box::from_raw(data);
            drop(Box::from_raw(data._bg_vec));
            drop(Box::from_raw(data._glyph_vec));
        }
    }
}

#[no_mangle]
pub extern "C" fn zn_terminal_cell_size(
    term: *mut ZenithTerminal,
    width: *mut f32,
    height: *mut f32,
) {
    let term = unsafe { &*term };
    unsafe {
        *width = term.font_ctx.cell_width;
        *height = term.font_ctx.cell_height;
    }
}

#[no_mangle]
pub extern "C" fn zn_terminal_clear_dirty(term: *mut ZenithTerminal) {
    let term = unsafe { &mut *term };
    term.term.clear_dirty();
}

#[no_mangle]
pub extern "C" fn zn_terminal_child_exited(term: *mut ZenithTerminal) -> i32 {
    let term = unsafe { &*term };
    match term.pty.child_exited() {
        Some(code) => code,
        None => -1,
    }
}

#[repr(C)]
pub struct ZNConfig {
    pub font_size: f32,
    pub font_family: *const c_char,
    pub window_opacity: f32,
    pub scrollback_lines: u32,
    pub ai_model: *const c_char,
}

#[no_mangle]
pub extern "C" fn zn_terminal_selection_text(
    term: *mut ZenithTerminal,
    start_col: u32,
    start_row: u32,
    end_col: u32,
    end_row: u32,
) -> *mut c_char {
    if term.is_null() {
        return std::ptr::null_mut();
    }
    let term = unsafe { &*term };
    let text = term.term.grid().display_text_range(
        (start_col as usize, start_row as usize),
        (end_col as usize, end_row as usize),
    );
    if text.is_empty() {
        return std::ptr::null_mut();
    }
    CString::new(text).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn zn_terminal_screen_text(
    term: *mut ZenithTerminal,
    scrollback_lines: u32,
) -> *mut c_char {
    if term.is_null() {
        return std::ptr::null_mut();
    }
    let term = unsafe { &*term };
    let text = term.term.grid().screen_text(scrollback_lines as usize);
    if text.is_empty() {
        return std::ptr::null_mut();
    }
    CString::new(text).unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn zn_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) }
    }
}

#[no_mangle]
pub extern "C" fn zn_config_load() -> *mut ZNConfig {
    let config = zenith_config::Config::load();
    let family = CString::new(config.appearance.font_family.as_str()).unwrap();
    let ai_model = CString::new(config.ai.model.as_str())
        .unwrap_or_else(|_| CString::new("sonnet").unwrap());
    let cfg = Box::new(ZNConfig {
        font_size: config.appearance.font_size,
        font_family: family.into_raw(),
        window_opacity: config.appearance.window_opacity,
        scrollback_lines: config.terminal.scrollback_lines as u32,
        ai_model: ai_model.into_raw(),
    });
    Box::into_raw(cfg)
}

#[no_mangle]
pub extern "C" fn zn_config_free(cfg: *mut ZNConfig) {
    if !cfg.is_null() {
        unsafe {
            let cfg = Box::from_raw(cfg);
            let _ = CString::from_raw(cfg.font_family as *mut c_char);
            let _ = CString::from_raw(cfg.ai_model as *mut c_char);
        }
    }
}
