# bins/marengo-limit-sync/

## Responsibility
Apply a Set Limits patch (hard/soft limits, expand-only URDF) to the local git checkout.

## Design
- Thin wrapper around `marengo-config::apply_local_limit_patch`
- Invoked by Consul only after the Pi reports Durable persist — never on Pending alone
- CLI args: `--repo-root`, `--joint`, `--lower/--upper`, `--soft-inset` or paired `--soft-lower/--soft-upper`
- One-sided soft flags are refused; all output goes through `tracing`
