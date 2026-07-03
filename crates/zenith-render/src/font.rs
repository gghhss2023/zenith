use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache,
};

pub struct FontContext {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub cell_width: f32,
    pub cell_height: f32,
    pub baseline: f32,
    pub font_size: f32,
    pub font_family: String,
}

impl FontContext {
    pub fn new(font_family: &str, font_size: f32) -> Self {
        let mut font_system = FontSystem::new();

        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        buffer.set_size(
            &mut font_system,
            Some(font_size * 10.0),
            Some(font_size * 2.0),
        );

        let attrs = Attrs::new().family(Family::Name(font_family));
        buffer.set_text(&mut font_system, "M", attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut font_system, false);

        let mut cell_width = font_size * 0.6;
        let cell_height = metrics.line_height;

        if let Some(run) = buffer.layout_runs().next() {
            if let Some(glyph) = run.glyphs.iter().next() {
                cell_width = glyph.w;
            }
        }

        Self {
            font_system,
            swash_cache: SwashCache::new(),
            cell_width,
            cell_height,
            baseline: font_size,
            font_size,
            font_family: font_family.to_string(),
        }
    }

    pub fn rasterize_glyph(
        &mut self,
        c: char,
        bold: bool,
        italic: bool,
    ) -> Option<RasterizedGlyph> {
        let metrics = Metrics::new(self.font_size, self.cell_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(
            &mut self.font_system,
            Some(self.cell_width * 4.0),
            Some(self.cell_height * 2.0),
        );

        let mut attrs = Attrs::new().family(Family::Name(&self.font_family));
        if bold {
            attrs = attrs.weight(cosmic_text::Weight::BOLD);
        }
        if italic {
            attrs = attrs.style(cosmic_text::Style::Italic);
        }

        let s = c.to_string();
        buffer.set_text(&mut self.font_system, &s, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                if let Some(image) = self
                    .swash_cache
                    .get_image(&mut self.font_system, physical.cache_key)
                {
                    let width = image.placement.width;
                    let height = image.placement.height;
                    if width == 0 || height == 0 {
                        return None;
                    }

                    let rgba = match image.content {
                        cosmic_text::SwashContent::Mask => {
                            let mut rgba = vec![0u8; (width * height * 4) as usize];
                            for (i, &alpha) in image.data.iter().enumerate() {
                                rgba[i * 4] = 255;
                                rgba[i * 4 + 1] = 255;
                                rgba[i * 4 + 2] = 255;
                                rgba[i * 4 + 3] = alpha;
                            }
                            rgba
                        }
                        cosmic_text::SwashContent::Color => image.data.clone(),
                        cosmic_text::SwashContent::SubpixelMask => {
                            let mut rgba = vec![0u8; (width * height * 4) as usize];
                            for i in 0..(width * height) as usize {
                                let idx = i * 3;
                                let avg = if idx + 2 < image.data.len() {
                                    ((image.data[idx] as u16
                                        + image.data[idx + 1] as u16
                                        + image.data[idx + 2] as u16)
                                        / 3) as u8
                                } else {
                                    0
                                };
                                rgba[i * 4] = 255;
                                rgba[i * 4 + 1] = 255;
                                rgba[i * 4 + 2] = 255;
                                rgba[i * 4 + 3] = avg;
                            }
                            rgba
                        }
                    };

                    return Some(RasterizedGlyph {
                        data: rgba,
                        width,
                        height,
                        bearing_x: image.placement.left as f32,
                        bearing_y: image.placement.top as f32,
                    });
                }
            }
        }
        None
    }
}

pub struct RasterizedGlyph {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}
