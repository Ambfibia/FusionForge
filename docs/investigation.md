# Focused legacy inspection

Use existing inventories to locate a candidate; query the smallest raw container and
bound stdout. No new cache/index or whole-build decompilation is required.

```powershell
./tools/legacy-sources/find.cmd -Source retrobution -Query "FutureNano"
cargo fusionforge inspect <container> --asset <serialized-name> --type MonoBehaviour --limit 20
cargo fusionforge inspect <container> --assembly "Assembly - CSharp.dll" --class cnQuit --method OnGUI --limit 10
cargo fusionforge inspect <container> --asset sharedassets0.assets --path-id 903 --font-text "Ag" --limit 1
```

`--format markdown` is optional final reference output. Managed inspection gives metadata
and bounded IL, not original source. Inspect command help for the exact supported surface.
With `--method`, JSON also includes up to 2,048 decoded instruction operands and resolved
MethodDef/MemberRef/field/string names. The raw `ilHex` remains capped at 512 bytes;
instruction output has an explicit limit marker. This is static IL, not recovered source.

`--font-text` requires an exact Font object (`--asset`, `--path-id`) and JSON output.
It reads the original serialized glyph vertices/UVs, advances, available font metrics
and raw object hash for at most 256 single-line characters. Missing or ambiguous glyphs
fail. These are font-local coordinates; `guiBaselineMeasured=false` explicitly means
they do not establish the rendered OnGUI baseline or a replacement-font translation.
Measure that translation on paired original/native frames at the same client size.

## Identity

Object key: `(build, relative container, serialized asset, type, pathId)`. Bundle routes,
serialized files, objects and streamed payloads are distinct. `pathId=0` is null even
with stale fileId; `fileId=0` is owner-local; positive fileId uses that owner's external
table. Missing/ambiguous resolution fails, never falls back to a similar name/ID.

Academy packed references: low 16 bits of word one select the external file, high 16
bits provide upper object-ID bits; word two provides lower 32 bits. Preserve both raw
words and the full 48-bit ID; lower-bit search is insufficient.

Trace GameObject↔components, Transform TRS/parents, renderer/collider/animation/audio/script
owners independently. Respect instantiated overrides, activity, layers/tags and disabled
objects; prefab/name/bounds similarity is not identity. Gamebryo/NIF/KFM remains distinct.
Cross-version match: inventory → container/route/owner → typed structure → behavior.

## Managed behavior and authority

`MonoBehaviour.m_Script → MonoScript → assembly/namespace/class`; instance fields are not
code. Use exact assembly bytes, decisive readers/callers, packet constants, timers,
coroutines and side effects. Check Awake/Start dependencies, enable/disable, frame/fixed
clocks, LateUpdate ordering, coroutine cancellation and OnGUI event consumption.
`callvirt` identifies a member, not the executed override. Reflection/native calls,
dynamic assemblies, server state and runtime-created objects remain explicit uncertainties.

Serialized objects prove defaults; IL proves static operations; XDT proves table relations;
runtime captures prove reachability/timing. RustyFusion/0104 owns server authority.
Label primary parity, accepted extension, donor, conversion artifact and unresolved facts.
Source roles/exceptions: [source contract](source-contract.md), only when selecting sources.

For requested evidence, retain source role, relative container/raw hash, serialized asset,
type/ID/owner, producer revision and replay command or tracked artifact hash. A case's
self-reference is not proof. Preserve rejected hypotheses and uncertainty. Existing strict
`dump-object-evidence` / `export-managed-assembly-evidence` tools and skill templates are
optional research interfaces, not required conversion stages. Never silently publish their
candidate outputs as native behavior.
