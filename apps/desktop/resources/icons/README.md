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

- `mac/app-icon.png` is the generated raster counterpart used to validate the application
  artwork; the menu-bar-only app does not apply a runtime Dock icon.
- `mac/app.icns` is the macOS bundle icon input for the repository packager.
- `mac/trayTemplate.png` and `mac/trayTemplate@2x.png` are black-and-alpha
  Template Images for light and dark menu bars.
- `windows/app.ico` is the Windows executable and window icon input. It contains
  16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, and 256 pixel images.
- `windows/tray.ico` is a transparent coral silhouette with 16, 20, 24, 32,
  40, 48, and 64 pixel images.

Development loads tray icons from this directory through `app.getAppPath()`. The macOS
packager copies only the two menu-bar Template Images to `<resources>/icons` and configures
`mac/app.icns` as bundle metadata; the Windows packager copies its runtime icons and
configures `windows/app.ico` as the executable icon. Packaging, platform-specific signing,
release artifacts, and installer behavior remain owned by Milestone 1D; the macOS release
policy uses coherent ad-hoc signing and direct disk-image distribution rather than
Developer ID notarization.
