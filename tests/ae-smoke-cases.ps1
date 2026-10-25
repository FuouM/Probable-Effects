# Cases for scripts\ae-smoke.ps1: each becomes one comp in every smoke project (one project per
# bit depth), rendered by AE and compared against the CLI with the same settings.
#   Name       CLI effect name (`probable list`), also the comp and output file name
#   MatchName  AE match name (EffectInfo::match_name)
#   AeParams   AE param label = value (numbers; popups are 1-based, checkboxes 0/1)
#   CliParams  the same settings as CLI key=value
@(
    @{ Name = "bloom"; MatchName = "PROBABLE Bloom"; AeParams = @{ Intensity = 2 }; CliParams = @("intensity=2") }
    @{ Name = "halation"; MatchName = "PROBABLE Halation"; AeParams = @{ Intensity = 1 }; CliParams = @("intensity=1") }
    @{ Name = "shoulder"; MatchName = "PROBABLE Shoulder"; AeParams = @{ Exposure = 3 }; CliParams = @("exposure=3") }
    @{ Name = "aberration"; MatchName = "PROBABLE ChromaticAberration"; AeParams = @{ "Amount (px)" = 8 }; CliParams = @("amount=8") }
    @{ Name = "grain"; MatchName = "PROBABLE FilmGrain"; AeParams = @{ Amount = 0.1 }; CliParams = @("amount=0.1") }
    @{ Name = "vignette"; MatchName = "PROBABLE Vignette"; AeParams = @{ Amount = 0.8 }; CliParams = @("amount=0.8") }
    @{ Name = "hatch"; MatchName = "PROBABLE Hatch"; AeParams = @{ "Line Spacing (px)" = 8 }; CliParams = @("spacing=8") }
    @{ Name = "print"; MatchName = "PROBABLE Print"; AeParams = @{ "Halftone Cell (px)" = 6; "Ink Bleed (px)" = 4 }; CliParams = @("cell=6", "bleed=4") }
    @{ Name = "ascii"; MatchName = "PROBABLE Ascii"; AeParams = @{ "Cell Width (px)" = 16 }; CliParams = @("cell=16") }
    @{ Name = "droste"; MatchName = "PROBABLE Droste"; AeParams = @{ "Fall (levels)" = 0.4; "Ring Opacity" = 1 }; CliParams = @("fall=0.4", "ring_opacity=1") }
    @{ Name = "shards"; MatchName = "PROBABLE MirrorShards"; AeParams = @{ "Shatter (s)" = 0.25 }; CliParams = @("shatter=0.25") }
)
