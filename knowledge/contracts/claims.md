# Claims And Output Contract

## Public Boundary

Lumiere is HDR-aware, not HDR-certified. Native high-dynamic-range acquisition and
target-aware HDR state do not prove that an output artifact preserves HDR.

Approved language:

- "HDR-aware screenshots for Windows."
- "Native capture with a platform-aware HDR pipeline."
- "sRGB output tuned for visual match in everyday clipboard and file use."
- "HDR-preserved export is not yet a supported public path."

Prohibited without named hardware and viewer verification on every claimed platform:

- "Universal HDR fidelity."
- "Three supported color modes."
- "HDR preserved in every output."
- "HDR10/JXR supported."

## MVP Output Semantics

sRGB Visual Match is the only official MVP path. Clipboard and folder delivery on
Windows must consume the same fixed semantic conversion; target- or platform-
specific tone mapping drift beyond recorded tolerance is a Lumiere defect.

The default conversion must:

- keep ordinary SDR-range content visually stable where practical;
- smoothly compress HDR highlights rather than hard-clamping them;
- preserve usable shadows and overall contrast;
- emit compatible RGBA8/sRGB pixels;
- remain fixed and regression-testable instead of exposing capture-time tuning.

## Blocking Visual Failures

The MVP must not ship when Lumiere causes any of these in supported scenarios:

- obvious overexposure, large dead-white regions, washed-out output, or gray output;
- ordinary desktop UI that is visibly too dark, color-shifted, or collapsed in contrast;
- obvious clipboard/folder visual drift for the same converted capture;
- unexplained baseline conversion drift for the same fixed fixture beyond recorded tolerance.

Narrow target-app recoloring/compression and loss of fine detail in an extreme HDR
scene may be recorded as limitations only when Lumiere's source artifact is valid.

## Stage Two: HDR-Preserved Export Gate

Before publishing one HDR-preserved path, record its exact format and extension,
source and destination pixel formats, transfer function, primaries, tone/gamut policy,
metadata policy, named viewer assumptions, target-aware display state, and observed
hardware verification on every claimed platform. A codec or high-bit-depth pixel
format existing proves implementation only, never product behavior.

## Wider Fidelity Gate

Any future wider support claim needs fixed-scene verification across its named display
and viewer matrix, measured tolerances and known differences. No unimplemented platform
or unobserved hardware is supported by inference. Universal claims remain prohibited.
