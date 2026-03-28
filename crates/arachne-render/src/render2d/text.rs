use std::collections::HashMap;
use arachne_math::{Color, Vec2, Rect};
use crate::buffer::DynamicBuffer;

// ---------------------------------------------------------------------------
// BMFont data structures
// ---------------------------------------------------------------------------

/// Metrics for a single glyph in a BMFont.
#[derive(Clone, Debug)]
pub struct GlyphMetrics {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub xoffset: f32,
    pub yoffset: f32,
    pub xadvance: f32,
    pub page: u32,
}

/// A parsed BMFont with glyph metrics and kerning.
#[derive(Clone)]
pub struct BmFont {
    pub glyphs: HashMap<u32, GlyphMetrics>,
    pub kerning: HashMap<(u32, u32), f32>,
    pub line_height: f32,
    pub base: f32,
    pub scale_w: f32,
    pub scale_h: f32,
}

impl BmFont {
    /// Parse a BMFont .fnt file (text format).
    pub fn from_fnt(data: &str) -> Self {
        let mut glyphs = HashMap::new();
        let mut kerning = HashMap::new();
        let mut line_height = 0.0f32;
        let mut base = 0.0f32;
        let mut scale_w = 1.0f32;
        let mut scale_h = 1.0f32;

        for line in data.lines() {
            let line = line.trim();

            if line.starts_with("common ") {
                line_height = parse_field(line, "lineHeight").unwrap_or(0.0);
                base = parse_field(line, "base").unwrap_or(0.0);
                scale_w = parse_field(line, "scaleW").unwrap_or(1.0);
                scale_h = parse_field(line, "scaleH").unwrap_or(1.0);
            } else if line.starts_with("char ") {
                let id: u32 = parse_field(line, "id").unwrap_or(0.0) as u32;
                let glyph = GlyphMetrics {
                    id,
                    x: parse_field(line, "x").unwrap_or(0.0),
                    y: parse_field(line, "y").unwrap_or(0.0),
                    width: parse_field(line, "width").unwrap_or(0.0),
                    height: parse_field(line, "height").unwrap_or(0.0),
                    xoffset: parse_field(line, "xoffset").unwrap_or(0.0),
                    yoffset: parse_field(line, "yoffset").unwrap_or(0.0),
                    xadvance: parse_field(line, "xadvance").unwrap_or(0.0),
                    page: parse_field(line, "page").unwrap_or(0.0) as u32,
                };
                glyphs.insert(id, glyph);
            } else if line.starts_with("kerning ") {
                let first: u32 = parse_field(line, "first").unwrap_or(0.0) as u32;
                let second: u32 = parse_field(line, "second").unwrap_or(0.0) as u32;
                let amount: f32 = parse_field(line, "amount").unwrap_or(0.0);
                kerning.insert((first, second), amount);
            }
        }

        Self {
            glyphs,
            kerning,
            line_height,
            base,
            scale_w,
            scale_h,
        }
    }

    /// Layout text, returning positioned glyph quads.
    pub fn layout_text(
        &self,
        text: &str,
        font_size: f32,
        max_width: Option<f32>,
    ) -> Vec<GlyphQuad> {
        let scale = font_size / self.line_height;
        let mut quads = Vec::new();
        let mut cursor_x = 0.0f32;
        let mut cursor_y = 0.0f32;
        let mut prev_char: Option<u32> = None;

        for ch in text.chars() {
            let id = ch as u32;

            if ch == '\n' {
                cursor_x = 0.0;
                cursor_y += self.line_height * scale;
                prev_char = None;
                continue;
            }

            let glyph = match self.glyphs.get(&id) {
                Some(g) => g,
                None => continue,
            };

            // Apply kerning
            if let Some(prev) = prev_char {
                if let Some(&kern) = self.kerning.get(&(prev, id)) {
                    cursor_x += kern * scale;
                }
            }

            let advance = glyph.xadvance * scale;

            // Word wrap
            if let Some(max_w) = max_width {
                if cursor_x + advance > max_w && cursor_x > 0.0 {
                    cursor_x = 0.0;
                    cursor_y += self.line_height * scale;
                }
            }

            // Glyph quad position
            let x = cursor_x + glyph.xoffset * scale;
            let y = cursor_y + glyph.yoffset * scale;
            let w = glyph.width * scale;
            let h = glyph.height * scale;

            // UV coordinates (normalized)
            let uv = Rect::new(
                Vec2::new(glyph.x / self.scale_w, glyph.y / self.scale_h),
                Vec2::new(
                    (glyph.x + glyph.width) / self.scale_w,
                    (glyph.y + glyph.height) / self.scale_h,
                ),
            );

            quads.push(GlyphQuad {
                position: Vec2::new(x, y),
                size: Vec2::new(w, h),
                uv,
                char_id: id,
            });

            cursor_x += advance;
            prev_char = Some(id);
        }

        quads
    }
}

