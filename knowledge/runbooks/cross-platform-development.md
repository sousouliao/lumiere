# Desktop Renderer Development Runbook

The retained React UI uses Tauri named commands and events. Native capture and hardware
checks belong to the [Windows runbook](windows-development.md).

```powershell
pnpm install --frozen-lockfile
pnpm check
pnpm test:shared
pnpm build
pnpm dev
```

Node.js 22+, pinned pnpm and the Windows native prerequisites are required for the full
app. `pnpm test` aliases the renderer/shared vocabulary tests. Markup tests establish
layout semantics and accessibility attributes, not interactive or native behavior.
Read `apps/desktop/DESIGN.md` before changing a surface. Keep generated tokens unchanged
unless updated from the design source. The Windows boards and retained baseline UI own
current chrome; historical design variants do not imply additional platform support.

## Renderer Components

The renderer uses Tailwind CSS 4 and copies beUI source through the configured
`@beui` shadcn registry. There is no beUI runtime package: installed components live
under `apps/desktop/src/renderer/src/components` and are owned by this repository.

Installed beUI components:

- `button-base`
- `dock`
- `select`
- `switch`

Preview and add a component from the live registry:

```sh
pnpm --filter @lumiere/desktop ui:dry-run @beui/<slug>
pnpm --filter @lumiere/desktop ui:add @beui/<slug>
```

Add its slug to the installed-components list above in the same change. Do not use
`shadcn add --all`; keep the renderer limited to components it actually uses.

To synchronize an installed component with current beUI source, start from a clean
worktree and inspect the upstream diff before overwriting local files:

```sh
pnpm --filter @lumiere/desktop exec shadcn add @beui/button-base --diff
pnpm --filter @lumiere/desktop ui:update @beui/button-base
pnpm check
pnpm test:shared
pnpm build
```

Update one component or one tightly related component family at a time. Review shared
helpers under `src/renderer/src/lib` carefully because an overwrite may affect several
installed components. Preserve Lumiere-specific accessibility, reduced-motion, theme,
and desktop interaction behavior when resolving upstream changes.

When the platform's native host executable is unavailable, the shell must report
`host-unavailable` and keep unsupported capture actions disabled. This is expected
fallback behavior, not passing capture evidence. Do not substitute browser capture to make the buttons appear to work.
