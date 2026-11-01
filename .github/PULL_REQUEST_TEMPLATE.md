## What changed

<!-- One or two sentences. Link the issue if there is one. -->

## Why

<!-- The problem this solves. -->

## How it was tested

- Windows version:
- Player(s):
- Installation type:
- Screenshots or recordings, for visual changes:
- Memory / CPU numbers, if rendering or the window changed:

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] `npm --prefix app/settings run build` passes
- [ ] `CHANGELOG.md` updated for user-visible changes
- [ ] README or docs updated if behavior changed
- [ ] No new telemetry or network hosts beyond LRCLIB and the documented lrcmux fallback
- [ ] Performance budget respected, or the trade-off is explained above
- [ ] No secrets, user font files, build output, installers, `site/`, or `AGENTS.md` staged
