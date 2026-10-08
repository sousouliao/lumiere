# Windows Development Runbook

## Prerequisites

- Windows x64, WebView2 runtime, Visual Studio C++ build tools and Windows SDK
- Node.js 22 or newer, repository-pinned pnpm 11.7.0
- Rust from `rust-toolchain.toml` (rustfmt and Clippy included)

## Repository And Native Gates

From the repository root:

```powershell
pnpm install --frozen-lockfile
pnpm check
pnpm test:shared
pnpm build
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --features lumiere-desktop/custom-protocol -- -D warnings
cargo test --locked --workspace --features lumiere-desktop/custom-protocol
cargo build --locked -p lumiere-windows-host
node scripts/verify-rust-protocol.mjs
```

`pnpm dev` builds the Debug Host, starts Vite and the Tauri shell. Release native
build: `pnpm --filter @lumiere/desktop build:native`. The shell supervises the adjacent
Host; a missing or disconnected Host is a recoverable product state.

Hardware/GUI tests are ignored by default. Read their specific prerequisites before
explicitly running them. Native Region fixtures live in `hosts/windows/tests/native-region.rs`;
on-demand WebView/settings/shortcut fixtures live in the shell's `src/tests.rs`.
`node scripts/verify-rust-display.mjs` performs real capture and delivery. Fixtures do
not certify physical input, compositor fidelity or other display/DPI combinations.

## Local Packaging

`pnpm package:windows` builds locked native payloads, transaction manifests, the NSIS
installer and matching `latest.yml` / signed `latest.json` into
`artifacts/windows/release`. It never pushes, tags or publishes. Updater signing uses
`TAURI_SIGNING_PRIVATE_KEY` (or its path) and password from the environment; local
fallback credentials live outside the repository at `%USERPROFILE%/.lumiere/updater`.
Never commit or print private keys/passwords. CI requires separately provisioned secrets.

`powershell -NoProfile -File scripts/verify-windows-installer.ps1` exercises isolated
fresh/reinstall/legacy/official-updater fixtures. It temporarily exports the three owned
HKCU registration keys, pins `/D` to fixture directories and waits for installer commit
before restoring registrations. Keep the real installed application out of fixture
lifecycle actions. Never execute the legacy recursive uninstaller.

## Truth And Recovery

Record exact commands, named display/HDR state, artifact delivery and visual observations
in the owning Issue. Repository tests, local runtime and CI are different truth levels.
The rewrite needs its own clean-machine, SDR/HDR, physical Region/compositor, topology,
DPI, sleep/reconnect, total-process memory and hot median/p90 observations; earlier
waivers do not certify it. Missing evidence blocks release claims, not honest source work.

Install rollback preserves originals and unknown files using an owned sibling backup.
On failure inspect structured stderr and `%TEMP%/Lumiere-installer-error.txt`; retry only
after the prior installer exits. Never recursively remove an installation directory as
recovery. See [release runbook](releasing.md) for separately authorized publication.
