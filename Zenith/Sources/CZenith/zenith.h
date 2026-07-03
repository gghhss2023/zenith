/* Zenith FFI Header */

#ifndef ZENITH_H
#define ZENITH_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct ZenithTerminal ZenithTerminal;

typedef struct {
    float position[2];
    float size[2];
    float color[4];
} BgInstance;

typedef struct {
    float position[2];
    float size[2];
    float tex_offset[2];
    float tex_size[2];
    float color[4];
} GlyphInstance;

typedef struct {
    float position[2];
    float size[2];
    float color[4];
} CursorInstance;

typedef struct {
    const BgInstance *bg_instances;
    uint32_t bg_count;
    const GlyphInstance *glyph_instances;
    uint32_t glyph_count;
    bool has_cursor;
    CursorInstance cursor;
    const uint8_t *atlas_data;
    uint32_t atlas_width;
    uint32_t atlas_height;
    bool atlas_dirty;
    void *_bg_vec;
    void *_glyph_vec;
} ZNRenderData;

typedef struct {
    float font_size;
    const char *font_family;
    float window_opacity;
    uint32_t scrollback_lines;
    const char *ai_model;
} ZNConfig;

void zn_init(void);

ZenithTerminal *zn_terminal_new(uint32_t cols, uint32_t rows,
                                 const char *font_family, float font_size);
void zn_terminal_destroy(ZenithTerminal *term);
bool zn_terminal_read(ZenithTerminal *term);
void zn_terminal_write(ZenithTerminal *term, const uint8_t *data, uint32_t len);
void zn_terminal_resize(ZenithTerminal *term, uint32_t cols, uint32_t rows);
int32_t zn_terminal_pty_fd(ZenithTerminal *term);
ZNRenderData *zn_terminal_render(ZenithTerminal *term,
                                  float viewport_width, float viewport_height);
void zn_render_data_free(ZNRenderData *data);
void zn_terminal_cell_size(ZenithTerminal *term, float *width, float *height);
void zn_terminal_clear_dirty(ZenithTerminal *term);
int32_t zn_terminal_child_exited(ZenithTerminal *term);
void zn_terminal_scroll_display(ZenithTerminal *term, int32_t delta);
char *zn_terminal_selection_text(ZenithTerminal *term,
                                 uint32_t start_col, uint32_t start_row,
                                 uint32_t end_col, uint32_t end_row);
void zn_string_free(char *s);
char *zn_terminal_screen_text(ZenithTerminal *term, uint32_t scrollback_lines);
char *zn_terminal_accept_suggestion(ZenithTerminal *term);

ZNConfig *zn_config_load(void);
void zn_config_free(ZNConfig *cfg);

#endif /* ZENITH_H */
