# Desktop icon resources

These files are generated from the selected canonical artwork at
`assets/brand/lumiere-logo.png`.

Application icon compositions place the white ferret at 100% scale, centered
horizontally and anchored to the bottom, so the coral field retains balanced
breathing room. Tray assets keep the fuller silhouette because 16px recognition
takes priority over app-icon spacing.

Run from the repository root:

```sh
pnpm icons:generate
pnpm icons:check
```

## Runtime ownership

`windows/app.ico` supplies the executable/window icon (16 through 256 pixel DPI
representations). `windows/tray.ico` and `windows/tray.png` supply the native tray
silhouette. Tauri embeds the runtime icons; the packager does not ship unused platform
resources. Generation and validation operate only on these Windows assets.
