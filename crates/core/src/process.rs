//! Native host path: host buffer → GPU → effect → host buffer, blocking until the pixels are back.

use std::sync::OnceLock;

use crate::Error;
use crate::effect::{Effect, Frame, RenderCx};
use crate::gpu::{Gpu, gpu};
use crate::image::{HostImage, HostImageMut, ImageDesc};
use crate::params::Values;

/// Renders one effect type. Keep one per effect per process (hosts store it in their global data).
pub struct Processor<E: Effect> {
    effect: OnceLock<E>,
}

impl<E: Effect> Default for Processor<E> {
    fn default() -> Self {
        Self { effect: OnceLock::new() }
    }
}

impl<E: Effect> Processor<E> {
    /// Render `src` into `dst`. Both are placed by their `origin` in one coordinate space; the
    /// effect runs on the union of both rectangles, so pixels outside `dst` still contribute.
    pub fn render(
        &self,
        src: &HostImage<'_>,
        dst: &mut HostImageMut<'_>,
        frame: &Frame,
        values: &Values,
    ) -> Result<(), Error> {
        src.desc.validate(src.data.len())?;
        dst.desc.validate(dst.data.len())?;
        let gpu = gpu()?;
        let effect = self.effect.get_or_init(|| E::new(gpu));

        let s = &src.desc;
        let d = &dst.desc;
        let x0 = s.origin[0].min(d.origin[0]);
        let y0 = s.origin[1].min(d.origin[1]);
        let x1 = (s.origin[0] + s.width as i32).max(d.origin[0] + d.width as i32);
        let y1 = (s.origin[1] + s.height as i32).max(d.origin[1] + d.height as i32);
        let size = [(x1 - x0) as u32, (y1 - y0) as u32];
        let max = gpu.device.limits().max_texture_dimension_2d;
        if size[0] > max || size[1] > max {
            return Err(Error::Image(format!("canvas {}x{} exceeds GPU limit {max}", size[0], size[1])));
        }

        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let result = run(gpu, effect, src, dst, frame, values, size, [x0, y0]);
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(Error::Gpu(e.to_string()));
        }
        result
    }
}

#[allow(clippy::too_many_arguments)]
fn run<E: Effect>(
    gpu: &Gpu,
    effect: &E,
    src: &HostImage<'_>,
    dst: &mut HostImageMut<'_>,
    frame: &Frame,
    values: &Values,
    size: [u32; 2],
    canvas_origin: [i32; 2],
) -> Result<(), Error> {
    use wgpu::TextureUsages as U;
    let s = src.desc;
    let d = dst.desc;
    let rel = |desc: &ImageDesc| [desc.origin[0] - canvas_origin[0], desc.origin[1] - canvas_origin[1]];

    let host_in = gpu.texture(s.width, s.height, s.layout.depth.texture_format(), U::TEXTURE_BINDING | U::COPY_DST);
    gpu.queue.write_texture(
        host_in.texture().as_image_copy(),
        src.data,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(s.stride as u32), rows_per_image: None },
        wgpu::Extent3d { width: s.width, height: s.height, depth_or_array_layers: 1 },
    );
    let canvas_in =
        gpu.texture(size[0], size[1], wgpu::TextureFormat::Rgba32Float, U::STORAGE_BINDING | U::TEXTURE_BINDING);
    let canvas_out =
        gpu.texture(size[0], size[1], wgpu::TextureFormat::Rgba32Float, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING);
    let host_out = gpu.texture(d.width, d.height, d.layout.depth.texture_format(), U::STORAGE_BINDING | U::COPY_SRC);

    let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("probable") });
    gpu.io.unpack(&gpu.device, &mut encoder, &s, &host_in.view, &canvas_in.view, size, rel(&s), frame.space);
    effect.render(
        &mut RenderCx { gpu, encoder: &mut encoder, src: &canvas_in.view, dst: &canvas_out.view, size, frame },
        values,
    );
    gpu.io.pack(&gpu.device, &mut encoder, &d, &host_out.view, &canvas_out.view, rel(&d), frame.space);

    let row = d.row_bytes();
    let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
    let staging = gpu.staging((padded * d.height as usize) as u64);
    encoder.copy_texture_to_buffer(
        host_out.texture().as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: staging.buffer(),
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded as u32), rows_per_image: None },
        },
        wgpu::Extent3d { width: d.width, height: d.height, depth_or_array_layers: 1 },
    );
    let submission = gpu.queue.submit(Some(encoder.finish()));

    let slice = staging.buffer().slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device
        .poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: None })
        .map_err(|e| Error::Gpu(format!("poll: {e}")))?;
    rx.recv().map_err(|_| Error::Gpu("map callback dropped".into()))?.map_err(|e| Error::Gpu(format!("map: {e}")))?;
    {
        let mapped = slice.get_mapped_range().map_err(|e| Error::Gpu(format!("mapped range: {e}")))?;
        for (y, chunk) in mapped.chunks_exact(padded).enumerate() {
            let at = y * d.stride;
            dst.data[at..at + row].copy_from_slice(&chunk[..row]);
        }
    }
    staging.buffer().unmap();
    Ok(())
}