/// A positioned glyph quad ready for rendering.
#[derive(Clone, Debug)]
pub struct GlyphQuad {
    pub position: Vec2,
    pub size: Vec2,
    pub uv: Rect,
    pub char_id: u32,
}

fn parse_field(line: &str, field: &str) -> Option<f32> {
    let pattern = format!("{}=", field);
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// ---------------------------------------------------------------------------
// Text vertex and renderer
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl TextVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}

/// GPU-side text rendering parameters.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextParams {
    pub edge_softness: f32,
    pub outline_width: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub outline_color: [f32; 4],
}

impl Default for TextParams {
    fn default() -> Self {
        Self {
            edge_softness: 0.1,
            outline_width: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
            outline_color: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Generates textured quads per glyph, batches by font texture.
pub struct TextRenderer {
    vertices: Vec<TextVertex>,
    indices: Vec<u32>,
    vertex_buffer: DynamicBuffer,
    index_buffer: DynamicBuffer,
}

impl TextRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            vertex_buffer: DynamicBuffer::new(device, 4096, wgpu::BufferUsages::VERTEX),
            index_buffer: DynamicBuffer::new(device, 2048, wgpu::BufferUsages::INDEX),
        }
    }

    pub fn begin_frame(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.vertex_buffer.clear();
        self.index_buffer.clear();
    }

    /// Add text to the render queue.
    pub fn draw_text(
        &mut self,
        font: &BmFont,
        text: &str,
        position: Vec2,
        font_size: f32,
        color: Color,
        max_width: Option<f32>,
    ) {
        let quads = font.layout_text(text, font_size, max_width);
        let c = [color.r, color.g, color.b, color.a];

        for quad in &quads {
            let base = self.vertices.len() as u32;
            let x0 = position.x + quad.position.x;
            let y0 = position.y + quad.position.y;
            let x1 = x0 + quad.size.x;
            let y1 = y0 + quad.size.y;

            let u0 = quad.uv.min.x;
            let v0 = quad.uv.min.y;
            let u1 = quad.uv.max.x;
            let v1 = quad.uv.max.y;

            self.vertices.push(TextVertex { position: [x0, y0], uv: [u0, v0], color: c });
            self.vertices.push(TextVertex { position: [x1, y0], uv: [u1, v0], color: c });
            self.vertices.push(TextVertex { position: [x1, y1], uv: [u1, v1], color: c });
            self.vertices.push(TextVertex { position: [x0, y1], uv: [u0, v1], color: c });

            self.indices.extend_from_slice(&[
                base, base + 1, base + 2,
                base, base + 2, base + 3,
            ]);
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> TextPrepared {
        if self.vertices.is_empty() {
            return TextPrepared {
                vertex_count: 0,
                index_count: 0,
            };
        }

        self.vertex_buffer.clear();
        self.index_buffer.clear();
        self.vertex_buffer.write(device, queue, &self.vertices);
        self.index_buffer.write(device, queue, &self.indices);

        TextPrepared {
            vertex_count: self.vertices.len() as u32,
            index_count: self.indices.len() as u32,
        }
    }

    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        prepared: &TextPrepared,
        pipeline: &'a wgpu::RenderPipeline,
        camera_bind_group: &'a wgpu::BindGroup,
        font_bind_group: &'a wgpu::BindGroup,
    ) {
        if prepared.index_count == 0 {
            return;
        }

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, Some(camera_bind_group), &[]);
        pass.set_bind_group(1, Some(font_bind_group), &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice(..));
        pass.set_index_buffer(
            self.index_buffer.buffer().slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..prepared.index_count, 0, 0..1);
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        self.vertex_buffer.buffer()
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        self.index_buffer.buffer()
    }

