# Cases for scripts\ae-smoke.ps1: each becomes one comp in every smoke project (one project per
# bit depth), rendered by AE and compared against the CLI with the same settings.
#   Name       CLI effect name (`probable list`), also the comp and output file name
#   MatchName  AE match name (EffectInfo::match_name)
#   AeParams   AE param label = value (numbers; popups are 1-based, checkboxes 0/1)
#   CliParams  the same settings as CLI key=value
@(
    @{ Name = "bloom"; MatchName = "PROBABLE Bloom"; AeParams = @{ Intensity = 2 }; CliParams = @("intensity=2") }
)
