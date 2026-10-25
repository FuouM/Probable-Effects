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
)
