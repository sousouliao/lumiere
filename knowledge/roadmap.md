# Product Roadmap

Current posture belongs to `state/CURRENT.md`; Issues own executable acceptance.
[ADR 0021](decisions/0021-windows-only-tauri-rust-migration.md) supersedes the former
cross-platform implementation route. The current product and distribution target Windows x64.

| Gate | Outcome | Required evidence |
|---|---|---|
| Windows replacement | Tauri/React UI, independent Rust capture library and resident JSONL Host | Approved P0–P6 migration acceptance under #25; source, UI, lifecycle, upgrade and baseline comparisons |
| HDR-aware MVP | Native Region/Display to fixed sRGB Visual Match | Named SDR/HDR/DPI/topology/consumer observations, clean install and update, stable repeat/cancel/shutdown |
| HDR-preserved export | One narrow named artifact path | Exact format/color/metadata/viewer contract and real hardware evidence |
| Wider fidelity claims | Measured consistency in a named support matrix | Fixed-scene tolerances and independent evidence for every claimed environment |

UI and output semantics stay governed by their contracts. Shipping a smaller binary
or passing protocol tests does not prove visual fidelity. No future platform adapter,
compatibility stub or speculative abstraction is part of this migration. Revisiting
platform scope requires a separate decision and acceptance criteria.
