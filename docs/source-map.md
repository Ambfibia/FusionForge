# Source ownership

The workspace root owns configuration; `crates/fusionforge` owns the CLI.
Start with a symbol in one crate, not all source or all historical evidence.

| Area | Source |
| --- | --- |
| CLI, Unity reading, managed/script inspection | `crates/fusionforge/src/fusionforge/` |
| Legacy backend and model/texture/container operations | `crates/fusionforge/src/backend/` |
| Native UI conversion and installation | `crates/ffone-asset-pipeline/src/ui/` |
| Character exports and shared rigs | `crates/ffone-asset-pipeline/src/characters/` |
| Model publication and materials | `crates/ffone-asset-pipeline/src/models/` |
| World, terrain and static content | `crates/ffone-asset-pipeline/src/world/` |
| Audio and catalog operations | `crates/ffone-asset-pipeline/src/audio/`, `catalogs/` |
| Shared conversion helpers | `crates/ffone-asset-pipeline/src/shared.rs` |
| Content schema and reference audit | `crates/ffone-content/`, `crates/ffone-reference-audit/` |

Facades preserve existing public module names. Read the relevant implementation,
not the full reexport list. Unit tests live with their owner; root `tests/` retains
workspace integration fixtures. Historical evidence/receipts retain their original
paths and hashes as dated evidence, not current build instructions.

```powershell
rg -n -m 20 "symbol" crates/ffone-asset-pipeline/src/ui
rg --no-ignore -n "regression" crates/ffone-asset-pipeline/src/ui/native_ui_install/tests
cargo test -p ffone-asset-pipeline --lib native_ui_install
cargo check -p fusionforge
```

`.ignore` excludes vendor, fixtures, payload assets and historical evidence from broad
searches. For those tasks use `rg --no-ignore` with an exact file/directory. The filter
is not a build exclusion. Source-contract evidence is still available when required.

`generate-wire-0104.py` emits final protocol modules through `partition_wire.py` and
`rust_items.py`; no intermediate Rust dump is required. Do not combine generated wire
records into a giant hand-edited file or bulk-read them for non-protocol work.
