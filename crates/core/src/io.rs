//! Compute passes converting host pixels to the linear canvas and back (`shaders/io.wgsl`).

use crate::color::ColorSpace;
use crate::image::{Depth, ImageDesc};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct IoUniform {
    offset: [i32; 2],
    size: [u32; 2],
    order: u32,
    int_scale: f32,
    transfer: u32,
    bottom_up: u32,
    straight: u32,
    _pad: [u32; 3],
    m: [[f32; 4]; 3],
}

pub(crate) struct IoPipelines {
    unpack_float: wgpu::ComputePipeline,
    unpack_uint: wgpu::ComputePipeline,
    pack_f32: wgpu::ComputePipeline,
    pack_u8: wgpu::ComputePipeline,
    pack_u16: wgpu::ComputePipeline,
}

const WG: u32 = 16;

impl IoPipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("io"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/io.wgsl").into()),
        });
        let make = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Self {
            unpack_float: make("unpack_float"),
            unpack_uint: make("unpack_uint"),
            pack_f32: make("pack_f32"),
            pack_u8: make("pack_u8"),
            pack_u16: make("pack_u16"),
        }
    }

    /// Host texture → canvas. `offset` = host origin − canvas origin.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn unpack(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        host: &ImageDesc,
        host_view: &wgpu::TextureView,
        canvas: &wgpu::TextureView,
        canvas_size: [u32; 2],
        offset: [i32; 2],
        space: ColorSpace,
    ) {
        let pipeline = match host.layout.depth {
            Depth::U8 | Depth::F32 => &self.unpack_float,
            Depth::U16Ae | Depth::U16 => &self.unpack_uint,
        };
        let host_binding = match host.layout.depth {
            Depth::U8 | Depth::F32 => 1,
            Depth::U16Ae | Depth::U16 => 2,
        };
        let uniform = uniform(device, host, canvas_size, offset, space.transfer(), space.to_rec709());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("unpack"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: host_binding, resource: wgpu::BindingResource::TextureView(host_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(canvas) },
            ],
        });
        dispatch(encoder, pipeline, &bind, canvas_size);
    }

    /// Canvas → host-format texture. `offset` = host origin − canvas origin.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn pack(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        host: &ImageDesc,
        host_view: &wgpu::TextureView,
        canvas: &wgpu::TextureView,
        offset: [i32; 2],
        space: ColorSpace,
    ) {
        let (pipeline, binding) = match host.layout.depth {
            Depth::F32 => (&self.pack_f32, 5),
            Depth::U8 => (&self.pack_u8, 6),
            Depth::U16Ae | Depth::U16 => (&self.pack_u16, 7),
        };
        let size = [host.width, host.height];
        let uniform = uniform(device, host, size, offset, space.transfer(), space.from_rec709());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pack"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(canvas) },
                wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(host_view) },
            ],
        });
        dispatch(encoder, pipeline, &bind, size);
    }
}

fn uniform(
    device: &wgpu::Device,
    host: &ImageDesc,
    size: [u32; 2],
    offset: [i32; 2],
    transfer: u32,
    m: [[f32; 3]; 3],
) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let u = IoUniform {
        offset,
        size,
        order: host.layout.order as u32,
        int_scale: host.layout.depth.int_scale(),
        transfer,
        bottom_up: host.bottom_up as u32,
        straight: host.straight_alpha as u32,
        _pad: [0; 3],
        m: m.map(|r| [r[0], r[1], r[2], 0.0]),
    };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("io"),
        contents: bytemuck::bytes_of(&u),
        usage: wgpu::BufferUsages::UNIFORM,
    })
}

fn dispatch(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    bind: &wgpu::BindGroup,
    size: [u32; 2],
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind, &[]);
    pass.dispatch_workgroups(size[0].div_ceil(WG), size[1].div_ceil(WG), 1);
}
