# Windows Rust Host

`lumiere-windows-host.exe` is a separate resident JSONL process supervised by Tauri.
Its transport lives in `rust/`; the independent native engine lives in
[`crates/capture-windows`](../../crates/capture-windows). The renderer never receives
raw frames or native handles.

Capabilities and Display use v5; native Region and request-id cancellation use v6.
Region retains one WGC RGBA16F frame, presents it at native resolution, and crops that
same frame. The fixed sRGB Visual Match conversion precedes clipboard/folder delivery.
An HDR-active target requires its current Windows SDR white level. Missing data fails
rather than producing an unverified artifact.

Build with `cargo build --locked -p lumiere-windows-host` from the repository root.
Debug and Release executables live in `target/debug` and `target/release`; packaged
shells use the adjacent executable. See the [Windows runbook](../../knowledge/runbooks/windows-development.md)
for tests and explicit hardware fixtures. Protocol stdout is reserved for newline-delimited
responses; structured diagnostics use stderr. EOF cancels and joins active native work.
