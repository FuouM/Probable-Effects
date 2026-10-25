//! After Effects / Premiere Pro adapter. One generic implementation serves every effect; each
//! plugin crate is `probable_ae::export_effect!(probable_effects::Bloom);` plus a `build.rs`.
//!
//! Render path: SmartFX (AE) and legacy Render (Premiere), both through
//! [`probable_core::Processor`]. Params are read in SmartPreRender and handed to SmartRender as
//! pre-render data.

pub use after_effects as ae;
pub use probable_core;

use ae::{Error, InData, Layer, Parameters, PixelFormat, PreRenderExtra, Rect, SmartRenderExtra};
use probable_core::{
    ChannelOrder, ColorSpace, Depth, Effect, Frame, HostImage, HostImageMut, ImageDesc, ParamDesc, ParamKind,
    PixelLayout, Processor, Value, Values, host_params,
};

/// AE param identity: the effect's stable `ParamDesc::id`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ParamId(pub u16);

pub struct AePlugin<E: Effect> {
    processor: Processor<E>,
}

impl<E: Effect> Default for AePlugin<E> {
    fn default() -> Self {
        Self { processor: Processor::default() }
    }
}

/// Params and frame context captured in SmartPreRender for SmartRender.
struct RenderData {
    values: Values,
    frame: Frame,
}

fn all_params<E: Effect>() -> impl Iterator<Item = &'static ParamDesc> {
    E::PARAMS.iter().chain(host_params::<E>())
}

pub fn params_setup<E: Effect>(params: &mut Parameters<ParamId>) -> Result<(), Error> {
    for desc in all_params::<E>() {
        let id = ParamId(desc.id);
        let set_id = |p: &mut ae::ParamDef| {
            p.set_id(desc.id as i32);
            -1
        };
        match desc.kind {
            ParamKind::Float { default, min, max, slider_min, slider_max, precision } => params.add_customized(
                id,
                desc.label,
                ae::FloatSliderDef::setup(|f| {
                    f.set_valid_min(min);
                    f.set_valid_max(max);
                    f.set_slider_min(slider_min);
                    f.set_slider_max(slider_max);
                    f.set_default(default as f64);
                    f.set_value(default as f64);
                    f.set_precision(precision as i16);
                }),
                set_id,
            )?,
            ParamKind::Bool { default } => params.add_customized(
                id,
                desc.label,
                ae::CheckBoxDef::setup(|c| {
                    c.set_label("");
                    c.set_default(default);
                    c.set_value(default);
                }),
                set_id,
            )?,
            ParamKind::Choice { options, default } => params.add_customized(
                id,
                desc.label,
                ae::PopupDef::setup(|p| {
                    p.set_options(options);
                    p.set_default(default as i32 + 1);
                    p.set_value(default as i32 + 1);
                }),
                set_id,
            )?,
        }
    }
    Ok(())
}

fn read_values(descs: &[ParamDesc], params: &Parameters<ParamId>) -> Result<Values, Error> {
    let mut values = Values::defaults(descs);
    for (i, desc) in descs.iter().enumerate() {
        let p = params.get(ParamId(desc.id))?;
        let v = match desc.kind {
            ParamKind::Float { .. } => Value::Float(p.as_float_slider()?.value() as f32),
            ParamKind::Bool { .. } => Value::Bool(p.as_checkbox()?.value()),
            ParamKind::Choice { .. } => Value::Choice((p.as_popup()?.value() - 1).max(0) as u32),
        };
        values.set(i, v);
    }
    Ok(values)
}

fn read_render_data<E: Effect>(in_data: &InData, params: &Parameters<ParamId>) -> Result<RenderData, Error> {
    let values = read_values(E::PARAMS, params)?;
    let host = host_params::<E>();
    let host_values = read_values(host, params)?;
    let space = ColorSpace::from_index(host_values.choice(0)).ok_or(Error::BadCallbackParameter)?;
    let grow = host.len() > 1 && host_values.bool(1);
    let frame = Frame {
        time: in_data.current_timestamp(),
        scale: [f32::from(in_data.downsample_x()), f32::from(in_data.downsample_y())],
        space,
        grow,
    };
    Ok(RenderData { values, frame })
}

fn grow_rect(mut r: Rect, m: [u32; 2]) -> Rect {
    if r.is_empty() {
        return r;
    }
    r.left -= m[0] as i32;
    r.right += m[0] as i32;
    r.top -= m[1] as i32;
    r.bottom += m[1] as i32;
    r
}

