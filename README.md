# Probable-Effects

GPU effects from [probable-video-editor](https://github.com/FuouM/probable-video-editor) as
After Effects / Premiere Pro plugins (OpenFX and a browser catalog are planned). Rust + wgpu,
Windows x64.

Effects: **Probable Bloom**. The rest of the list and the plan are in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Build

Needs Visual Studio Build Tools (MSVC + Windows SDK) and a GPU. Rust is installed into the repo:

```powershell
.\setup_env.ps1          # once; also writes env.yaml (machine paths, git-ignored, see env.example.yaml)
. .\env.ps1              # every new shell
cargo test --workspace
```

## Try an effect without a host

```powershell
cargo run -p probable-cli -- list
cargo run -p probable-cli -- render bloom tests\fixtures\smoke.png out.png intensity=2 size=256 grow=on
```

PNG (8/16-bit) and EXR (32-bit float) in and out. `space=` picks the working space
(`srgb`, `linear`, … see `list`).

## After Effects

```powershell
.\scripts\ae-install.ps1 -Setup   # once, in an elevated PowerShell
.\scripts\ae-build.ps1            # add -Release for an optimised build
.\scripts\ae-install.ps1          # close AE first
.\scripts\ae-smoke.ps1            # optional: headless aerender check against the CLI
```

Effects appear under **Effect > Probable**.

## License

AGPL-3.0, see [LICENSE](LICENSE).
