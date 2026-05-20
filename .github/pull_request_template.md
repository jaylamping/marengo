## Summary

<!-- What changed and why? -->

## Checklist

- [ ] `just check` (or `docker compose run --rm check`) passes
- [ ] If `proto/` changed: `cd consul && npm run gen:proto` and updated `consul/src/gen/.checksum` if TS output changed
- [ ] If control/safety behavior changed: updated [docs/safety.md](../docs/safety.md) or noted N/A
- [ ] Follows [docs/rust-patterns.md](../docs/rust-patterns.md)

## Safety impact

<!-- None / bench-only / affects enable path / hardware — describe -->