impl<E: Effect> AePlugin<E> {
    pub fn handle(
        &self,
        cmd: ae::Command,
        in_data: InData,
        mut out_data: ae::OutData,
        params: &mut Parameters<ParamId>,
    ) -> Result<(), Error> {
        match cmd {
            ae::Command::About => {
                out_data.set_return_msg(&format!("{}\r{}", E::INFO.name, E::INFO.description));
            }
            ae::Command::GlobalSetup => {
                if in_data.is_premiere() {
                    let pf = ae::pf::suites::PixelFormat::new()?;
                    pf.clear_supported_pixel_formats(in_data.effect_ref())?;
                    pf.add_supported_pixel_format(in_data.effect_ref(), ae::pr::PixelFormat::Bgra4444_8u)?;
                    pf.add_supported_pixel_format(in_data.effect_ref(), ae::pr::PixelFormat::Bgra4444_32f)?;
                }
            }
            ae::Command::SmartPreRender { extra } => self.pre_render(&in_data, extra, params)?,
            ae::Command::SmartRender { extra } => self.smart_render(extra)?,
            ae::Command::Render { in_layer, out_layer } => {
                let data = read_render_data::<E>(&in_data, params)?;
                let order = ChannelOrder::Bgra;
                let layout = |l: &Layer| -> Result<PixelLayout, Error> {
                    let depth = match l.pr_pixel_format()? {
                        ae::pr::PixelFormat::Bgra4444_8u => Depth::U8,
                        ae::pr::PixelFormat::Bgra4444_32f => Depth::F32,
                        other => {
                            log::error!("{}: unsupported Premiere pixel format {other:?}", E::INFO.name);
                            return Err(Error::BadCallbackParameter);
                        }
                    };
                    Ok(PixelLayout { order, depth })
                };
                let (src_layout, dst_layout) = (layout(&in_layer)?, layout(&out_layer)?);
                self.render(&in_layer, src_layout, [0, 0], out_layer, dst_layout, [0, 0], &data)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn pre_render(
        &self,
        in_data: &InData,
        mut extra: PreRenderExtra,
        params: &Parameters<ParamId>,
    ) -> Result<(), Error> {
        let data = read_render_data::<E>(in_data, params)?;
        let reach = E::margin(&data.values, data.frame.scale);

        // Ask for input around the requested region: light from just outside still lands inside.
        let mut req = extra.output_request();
        let rect = grow_rect(req.rect.into(), reach);
        req.rect = rect.into();
        let checkout = extra.callbacks().checkout_layer(
            0,
            0,
            &req,
            in_data.current_time(),
            in_data.time_step(),
            in_data.time_scale(),
        )?;

        let grow = if data.frame.grow { reach } else { [0, 0] };
        extra.set_result_rect(grow_rect(checkout.result_rect.into(), grow));
        extra.set_max_result_rect(grow_rect(checkout.max_result_rect.into(), grow));
        extra.set_pre_render_data(data);
        Ok(())
    }

    fn smart_render(&self, extra: SmartRenderExtra) -> Result<(), Error> {
        let cb = extra.callbacks();
        let data = extra.pre_render_data::<RenderData>().ok_or(Error::BadCallbackParameter)?;
        let Some(input) = cb.checkout_layer_pixels(0)? else {
            return Ok(());
        };
        let Some(output) = cb.checkout_output()? else {
            cb.checkin_layer_pixels(0)?;
            return Ok(());
        };
        let layout = |l: &Layer| -> Result<PixelLayout, Error> {
            let depth = match l.pixel_format()? {
                PixelFormat::Argb32 => Depth::U8,
                PixelFormat::Argb64 => Depth::U16Ae,
                PixelFormat::Argb128 => Depth::F32,
                other => {
                    log::error!("{}: unsupported AE pixel format {other:?}", E::INFO.name);
                    return Err(Error::BadCallbackParameter);
                }
            };
            Ok(PixelLayout { order: ChannelOrder::Argb, depth })
        };
        let result = (|| {
            let (src_layout, dst_layout) = (layout(&input)?, layout(&output)?);
            let src_origin = [input.origin().h, input.origin().v];
            let dst_origin = [output.origin().h, output.origin().v];
            self.render(&input, src_layout, src_origin, output, dst_layout, dst_origin, data)
        })();
        cb.checkin_layer_pixels(0)?;
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn render(
        &self,
        input: &Layer,
        src_layout: PixelLayout,
        src_origin: [i32; 2],
        mut output: Layer,
        dst_layout: PixelLayout,
        dst_origin: [i32; 2],
        data: &RenderData,
    ) -> Result<(), Error> {
        let desc = |l: &Layer, layout: PixelLayout, origin: [i32; 2]| ImageDesc {
            width: l.width() as u32,
            height: l.height() as u32,
            stride: l.buffer_stride(),
            bottom_up: l.row_bytes() < 0,
            straight_alpha: false,
            layout,
            origin,
        };
        let src = HostImage { data: input.buffer(), desc: desc(input, src_layout, src_origin) };
        let dst_desc = desc(&output, dst_layout, dst_origin);
        let mut dst = HostImageMut { data: output.buffer_mut(), desc: dst_desc };
        self.processor.render(&src, &mut dst, &data.frame, &data.values).map_err(|e| {
            log::error!("{}: {e}", E::INFO.name);
            Error::Generic
        })
    }
}

/// Compile-time string equality, used to keep `build.rs` PiPL names in sync with `EffectInfo`.
pub const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Export an effect as an AE/Premiere plugin. The plugin crate's `build.rs` must call
/// `probable_ae_build::effect` with the same name and match name as the effect's `EffectInfo`.
#[macro_export]
macro_rules! export_effect {
    ($effect:ty) => {
        const _: () = assert!(
            $crate::str_eq(env!("PIPL_NAME"), <$effect as $crate::probable_core::Effect>::INFO.name)
                && $crate::str_eq(env!("PIPL_MATCH_NAME"), <$effect as $crate::probable_core::Effect>::INFO.match_name),
            "build.rs PiPL name/match name differ from the effect's EffectInfo"
        );

        // define_effect! names `ae::Error` unqualified in its generated traits.
        use $crate::ae;
        $crate::ae::define_effect!($crate::AePlugin<$effect>, (), $crate::ParamId);

        impl AdobePluginGlobal for $crate::AePlugin<$effect> {
            fn params_setup(
                &self,
                params: &mut $crate::ae::Parameters<$crate::ParamId>,
                _in_data: $crate::ae::InData,
                _out_data: $crate::ae::OutData,
            ) -> Result<(), $crate::ae::Error> {
                $crate::params_setup::<$effect>(params)
            }

            fn handle_command(
                &self,
                cmd: $crate::ae::Command,
                in_data: $crate::ae::InData,
                out_data: $crate::ae::OutData,
                params: &mut $crate::ae::Parameters<$crate::ParamId>,
            ) -> Result<(), $crate::ae::Error> {
                self.handle(cmd, in_data, out_data, params)
            }
        }
    };
}