    pub fn glyph_count(&self) -> usize {
        self.vertices.len() / 4
    }
}

pub struct TextPrepared {
    pub vertex_count: u32,
    pub index_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fnt() -> &'static str {
        r#"info face="TestFont" size=32 bold=0 italic=0 charset="" unicode=1 stretchH=100 smooth=1 aa=1 padding=0,0,0,0 spacing=1,1 outline=0
common lineHeight=40 base=32 scaleW=256 scaleH=256 pages=1 packed=0 alphaChnl=0 redChnl=4 greenChnl=4 blueChnl=4
page id=0 file="test.png"
chars count=5
char id=72  x=0   y=0   width=20  height=30  xoffset=1  yoffset=2  xadvance=22  page=0  chnl=15
char id=101 x=21  y=0   width=16  height=22  xoffset=1  yoffset=10 xadvance=18  page=0  chnl=15
char id=108 x=38  y=0   width=8   height=30  xoffset=2  yoffset=2  xadvance=10  page=0  chnl=15
char id=111 x=47  y=0   width=16  height=22  xoffset=1  yoffset=10 xadvance=18  page=0  chnl=15
char id=32  x=0   y=0   width=0   height=0   xoffset=0  yoffset=0  xadvance=10  page=0  chnl=15
kerning first=72 second=101 amount=-1
"#
    }

    #[test]
    fn parse_bmfont() {
        let font = BmFont::from_fnt(sample_fnt());
        assert_eq!(font.line_height, 40.0);
        assert_eq!(font.base, 32.0);
        assert_eq!(font.scale_w, 256.0);
        assert_eq!(font.scale_h, 256.0);
        assert_eq!(font.glyphs.len(), 5);
        assert!(font.glyphs.contains_key(&72)); // 'H'
        assert!(font.glyphs.contains_key(&101)); // 'e'
    }

    #[test]
    fn layout_hello_glyph_positions() {
        let font = BmFont::from_fnt(sample_fnt());
        let quads = font.layout_text("Hello", 40.0, None);

        // H=72, e=101, l=108, l=108, o=111
        // At scale = 40/40 = 1.0
        assert_eq!(quads.len(), 5, "Hello has 5 glyphs");

        // Verify glyph char ids
        assert_eq!(quads[0].char_id, 72);  // H
        assert_eq!(quads[1].char_id, 101); // e
        assert_eq!(quads[2].char_id, 108); // l
        assert_eq!(quads[3].char_id, 108); // l
        assert_eq!(quads[4].char_id, 111); // o

        // H: xoffset=1, xadvance=22
        // Kerning H->e = -1
        // e: starts at cursor=22 + kern(-1) = 21, xoffset=1 -> x=22
        let h_x = quads[0].position.x;
        let e_x = quads[1].position.x;
        assert!((h_x - 1.0).abs() < 0.01, "H x-pos: {}", h_x);
        assert!((e_x - 22.0).abs() < 0.01, "e x-pos: {}", e_x);

        // Check that positions increase monotonically (no overlap in x)
        for i in 1..quads.len() {
            assert!(
                quads[i].position.x > quads[i - 1].position.x,
                "glyph {} should be to the right of glyph {}",
                i,
                i - 1
            );
        }
    }

    #[test]
    fn layout_word_wrap() {
        let font = BmFont::from_fnt(sample_fnt());
        let quads = font.layout_text("Hello Hello", 40.0, Some(100.0));

        // "Hello" width ≈ 22+18+10+10+18 = 78, space = 10, total for "Hello " = 88
        // Second "Hello" starts at x=88 but 88+22 > 100 so it should wrap
        let wrapped = quads.iter().any(|q| q.position.y > 0.0);
        assert!(wrapped, "text should wrap at max_width=100");
    }

    #[test]
    fn layout_empty_string() {
        let font = BmFont::from_fnt(sample_fnt());
        let quads = font.layout_text("", 40.0, None);
        assert!(quads.is_empty());
    }

    #[test]
    fn kerning_applied() {
        let font = BmFont::from_fnt(sample_fnt());
        assert_eq!(font.kerning.get(&(72, 101)), Some(&-1.0));
    }
}
