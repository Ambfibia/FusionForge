# World-map NPC visibility, 2026-09-10

Question: why does the vendor filter admit Ranger Sam beyond the named world,
and why can local-view panning expose distant NPCs?

Authority: primary `retrobution-20260821`, `main.unity3d`, level 0,
`Assembly - CSharp.dll`, `WorldMapMode.DoWindow` and `GetDongName`.
The raw container SHA-256 is
`01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`;
the assembly SHA-256 is
`0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
Both match the existing primary managed evidence used for decompilation.

Re-exported from the Editor root with:

```powershell
work/build/debug/fusionforge.exe export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d 'Assembly - CSharp.dll' --level 0 --out work/cases/world-map-npc-visibility-20260910/assembly.evidence.json --payload-out work/cases/world-map-npc-visibility-20260910/Assembly-CSharp.dll
```

The existing focused decompilation is
`work/legacy-sources/gm-chat-commands-primary/decompiled/WorldMapMode.cs`.
`screenSize` is `(950,624)` (line 71). `DoWindow` constructs a rectangle of
that size centered on the player (1514–1520), intersects it with the target
view for Type4 NPC admission (1539), then requires `GetDongName` (1546).
`GetDongName` rejects unknown locations (612–630). Presence is keyed by NPC
type, not placement, and therefore cannot replace the location check.

The native waypoint catalog has five type-653 placements. Row 1227 is the
ordinary Peach Creek Commons vendor. Rows 2593–2596 lie near X=3325–3361,
Z=7530–7565 and have no named location in the native world-location table.
The runtime fix restores named-location admission and the player-centered
local rectangle. It keeps placement data, marker projection, filters,
mission priority, interpolation, fonts and layout intact.

Verification: `cargo test -p ffone-client --lib world_map -- --nocapture`
passes all 24 matching tests. The separate production EN/RU key and template
parity test also passes (66,846 keys). New tests consume the production
type-653 rows and exercise local panning followed by player movement.
The two regressions also fail against a disposable copy with only the two
restored gates removed: Sam yields five markers instead of one, and panning
admits the distant NPC. The production source is not changed by this check.
English Type1 and Type4 GPU
previews pass their text audits and were visually reviewed; captures are
under FFOneClient `target/performance/map-visibility/{world,local}-en.png`.
Their SHA-256 values are respectively
`C84BEB80559714EF67921CB58987C01672FEFFAA49120768EA79E142F6DDC5A6` and
`EF5446B15C58A4B211A7DB65A2D0D88ACF2F712DA9494BB487CF7C3224FAB047`.
The Russian preview stops at the existing text-metric audit: the current
localization adapter gives the location label size 7 / line 6.855, whereas
the harness expects the unscaled label metrics. No font change is included.

The normal `cargo build -p ffone-client --bin ffone-client --locked` also
succeeded. The rebuilt Dev executable was exercised with
`FFONE_PERF_RETROBUTION_MAP=1`, `FFONE_PERF_POSITION='-4333.2 -52.63 3828.37'`,
`FFONE_PERF_OUTPUT=target/performance/map-visibility/full-client`, and
`--language ru`. This uses the production catalog and map projection, with
all NPC types admitted by the offline fixture. It exits successfully and
records `assets=Ready`, `phase=Open`, `validation=Ok(())`, and 290 markers.
The Russian full-client frame was visually reviewed; SHA-256:
`7A0B9ED88BB0C24586F149F2DBD672E31B3B7E1E412723B860F76BBC237F9300`.
The report's rejected-gameplay-packet message is the documented offline
fixture boundary, not a live-server validation result. Timing from this
capture is not an FPS comparison.
