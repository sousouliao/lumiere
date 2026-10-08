# Desktop UI Contract

## Direction

Lumiere should feel like a calm, precise desktop tool on Windows. Capture
speed, accuracy, and trust take priority over decorative product presentation.

- Respect platform window, menu, shortcut, typography, focus, and permission conventions.
- Use compact tool surfaces with clear hierarchy and stable dimensions.
- Use accent color rarely and functionally for action, selection, focus, or critical status.
- Keep motion brief, stateful, and non-blocking.
- Use materials/elevation only to clarify real layering.

Avoid generic SaaS dashboards, oversized heroes, nested decorative cards,
purple-blue AI gradients, glow-heavy surfaces, low-contrast copy, and motion that
delays capture or confirmation.

## Controls And Layout

- Dark is the only delivered theme. Design tokens are generated from the Ardot prototype
  into `apps/desktop/src/renderer/src/tokens.generated.css` and are never hand-edited;
  see [`design/mvp-prototype-spec.md`](../design/mvp-prototype-spec.md) §15.
- Use Tailwind CSS 4 as the renderer styling foundation and the configured beUI
  registry as the default source for reusable animated components. Copy only
  components the product uses; the checked-in source is owned and adapted by Lumiere.
- Prefer semantic HTML controls and restrained shared styling before custom primitives.
- Use Tauri native facilities for windows, menus, tray and shortcuts;
  use native hosts for capture permission and platform-owned failure flows.
- Custom UI is appropriate for overlay, crop handles, magnifier, result preview,
  and future annotation canvas, but does not establish HDR fidelity.
- Overlay geometry and controls must remain stable while the pointer moves.
- Support platform theme behavior, keyboard access, visible focus, reduced motion,
  zoom, and platform contrast guidance.

## Writing

Use calm, concise, sentence-case copy. Tell the user what happened and what they
can do next. Keep capture, preview, conversion, delivery, and validation distinct.
Normal capture surfaces may identify sRGB Visual Match but must not expose internal
validation vocabulary or imply P3/HDR10 support.
