# Windows installation transaction

The NSIS template derives from Tauri CLI `tauri-cli-v2.12.1` under MIT OR Apache-2.0:
https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.1/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi.
Its official RestartManager, WebView2 bootstrap, x64, shortcut/AUMID and precise-file
uninstaller remain. Lumiere removes previous-uninstaller execution and application-data
removal, inherits the existing per-user path, and surrounds file/registration writes
with the private Rust transaction helper. No second resident service is installed.

`tools/windows-installer` copies an explicit program inventory, registration values
and shortcut bytes into a hashed sibling backup outside the installation directory.
It flushes a journal before program replacement. Missing files are recorded too, so
rollback removes partially copied new files. A live installer PID/creation-time pair
rejects overlapping transactions. A subsequent installer recovers an interrupted
journal before writing. Commit checks the payload hashes and registration, removes
only the frozen legacy inventory, writes the new manifest and commits the journal.
Only the private, checked backup tree is recursively removed. Unknown files and
`%APPDATA%/Lumiere/settings.json` are never transaction-owned.

`pnpm package:windows` stages a local NSIS installer, signs its final bytes with the
official Tauri signer and creates both `latest.yml` and `latest.json` for that same
installer. `prepare-release` builds native files/manifests without an installer;
`build-installer` packages already prepared files. This performs no publication or
version decision. The source, package and Tauri versions must agree.

Private signing material must stay outside Git. CI uses `TAURI_SIGNING_PRIVATE_KEY`
or `TAURI_SIGNING_PRIVATE_KEY_PATH`, and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Local
fallback is the current user's ACL-protected `~/.lumiere/updater/lumiere.key` and
`password.txt`. Back up the private key securely before any public distribution;
losing it prevents existing clients from accepting future signed updates. The
public key is versioned in `tauri.conf.json`. Updater signatures are independent of
Authenticode; unsigned local artifacts do not claim Windows publisher identity.
