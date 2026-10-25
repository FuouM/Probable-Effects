//! `build.rs` side of a Probable AE plugin: one call emits the PiPL resource with the flags the
//! shared adapter (`probable-ae`) relies on.

use pipl::*;

const PF_PLUG_IN_VERSION: u16 = 13;
const PF_PLUG_IN_SUBVERS: u16 = 28;

pub struct Plugin {
    /// Must equal the effect's `EffectInfo::name` (checked at compile time by `export_effect!`).
    pub name: &'static str,
    /// Must equal `EffectInfo::match_name`. Never change after release: AE projects store it.
    pub match_name: &'static str,
    pub category: &'static str,
    /// Output changes with time even when params do not (grain, flicker): AE must not reuse frames.
    pub time_varying: bool,
}

#[rustfmt::skip]
pub fn effect(p: Plugin) {
    let major: u32 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    let minor: u32 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap();
    let patch: u32 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap();

    let mut flags = OutFlags::DeepColorAware;
    if p.time_varying {
        flags |= OutFlags::NonParamVary;
    }

    pipl::plugin_build(vec![
        Property::Kind(PIPLType::AEEffect),
        Property::Name(p.name),
        Property::Category(p.category),
        #[cfg(target_os = "windows")]
        Property::CodeWin64X86("EffectMain"),
        #[cfg(target_os = "macos")]
        Property::CodeMacIntel64("EffectMain"),
        #[cfg(target_os = "macos")]
        Property::CodeMacARM64("EffectMain"),
        Property::AE_PiPL_Version { major: 2, minor: 0 },
        Property::AE_Effect_Spec_Version { major: PF_PLUG_IN_VERSION, minor: PF_PLUG_IN_SUBVERS },
        Property::AE_Effect_Version { version: major, subversion: minor, bugversion: patch, stage: Stage::Develop, build: 1 },
        Property::AE_Effect_Info_Flags(0),
        Property::AE_Effect_Global_OutFlags(flags),
        Property::AE_Effect_Global_OutFlags_2(
            OutFlags2::SupportsSmartRender
                | OutFlags2::FloatColorAware
                | OutFlags2::SupportsThreadedRendering
                | OutFlags2::SupportsGetFlattenedSequenceData,
        ),
        Property::AE_Effect_Match_Name(p.match_name),
        Property::AE_Reserved_Info(8),
        Property::AE_Effect_Support_URL("https://github.com/FuouM/Probable-Effects"),
    ]);
    println!("cargo:rustc-cfg=with_premiere");
    println!("cargo:rustc-cfg=catch_panics");
}
