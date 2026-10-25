//! Process-wide wgpu device shared by every effect and render thread, plus a texture pool so
//! repeated frames of the same size do not reallocate.

use std::sync::{LazyLock, Mutex};

use wgpu::util::DeviceExt;

use crate::Error;

/// Fullscreen-triangle vertex stage, prepended to effect shaders. Provides `VsOut { pos, uv }`
/// with `uv` (0,0) at the top-left texel corner.
pub const FULLSCREEN_WGSL: &str = include_str!("shaders/fullscreen.wgsl");

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    /// Bilinear, clamp-to-edge.
    pub linear_clamp: wgpu::Sampler,
    pool: Mutex<Vec<wgpu::Texture>>,
    staging: Mutex<Vec<wgpu::Buffer>>,
    pub(crate) io: crate::io::IoPipelines,
}

const POOL_CAP: usize = 64;

static GPU: LazyLock<Result<Gpu, String>> = LazyLock::new(Gpu::new);

/// The shared device, created on first use.
pub fn gpu() -> Result<&'static Gpu, Error> {
    GPU.as_ref().map_err(|e| Error::Gpu(e.clone()))
}

impl Gpu {
    fn new() -> Result<Self, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        // DX12 with the debug layer on would tear down the host's own D3D12 device (AE renders on
        // DX12). Validation stays available through the Vulkan backend.
        if desc.flags.contains(wgpu::InstanceFlags::VALIDATION) {
            desc.backends.remove(wgpu::Backends::DX12);
        }
        let instance = wgpu::Instance::new(desc);
        let power_preference = wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::HighPerformance);
        let adapter = pollster::block_on(
            instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference, ..Default::default() }),
        )
        .map_err(|e| format!("no GPU adapter: {e}"))?;

        // Effects sample the f32 canvas with bilinear filtering.
        let required_features = wgpu::Features::FLOAT32_FILTERABLE;
        if !adapter.features().contains(required_features) {
            return Err(format!("adapter {} lacks {required_features:?}", adapter.get_info().name));
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("probable"),
            required_features,
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
        }))
        .map_err(|e| format!("request_device: {e}"))?;

        let info = adapter.get_info();
        log::info!("probable: GPU {} ({:?})", info.name, info.backend);

        let linear_clamp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear_clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let io = crate::io::IoPipelines::new(&device);
        Ok(Self {
            device,
            queue,
            info,
            linear_clamp,
            pool: Mutex::new(Vec::new()),
            staging: Mutex::new(Vec::new()),
            io,
        })
    }

    /// A MAP_READ | COPY_DST buffer of exactly `size` bytes, pooled like [`Self::texture`].
    pub(crate) fn staging(&self, size: u64) -> PooledBuffer<'_> {
        let mut pool = self.staging.lock().unwrap();
        let buffer = match pool.iter().position(|b| b.size() == size) {
            Some(i) => pool.swap_remove(i),
            None => {
                drop(pool);
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("staging"),
                    size,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            }
        };
        PooledBuffer { gpu: self, buffer: Some(buffer) }
    }

    /// A texture from the pool (or a new one). It returns to the pool when dropped, which must
    /// happen after the commands using it were submitted (queue order makes reuse safe).
    pub fn texture(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> PooledTexture<'_> {
        let mut pool = self.pool.lock().unwrap();
        let hit = pool
            .iter()
            .position(|t| t.width() == width && t.height() == height && t.format() == format && t.usage() == usage);
        let texture = match hit {
            Some(i) => pool.swap_remove(i),
            None => {
                drop(pool);
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            }
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        PooledTexture { gpu: self, texture: Some(texture), view }
    }

    pub fn uniform<T: bytemuck::Pod>(&self, value: &T) -> wgpu::Buffer {
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(value),
            usage: wgpu::BufferUsages::UNIFORM,
        })
    }

    /// A WGSL module made of [`FULLSCREEN_WGSL`] followed by `source`.
    pub fn fullscreen_module(&self, label: &str, source: &str) -> wgpu::ShaderModule {
        let code = format!("{FULLSCREEN_WGSL}\n{source}");
        self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(code.into()),
        })
    }

    /// Fullscreen render pipeline with an auto layout (group 0 holds the entry point's bindings).
    pub fn fullscreen_pipeline(
        &self,
        module: &wgpu::ShaderModule,
        fragment_entry: &str,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fragment_entry),
            layout: None,
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_fullscreen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fragment_entry),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Bind `resources` to bindings 0.. of `pipeline`'s group 0.
    pub fn bind(&self, pipeline: &wgpu::RenderPipeline, resources: &[wgpu::BindingResource<'_>]) -> wgpu::BindGroup {
        let entries: Vec<wgpu::BindGroupEntry> = resources
            .iter()
            .enumerate()
            .map(|(i, r)| wgpu::BindGroupEntry { binding: i as u32, resource: r.clone() })
            .collect();
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        })
    }

    /// Draw the fullscreen triangle into `target`, overwriting it.
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        target: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub struct PooledTexture<'g> {
    gpu: &'g Gpu,
    texture: Option<wgpu::Texture>,
    pub view: wgpu::TextureView,
}

impl PooledTexture<'_> {
    pub fn texture(&self) -> &wgpu::Texture {
        self.texture.as_ref().unwrap()
    }

    pub fn size(&self) -> [u32; 2] {
        let t = self.texture();
        [t.width(), t.height()]
    }
}

impl Drop for PooledTexture<'_> {
    fn drop(&mut self) {
        let texture = self.texture.take().unwrap();
        let mut pool = self.gpu.pool.lock().unwrap();
        if pool.len() >= POOL_CAP {
            pool.remove(0);
        }
        pool.push(texture);
    }
}

pub(crate) struct PooledBuffer<'g> {
    gpu: &'g Gpu,
    buffer: Option<wgpu::Buffer>,
}

impl PooledBuffer<'_> {
    pub(crate) fn buffer(&self) -> &wgpu::Buffer {
        self.buffer.as_ref().unwrap()
    }
}

impl Drop for PooledBuffer<'_> {
    fn drop(&mut self) {
        let buffer = self.buffer.take().unwrap();
        let mut pool = self.gpu.staging.lock().unwrap();
        if pool.len() >= POOL_CAP / 4 {
            pool.remove(0);
        }
        pool.push(buffer);
    }
}
