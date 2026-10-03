# Nano Station equip route

Question: why does the Nano Station service expose no way to replace equipped Nanos in FFOne?
The native NpcIcon service entry existed, but its action had no handler. The shared Nano viewer
implemented only inventory ownership, so it always displayed the station notice.

## Primary authority

This recovery uses canonical `primary`, Retrobution 2026-08-21. The exact `main.unity3d` is
8,221,718 bytes, SHA-256 `01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
The extracted `Assembly - CSharp.dll` is 1,762,816 bytes, SHA-256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The managed assembly report and decompilation are reused from
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary`.

Focused evidence is under `work/legacy-sources/nano-station-20260904`:

- `nano-machine.evidence.json`: strict `sharedassets0.assets` MonoBehaviour 1420,
  raw object SHA-256 `e42020f86b7d42ed222fd74963cb59cc55612cc04c430b0bfc82a1e280a536da`.
  It belongs to GameObject 1338 (`InvManager`) and resolves MonoScript 1141 (`NanoMachineScript`).
  It selects NanoSkin 1379 and popup textures 677/609/552, all through local pointers.
- `interaction.evidence.json`: static IL/CFG analysis of `NanoMachineScript.DoNanoPopup`,
  bound to the exact assembly above. This inventories calls, not runtime reachability.
- `skin.triage.json`: exact NanoSkin object and local style references. The normal, hover and
  active backgrounds resolve locally to 640/309/640. Unused built-in toggle backgrounds and
  the built-in skin script reference `library/unity default resources` and remain unresolved.
  No accepted control uses those unresolved references.
- `button-normal.json`, `button-hover.json`, `button-native-check.json`: exact raw texture
  exports checked against the existing native PNGs. No image payload was changed.

Replay from FusionForge with `target-build/debug/fusionforge.exe fusionforge`:

```text
dump-object-evidence primary ../builds/retrobution-20260821 main.unity3d 1420 --serialized-asset sharedassets0.assets --type MonoBehaviour --out work/legacy-sources/nano-station-20260904/nano-machine.evidence.json
export-ui-interaction-evidence work/legacy-sources/nano-station-20260904/interaction.request.json --out work/legacy-sources/nano-station-20260904/interaction.evidence.json
export-exact-texture ../builds/retrobution-20260821/main.unity3d 640 work/legacy-sources/nano-station-20260904/button-normal.json
export-exact-texture ../builds/retrobution-20260821/main.unity3d 309 work/legacy-sources/nano-station-20260904/button-hover.json
```

## Native behavior

`NpcIconMode.StartNanoMachine` selects the equip mode. `CnEquip.SetEquipMode` opens the shared
inventory, equipment and avatar panels with the Nano tab selected. `NanoMachineScript` exposes
the mutation controls only with station ownership. The ordinary Nano book remains read-only.

Coordinates below are popup-local, x right/y down. Equipped popups retain the existing native
inner offset (2.5, 18); nothing changes the accepted gallery or avatar layout.

| Control | Rect | Gate/action |
| --- | --- | --- |
| EQUIP caption | 155,556,55,14 | Owned, not equipped |
| NANO 1 | 25,574,101,26 | Send equip to wire slot 0 unless that slot is active |
| NANO 2 | 130,574,101,26 | Send equip to wire slot 1 unless that slot is active |
| NANO 3 | 235,574,101,26 | Send equip to wire slot 2 unless that slot is active |
| UNEQUIP | 220,574,120,26 | Send unequip using the Nano's actual equipped slot |

NanoSkin's button has border 6/6/6/4, centered JEFFE14 text, normal/active color
(0.8,1,1), hover color (0,0.2784314,0.4784314). The existing approved `fonts/jeffe.otf`
replacement is reused. EN/RU labels use `ui.nano_station.*` keys. Control paint order stays
above the equipped popup's inner background.

Requests lock the popup without changing bank or slot authority. Exact matching success unlocks
and closes the card, following `InventoryManagerScript.ReceiveNanoPacket`, then the existing
production projection refreshes from the server state. A ten-second unlock is a native transport
recovery extension; it does not invent a successful equip. Closing the session or losing the NPC
removes station ownership. Untuned/unowned entries cannot emit equip requests.

Scope is the missing equip/unequip UI. Paid power activation and the station camera/animation
are separate existing gaps; this change does not assert complete Nano Station visual parity.

## Verification

Native tests cover station versus book ownership, active-slot rejection, unowned and invalid
targets, exact unequip slot selection, pending reply correlation, timeout, and pointer emission
without authoritative mutation. The production EN/RU bundle test includes every new label.

The existing `user_equip_ui_gpu_preview` accepts `FFONE_NANO_STATION=1` and
`FFONE_USER_EQUIP_LOCALE=en|ru`; its existing Nano viewer selector chooses the card.
Five station tests passed, including an ECS spawn/bind test of absolute control coordinates,
exclusive equip/unequip visibility and ordinary-book hiding. Three existing Nano projection,
tab and viewer tests and the production localization test also passed (nine targeted tests).
All 65,711 production EN/RU keys and placeholders matched.

Accepted native GPU captures were visually reviewed on 2026-09-04. They live in the ignored
work directory above; the index is `FFONE_USER_EQUIP_NANO_VIEWER_INDEX`:

| Capture | Locale/index | SHA-256 |
| --- | --- | --- |
| `station-equip-en.png` | en/4 | `bdfd1e4dcdea2cb969f17cdd633954725b79347663e350bd6ad232bcf2b13df2` |
| `station-equip-ru.png` | ru/4 | `fe6cc876a691f43aecae9e507eae87f917416b7cca482e6e1eb3e49a253afc67` |
| `station-unequip-ru.png` | ru/3 | `ed39dda42900d60e1a433d4058e75e83c34c8c2fe512936a4f43b68e9759f2a4` |

The captures show all three slot buttons, disabled active slot 1, readable EN/RU labels,
and only UNEQUIP on the equipped card. Earlier `*-first.png` frames are rejected iterations.
The unchanged preview example was compiled against the current standard native library into
the ignored work directory to avoid the concurrent Cargo queue. No alternate UI implementation
was used. The development client built at 03:13 passed `--validate-assets` and a 20-second
startup smoke check. No live-server equip transaction or primary-live screenshot comparison
is claimed by these checks.

The final standard Cargo build of both `ffone-client` and `user_equip_ui_gpu_preview` also
completed successfully (`cargo build -p ffone-client --bin ffone-client --example
user_equip_ui_gpu_preview`). The long elapsed time included the shared Cargo build lock.
