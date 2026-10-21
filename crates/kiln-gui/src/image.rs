//! Home's lockup: one picture drawn as a textured quad. Fenix's
//! `pdf_texture.rs` pipeline, cut to the one quad Kiln needs. The
//! picture is drawn by `kiln-brand` at exactly the pixel height its rows
//! give it, in the theme's text colour, so it is crisp at any font size
//! and follows the theme; it is drawn again only when either changes.

use crate::gpu::Gpu;
use crate::render::Rect;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}

/// The pipeline and the picture on the GPU.
pub struct Picture {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertices: wgpu::Buffer,
    texture: Option<Lockup>,
}

/// The lockup on the GPU.
struct Lockup {
    /// What it was drawn for: height and colour.
    key: (u32, [u8; 3]),
    /// Its size in pixels.
    size: (u32, u32),
    group: wgpu::BindGroup,
}

impl Picture {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Picture {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kiln-image-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("image.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kiln-image-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kiln-image-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kiln-image-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kiln-image-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kiln-image-vertices"),
            size: (6 * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Picture {
            pipeline,
            layout,
            sampler,
            vertices,
            texture: None,
        }
    }

    /// Makes sure the lockup is drawn `height` pixels tall in `color`,
    /// and returns its size in pixels.
    pub fn lockup(&mut self, gpu: &Gpu, height: u32, color: [u8; 3]) -> (u32, u32) {
        let key = (height.max(8), color);
        if let Some(l) = &self.texture {
            if l.key == key {
                return l.size;
            }
        }
        let image = kiln_brand::lockup(key.0, color);
        let size = wgpu::Extent3d {
            width: image.width.max(1),
            height: image.height.max(1),
            depth_or_array_layers: 1,
        };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kiln-image-texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // The surface isn't sRGB and colours are written as they are
            // (rect.wgsl), so the picture's bytes go through unchanged too.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        if image.width > 0 && image.height > 0 {
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &image.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width * 4),
                    rows_per_image: Some(image.height),
                },
                size,
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kiln-image-bind-group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let px = (size.width, size.height);
        self.texture = Some(Lockup {
            key,
            size: px,
            group,
        });
        px
    }

    /// Queues the quad for `rect` (physical pixels) on a `width`×`height`
    /// surface; call before the pass.
    pub fn place(&self, gpu: &Gpu, rect: Rect, width: f32, height: f32) {
        let x = |px: f32| px / width * 2.0 - 1.0;
        let y = |py: f32| 1.0 - py / height * 2.0;
        let (l, t, r, b) = (x(rect.x), y(rect.y), x(rect.x + rect.w), y(rect.y + rect.h));
        let v = |position, uv| Vertex { position, uv };
        let quad = [
            v([l, t], [0.0, 0.0]),
            v([r, t], [1.0, 0.0]),
            v([l, b], [0.0, 1.0]),
            v([r, t], [1.0, 0.0]),
            v([r, b], [1.0, 1.0]),
            v([l, b], [0.0, 1.0]),
        ];
        gpu.queue
            .write_buffer(&self.vertices, 0, bytemuck::cast_slice(&quad));
    }

    /// Draws the quad placed last.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(l) = &self.texture else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &l.group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..6, 0..1);
    }
}
