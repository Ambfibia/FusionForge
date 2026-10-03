# Service UI: source measurements, 2026-09-12

## Scope and source boundary

Review BankMode, VendorMode, Enchant/Croc Pot and the inventory redeem-code
footer. The owner requested functional controls, text/image positions, UI states
and nine-slice panels. Runtime implementation belongs to FFOneClient; this case
retains discovery and provenance in FusionForge.

Primary source: Retrobution 20260821, `main.unity3d`, 8,221,718 bytes,
SHA-256 `01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
The managed assembly was re-exported with FusionForge into
`work/cases/service-ui-repair-20260912/managed/Assembly-CSharp.dll`:
SHA-256 `0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The export report is `managed.evidence.json` beside that directory.
Historical decompilation locator (not a required intermediate step):
`work/legacy-sources/whole-client-audit-20260905/main-decompiled/` for
`Panel_PCStuffScript`, `Panel_BankScript`, `Panel_Vendor`, `cnEnchantMode`,
`cnGuiEnchant` and `cnItemDisplayInfo`.

## Evidence status

`sharedassets0.assets` object 1366, `FusionFallInvenSkin`, class 114,
contains 35,772 raw bytes, SHA-256
`05f47d466fd861dccfc753c98fe6a1e4511c168fc06aa741d045ab85e18b966a`.
Object 1367, `FusionFallCombi`, contains 22,744 raw bytes, SHA-256
`3101ce2fbb1caef10c327b8b6937f0f42153b162e70bb071e7045a35fb02dd10`.

Strict dumps reject built-in script/default-resource references to
`library/unity default resources`. The explicit `--allow-unresolved-pointers`
outputs `inventory-skin.triage.json` and `enchant-skin.triage.json` are TRIAGE,
not acceptance of an entirely resolved skin graph. The numeric fields and local
texture/font pointers below are available. No new texture payload was published
from the unresolved graph.

## Recovered contracts

- PCStuff Taros Rect is `(20,580,149,32)`, displayed at Y minus 20. The
  redeem-code button is `(15,598,149,25)`, derived from that footer Rect.
  It opens the shared redeem modal and gates the owning inventory controls.
- Inventory button normal and active use texture 640; hover uses 309.
  Border is left/right/top 6, bottom 4; padding is `(6,6,3,3)`.
  Normal/active text RGB is `(0.9,0.9,0.9)`; inventory hover RGB is
  `(0.229838714,0.463709682,1)`. Combi hover text is white.
- Bank information Rect is `(3,9,481,87)`. Its title is `(14,10,400,20)`
  relative to that information group, with the Morbucks bank name. The
  bank tab uses BANK. Older native VAULT/MY VAULT copy and the 150-wide title
  do not match this primary source.
- Bank dialog is `(11,105,482,531)`, viewport `(29,34,450,445)`, shadow
  `(6,32,447,449)`. Search strip is `(231,0,250,32)` and text field is
  `(295,7,177,18)`, relative to the dialog.
- Search is a case-insensitive localized item-name substring filter. An empty
  query includes vacant slots. Nonempty queries omit them and pack matches in
  six columns at 67-pixel spacing, preserving their original SlotID.
  Content height is `ceil(matches / 6) * 67`. The placeholder is shown only
  when the field is empty and unfocused.
- Textfield font pointer is 949, ChaletBook Regular Small. Border and padding
  are 3 on every side; Panel_Bank Start changes top padding to 1 and makes
  active/focused match normal. Native replacement uses ChaletBook Regular,
  12-pixel size and its existing 13.56000042-pixel line-height adapter.
  Textfield background pointer 337 already exists as the native launcher
  `ff-textfield-normal.png`; the search strip already exists in bank-mode.
- Vertical scrollbar width is 17; arrow height is 12. Track texture pointer
  is 374, thumb 324, up 63, down 415. Track/thumb border is `(2,2,4,4)`.
  Thumb fixed width is 15, fixed height 0, vertical padding 12 in total.
  Bank's right-edge scrollbar X is `29 + 450 - 17 = 462` within its dialog.
  Vendor's X is `2 + 10 + 464 - 17 = 459`; track Y is 54, height 376.

## Supplemental runtime evidence

Bevy 0.17.3 `bevy_ui/src/focus.rs` establishes that UI Interaction hit testing
uses FocusPolicy independently of Pickable. An ignored picking decoration can
still block its parent button unless its FocusPolicy passes the pointer through.

The [Unity reference SliderHandler](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Modules/IMGUI/SliderHandler.cs)
provides supplemental engine arithmetic: the thumb's minimum extent comes from
its padding, with the viewport contribution scaled over content length. Rendering
and pointer travel use the same extent. This reference is not a new primary
Retrobution binary and is kept distinct from the primary serialized measurements.

## Later native corrections (September 13–15)

Canonical runtime contracts are in FFOneClient `docs/ui/{item-cards,vendor,bank,service-modes}.md`:
retain cold image handles, distinguish table/wire NPC from placed/camera NPC identity,
use intrinsic Text within fixed Rects, proportional scrollbar geometry and captured
Enchant release. The OpenFusion table-ID mapping is compatibility behavior, not primary
Unity wire evidence. Black scroll-edge tint is an owner-selected native correction.

Detailed run records remain under the original ignored `target/performance/` case
locations and tracked evidence artifacts where supplied. They describe offline/native
fixtures, not live purchases/enchant transactions. Prior generic suite failures and
TRIAGE status above are not resolved by a successful focused fixture. This historical
case does not require rebuilding its old intermediate dump tree.
