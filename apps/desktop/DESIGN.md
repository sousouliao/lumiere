# Desktop UI Implementation

This file governs implementation choices inside `apps/desktop`. Product behavior,
copy, claims, and platform differences remain owned by the linked contracts and MVP
prototype spec; this file does not duplicate them.

## Source Order

For every renderer surface, resolve design truth in this order:

1. Behavior, state, copy, and platform differences —
   [`knowledge/design/mvp-prototype-spec.md`](../../knowledge/design/mvp-prototype-spec.md).
2. Color, type, radius, and spacing values — generated
   [`tokens.generated.css`](src/renderer/src/tokens.generated.css). Change the Ardot
   variables and re-export rather than editing this file.
3. Layout and pixel detail —
   [`lumiere-mvp-boards.pdf`](../../knowledge/design/design-export/lumiere-mvp-boards.pdf).

Inspect the relevant PDF board before implementing or reviewing a surface. Match its
hierarchy, density, alignment, states, and platform chrome as closely as the production
behavior permits. Product truth and honest capability state take precedence when a
board depicts functionality that is not implemented yet.

## Component Choice

Use the checked-in beUI source as the default for interactive and reusable renderer
components.

1. Check `src/renderer/src/components` for an installed beUI component.
2. If needed, inspect one specific registry component with `ui:dry-run`, then add only
   that component with `ui:add` and update the installed-component list in the shared
   development runbook.
3. Adapt the checked-in beUI source or add a narrow Lumiere variant when generic beUI
   motion, shape, or spacing conflicts with the approved design. The prototype wins;
   do not accept visual drift merely to preserve a component default.
4. Use semantic HTML for document structure. Use a raw interactive HTML control only
   when no installed or suitable beUI component exists, and account for that exception
   in the review or handoff.

Do not add a beUI runtime package. Lumiere owns the copied component source and keeps
the renderer limited to components it actually uses.

## Visual Completion

A renderer slice is visually complete when all affected states have been compared with
the relevant PDF boards at the intended window size and the implementation has checked:

- generated-token use with no substitute hard-coded palette;
- default, hover, pressed, disabled, and visible keyboard-focus states where applicable;
- stable geometry during pointer and capture-state changes;
- usable WebView zoom and reduced-motion behavior;
- retained Windows chrome and layout without inferring all-state pixel parity from source checks.

Keep verification proportional to the slice: run the repository gates and perform one
real visual/runtime observation on the owning platform rather than building a parallel
UI test system.
