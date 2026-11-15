//! Crisp name tags: drawn with the system font at full screen resolution in
//! a tiny GPU pass on top of the pixel-art pond (which is upscaled 6x, far too
//! coarse for readable text).

use pixels::{wgpu, Pixels};

const ATLAS_W: u32 = 1024;
const ROW_H: u32 = 64;
const ROWS: u32 = 2;
const MAX_TAGS: usize = ROWS as usize;

const SHADER: &str = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@vertex fn vs_main(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>(pos, 0.0, 1.0);
    o.uv = uv;
    return o;
}
@fragment fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, i.uv);
}
"#;

/// A tag request: text, centre x and the y its bottom edge should sit at,
/// both in physical pixels of the window.
pub struct Tag {
    pub text: String,
    pub x: f32,
    pub bottom: f32,
}

struct Raster {
    text: String,
    w: u32,
    h: u32,
}

pub struct Labels {
    font: fontdue::Font,
    scale: f32,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    vbuf: wgpu::Buffer,
    rasters: Vec<Raster>,
    quads: u32,
}

fn load_font() -> Option<fontdue::Font> {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/SFNS.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ]
    } else {
        &[
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
        ]
    };
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let settings = fontdue::FontSettings {
                scale: 32.0,
                ..Default::default()
            };
            if let Ok(f) = fontdue::Font::from_bytes(bytes, settings) {
                return Some(f);
            }
        }
    }
    None
}

