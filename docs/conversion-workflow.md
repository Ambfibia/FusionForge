# Direct conversion

Immutable raw build → in-memory/streamed recovery and validation → final editable
`FFOneClient/assets/game` files. No staging, extraction cache, intermediate JSON/project,
required receipt or manual repair/copy/publish step. Native output has semantic IDs and
relative paths, not source locators. Optional requested research is final output, not input.

## Supported paths

```powershell
cargo fusionforge convert-native-model <bundle> <exact-container-route> <family> <native-output-root>
cargo fusionforge convert-native --recipe <recipe> --source-root <raw-root> --target-root <native-root> --check
cargo fusionforge convert-native-ui <raw-build-root> quit-menu <native-asset-root>
cargo fusionforge convert-native-ui <raw-build-root> quit-menu <native-asset-root> --check
```

Model conversion resolves sibling archives, performs GLB semantic roundtrip/native validation
in memory, and writes final GLB/PNGs to the selected domain outside the source root.
The recipe engine handles source assertions, models, textures, audio, collision and native
JSON in memory. `--check` writes nothing; `--replace-existing` is explicit replacement
consent. `publish-native` aliases this direct implementation, not a second pipeline.

QuitMenu is source-pinned: native description, six images, three accepted typed handlers,
preserved fonts and EN/RU. Unsupported screens/revisions fail explicitly. Generic managed
behavior, arbitrary UI and all historical domain repairs are not covered by that adapter.
`native` delegates to the pipeline library; `utility` exposes retained specialized operations.

`--check` runs the final-output conflict/dependency preflight without writing files
or creating directories. `--text-metrics <capture.json>` accepts a final metric capture:
an array of two `fusionforge.unity-text-adapter-input.v2` objects, standard/Cancel style
order. Conversion runs the measurement adapter in memory, requires every reachable
button label in EN/RU, and binds the capture to source container/Font-object hashes,
native font bytes, control size, padding, contentOffset, anchor, clipping, wrapping
and replacement metrics. Failed acceptance, glyph transforms or a distinct wrap width
unsupported by the consumer fail before installation. Only measured `fontCompensation`
is written to the native description; evidence stays outside runtime. Omitting capture
retains zero compensation and does not establish baseline parity. Pressed backgrounds
resolve from the actual Active state, with null falling back to Normal.
Different existing descriptions remain conflicts by default. `--replace-existing`
explicitly permits replacing the description; it can be combined with `--check` to
preflight that write without mutation. Accepted image pixels, fonts and EN/RU remain protected.

## Safety and acceptance

Preflight every dependency, unsupported feature and conflict. Equal output is reusable;
different existing files block the default write set. Preserve user edits and accepted
[source selections](source-contract.md); a generic conversion does not authorize replacing
retained production assets. Disk-write failure removes newly created files; crash-atomic
multi-file replacement is not guaranteed by the documented implementation.

Apply one basis conversion: `H=diag(-1,1,1)`, `H*p`, inverse-transpose normals, conjugate
rotations, determinant-aware winding/tangent handedness. Test parent composition and an
asymmetric mesh. Preserve hierarchy/slots/rig, full texture mip/color/sampler/pass contract,
clip events/timing/root motion, terrain seams/collision and native authority. Check only the
changed domain; [native acceptance](../../FFOneClient/docs/native-asset-workflow.md#acceptance-by-changed-domain)
holds the detailed domain checklist. UI adapter semantics: [ui-conversion](ui-conversion.md).

Replay twice; verify unsupported input/conflicts leave existing files intact; inspect the
write set; exercise the actual Bevy loader plus affected visuals/interactions. Static
analysis and compilation alone do not establish parity.

## Migration debt

Specialized `repair-native` and domain helpers still include staged implementations.
Consult [native-cli](native-cli.md) only for that operation. Their existence is not
no-intermediate acceptance: integrate into direct conversion and verify equivalent output
before retiring an old procedure. Do not run historical staged commands as normal workflow.
