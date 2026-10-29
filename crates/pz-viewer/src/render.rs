use crate::assets::{Assets, PageKey};
use std::{collections::HashMap, ops::Range};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance { pub pos: [f32; 2], pub size: [f32; 2], pub uv: [f32; 4], pub color: [f32; 4], pub layer: f32 }

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CamUniform { center: [f32; 2], half_view: [f32; 2] }

/// Page key of the built-in white texture (sample at uv WHITE_UV) (untextured, tinted quads).
pub const WHITE: PageKey = PageKey { pack: u16::MAX, page: 0 };
pub const WHITE_UV: [f32; 4] = [0.002; 4];

/// Consecutive instances sharing one atlas page (painter's order is preserved).
pub struct Batch { pub page: PageKey, pub range: Range<u32> }

/// Every atlas page is at most this many pixels square; pages are stored in array-texture slabs.
pub const ATLAS: u32 = 1024;
const SLAB_LAYERS: u32 = 64;

struct Slab { tex: wgpu::Texture, bind: wgpu::BindGroup, used: u32 }

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    cam_buf: wgpu::Buffer,
    cam_bg: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pages: HashMap<PageKey, (usize, u32)>,
    slabs: Vec<Slab>,
    draws: Vec<(usize, Range<u32>)>,
    inst_buf: wgpu::Buffer,
    inst_cap: u64,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprites"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let cam_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None, bind_group_layouts: &[&cam_layout, &tex_layout], push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprites"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: Some("vs"), compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4, 4 => Float32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: Some("fs"), compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None, multisample: wgpu::MultisampleState::default(), multiview: None, cache: None,
        });
        let cam_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cam"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cam_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &cam_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: cam_buf.as_entire_binding() }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default()
        });
        let inst_cap = 1 << 16;
        let inst_buf = Self::inst_buffer(&device, inst_cap);
        let mut r = Self { device, queue, format, pipeline, cam_buf, cam_bg, tex_layout, sampler, pages: HashMap::new(), slabs: Vec::new(), draws: Vec::new(), inst_buf, inst_cap };
        r.upload_page(WHITE, 4, 4, &[255; 64]);
        r
    }

    fn inst_buffer(device: &wgpu::Device, cap: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"), size: cap * std::mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        })
    }

    /// Decode + upload an atlas page the first time it is needed.
    pub fn ensure_page(&mut self, assets: &Assets, key: PageKey) {
        if self.pages.contains_key(&key) { return; }
        let (w, h, rgba) = assets.decode_page(key).expect("decode atlas page");
        self.upload_page(key, w, h, &rgba);
    }

    fn upload_page(&mut self, key: PageKey, w: u32, h: u32, rgba: &[u8]) {
        if self.slabs.last().map_or(true, |s| s.used == SLAB_LAYERS) {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("atlas slab"),
                size: wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: SLAB_LAYERS },
                mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None, layout: &self.tex_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            self.slabs.push(Slab { tex, bind, used: 0 });
        }
        let si = self.slabs.len() - 1;
        let layer = self.slabs[si].used;
        self.slabs[si].used += 1;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.slabs[si].tex, mip_level: 0, origin: wgpu::Origin3d { x: 0, y: 0, z: layer }, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.pages.insert(key, (si, layer));
    }

    /// Resolve page keys to (slab, layer): stamps each instance's layer and builds the draw list,
    /// merging neighbouring batches that live in the same slab.
    pub fn prepare(&mut self, assets: &Assets, instances: &mut [Instance], batches: &[Batch]) {
        self.draws.clear();
        for b in batches {
            self.ensure_page(assets, b.page);
            let (slab, layer) = self.pages[&b.page];
            for i in &mut instances[b.range.start as usize..b.range.end as usize] { i.layer = layer as f32; }
            match self.draws.last_mut() {
                Some((s, r)) if *s == slab => r.end = b.range.end,
                _ => self.draws.push((slab, b.range.clone())),
            }
        }
    }

    pub fn draw_calls(&self) -> usize { self.draws.len() }

    pub fn draw(&mut self, target: &wgpu::TextureView, viewport: (u32, u32), center: [f32; 2], zoom: f32,
                instances: &[Instance]) {
        if instances.len() as u64 > self.inst_cap {
            self.inst_cap = (instances.len() as u64).next_power_of_two();
            self.inst_buf = Self::inst_buffer(&self.device, self.inst_cap);
        }
        self.queue.write_buffer(&self.inst_buf, 0, bytemuck::cast_slice(instances));
        let cam = CamUniform { center, half_view: [viewport.0 as f32 / (2.0 * zoom), viewport.1 as f32 / (2.0 * zoom)] };
        self.queue.write_buffer(&self.cam_buf, 0, bytemuck::bytes_of(&cam));
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.06, b: 0.05, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.cam_bg, &[]);
            pass.set_vertex_buffer(0, self.inst_buf.slice(..));
            for (slab, range) in &self.draws {
                pass.set_bind_group(1, &self.slabs[*slab].bind, &[]);
                pass.draw(0..4, range.clone());
            }
        }
        self.queue.submit([enc.finish()]);
    }
}