impl Labels {
    /// None when no usable system font is found (the pixel font is the fallback).
    pub fn new(pixels: &Pixels, scale: f64) -> Option<Labels> {
        let font = load_font()?;
        let device = pixels.device();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("duckpond_tags"),
            size: wgpu::Extent3d {
                width: ATLAS_W,
                height: ROW_H * ROWS,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("duckpond_tags_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("duckpond_tags_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("duckpond_tags_bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("duckpond_tags_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("duckpond_tags_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let vlayout = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
            ],
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("duckpond_tags_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: std::slice::from_ref(&vlayout),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: pixels.render_texture_format(),
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("duckpond_tags_vbuf"),
            size: (MAX_TAGS * 6 * 16) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(Labels {
            font,
            scale: scale as f32,
            texture,
            bind_group,
            pipeline,
            vbuf,
            rasters: vec![],
            quads: 0,
        })
    }

    pub fn set_scale(&mut self, s: f64) {
        if (self.scale - s as f32).abs() > 0.01 {
            self.scale = s as f32;
            self.rasters.clear();
        }
    }

    /// Draw `text` into atlas row `row`, like a Finder label: rounded dark pill,
    /// white system-font text. Returns its size in pixels.
    fn rasterize(&self, pixels: &Pixels, row: u32, text: &str) -> (u32, u32) {
        let s = self.scale;
        let size = 12.5 * s;
        let pad_x = (7.0 * s).round() as i32;
        let pad_y = (3.0 * s).round() as i32;
        let lm = self.font.horizontal_line_metrics(size);
        let (ascent, descent) = lm
            .map(|m| (m.ascent, m.descent))
            .unwrap_or((size * 0.8, -size * 0.2));
        let max_w = (ATLAS_W as i32 - pad_x * 2).min((300.0 * s) as i32);

        // lay out, truncating with an ellipsis if needed
        let mut chars: Vec<char> = text.chars().collect();
        let width_of = |cs: &[char]| -> f32 {
            cs.iter()
                .map(|&c| self.font.metrics(c, size).advance_width)
                .sum()
        };
        if width_of(&chars) > max_w as f32 {
            while !chars.is_empty()
                && width_of(&chars) + self.font.metrics('…', size).advance_width > max_w as f32
            {
                chars.pop();
            }
            chars.push('…');
        }
        let text_w = width_of(&chars).ceil() as i32;
        let w = (text_w + pad_x * 2).min(ATLAS_W as i32) as u32;
        let h = ((ascent - descent).ceil() as i32 + pad_y * 2).min(ROW_H as i32) as u32;

        let mut buf = vec![0u8; (w * h * 4) as usize];
        // rounded pill background (premultiplied)
        let r = h as f32 / 2.0;
        let bg = [34.0, 26.0, 58.0];
        let bg_a = 0.86;
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let cx = fx.clamp(r, w as f32 - r);
                let d = ((fx - cx).powi(2) + (fy - r).powi(2)).sqrt();
                let cov = (r - d + 0.5).clamp(0.0, 1.0) * bg_a;
                let i = ((y * w + x) * 4) as usize;
                buf[i] = (bg[0] * cov) as u8;
                buf[i + 1] = (bg[1] * cov) as u8;
                buf[i + 2] = (bg[2] * cov) as u8;
                buf[i + 3] = (255.0 * cov) as u8;
            }
        }
        // glyphs, white over the pill
        let baseline = pad_y as f32 + ascent;
        let mut pen = pad_x as f32;
        for &c in &chars {
            let (m, cov) = self.font.rasterize(c, size);
            let gx = (pen + m.xmin as f32).round() as i32;
            let gy = (baseline - m.height as f32 - m.ymin as f32).round() as i32;
            for yy in 0..m.height as i32 {
                for xx in 0..m.width as i32 {
                    let (px, py) = (gx + xx, gy + yy);
                    if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                        continue;
                    }
                    let a = cov[(yy * m.width as i32 + xx) as usize] as f32 / 255.0;
                    if a <= 0.0 {
                        continue;
                    }
                    let i = ((py as u32 * w + px as u32) * 4) as usize;
                    for k in 0..3 {
                        buf[i + k] = (255.0 * a + buf[i + k] as f32 * (1.0 - a)) as u8;
                    }
                    buf[i + 3] = (255.0 * a + buf[i + 3] as f32 * (1.0 - a)) as u8;
                }
            }
            pen += m.advance_width;
        }
        // clear the row first so a shorter tag doesn't show the old one's tail
        let clear = vec![0u8; (ATLAS_W * ROW_H * 4) as usize];
        let q = pixels.queue();
        q.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: row * ROW_H,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &clear,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS_W * 4),
                rows_per_image: Some(ROW_H),
            },
            wgpu::Extent3d {
                width: ATLAS_W,
                height: ROW_H,
                depth_or_array_layers: 1,
            },
        );
        q.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: row * ROW_H,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &buf,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        (w, h)
    }

    /// Update the tags shown this frame. `surface` is the window size in
    /// physical pixels.
    pub fn prepare(&mut self, pixels: &Pixels, tags: &[Tag], surface: (u32, u32)) {
        let tags = &tags[..tags.len().min(MAX_TAGS)];
        for (row, t) in tags.iter().enumerate() {
            let stale = self
                .rasters
                .get(row)
                .map(|r| r.text != t.text)
                .unwrap_or(true);
            if stale {
                let (w, h) = self.rasterize(pixels, row as u32, &t.text);
                let r = Raster {
                    text: t.text.clone(),
                    w,
                    h,
                };
                if row < self.rasters.len() {
                    self.rasters[row] = r;
                } else {
                    self.rasters.push(r);
                }
            }
        }
        let (sw, sh) = (surface.0.max(1) as f32, surface.1.max(1) as f32);
        let mut verts: Vec<f32> = Vec::with_capacity(tags.len() * 24);
        for (row, t) in tags.iter().enumerate() {
            let r = &self.rasters[row];
            let (w, h) = (r.w as f32, r.h as f32);
            let x0 = (t.x - w / 2.0).round().clamp(2.0, (sw - w - 2.0).max(2.0));
            let mut y0 = (t.bottom - h).round();
            if y0 < 2.0 {
                y0 = 2.0;
            }
            let (nx0, nx1) = (x0 / sw * 2.0 - 1.0, (x0 + w) / sw * 2.0 - 1.0);
            let (ny0, ny1) = (1.0 - y0 / sh * 2.0, 1.0 - (y0 + h) / sh * 2.0);
            let (u0, u1) = (0.0, w / ATLAS_W as f32);
            let atlas_h = (ROW_H * ROWS) as f32;
            let (v0, v1) = (
                (row as u32 * ROW_H) as f32 / atlas_h,
                (row as u32 * ROW_H) as f32 / atlas_h + h / atlas_h,
            );
            for (x, y, u, v) in [
                (nx0, ny0, u0, v0),
                (nx1, ny0, u1, v0),
                (nx0, ny1, u0, v1),
                (nx1, ny0, u1, v0),
                (nx1, ny1, u1, v1),
                (nx0, ny1, u0, v1),
            ] {
                verts.extend_from_slice(&[x, y, u, v]);
            }
        }
        self.quads = tags.len() as u32;
        if !verts.is_empty() {
            let bytes: Vec<u8> = verts.iter().flat_map(|f| f.to_ne_bytes()).collect();
            pixels.queue().write_buffer(&self.vbuf, 0, &bytes);
        }
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        if self.quads == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("duckpond_tags_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        pass.draw(0..self.quads * 6, 0..1);
    }
}
