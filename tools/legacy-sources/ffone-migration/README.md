# Historical domain helpers

These scripts retain specialized migration/research behavior, not a supported direct
conversion API. Several require dumps/candidates and write reports. Do not create `work/`
or intermediate projects to follow old instructions; migrate a needed operation into
FusionForge and compare its output first. [Coverage](../../../docs/conversion-workflow.md).

| Need | Existing helper |
| --- | --- |
| Character UI from old dumps | `build_native_character_ui_assets.mjs` |
| Shader/prop evidence | `generate_logical_shader_coverage.mjs`, `report-logical-prop-candidates.mjs` |
| Explicit GPU research | `run-logical-model-gpu-batch.mjs`, `summarize-logical-model-gpu-batch.mjs`, `render_xdt_nano_gallery.mjs` |
| Existing native candidate publication | `install-semantic-character-overlay.mjs`, `publish-xdt-npc-texture-catalog.mjs`, `promote_native_world_glb.mjs` |
| World audio | `publish-retrobution-world-audio.ps1` |
| Requested reference capture/input | `capture_process_window.ps1`, `click_process_window.ps1`, `unity_reference_input.ps1` |

Source builds stay immutable. Inspect a helper's actual inputs/outputs before reuse;
historical hardcoded paths do not define current project layout or runtime dependencies.
