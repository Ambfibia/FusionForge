# Shared UI: primary evidence (September 2026)

Historical evidence, not a current task list or mandatory migration procedure. Existing
character shaders, language controls and replacement fonts are intentional extensions.
Native ownership is documented once in FFOneClient `docs/ui/{item-cards,bank,vendor,service-modes}.md`.
Old `work/` paths below locate historical reports; do not recreate them as a conversion step.

## Evidence questions

1. Which NPC and framing populate Combi/Enchant primary and waiting panels, and who releases the native cameras and render targets?
2. How does shared inventory code entry retain draft text, block input, and return a submit/cancel choice to Enchant without changing inventory locally?

Authority: Retrobution 20260821, canonical `primary`, raw `main.unity3d`, 8,221,718 bytes, SHA-256 `01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`. Managed authority and replay commands are recorded in `whole-client-static-audit-20260905.md` and its hashed artifact manifest.

## Recovered contracts

| Role | Primary evidence | Native consumer / acceptance |
|---|---|---|
| Combi target | `cnCombiMode.InitCombiMode`: event `(4,3)` queries table NPC 3219 and assigns both cameras | The live service-portrait owner resolves table identity; no character-model clone |
| Enchant target | `cnEnchantMode.InitEnchantMode`: `NpcIconMode.pTarget` | The current service session's runtime NPC identity |
| Framing | `sharedassets0.assets` MonoBehaviour 1493 and 1481: distance 1.3, world height 0.55, yaw -20, Neck target path | Explicit native-basis yaw, root fallback for NPCs without the player-avatar neck resolver, LookAt before world height |
| Combi waiting framing | Independently recovered MonoBehaviour 1495: distance 2.2, world height 0.42, yaw 0 | This surface must not reuse the primary portrait's close-up framing; separate regression test |
| Camera projection | Camera 1492: FOV 45, near 0.2, far 5 | Independent per-surface targets with the existing published UI Rect dimensions; alpha-cleared target replaces framebuffer depth-only rendering |
| Visibility/lifetime | `AvatarUtil.DrawCamera` is reached by the primary and waiting GUI branches | No waiting camera outside waiting; late mesh admission, stable handles while open, release on target loss or mode close |
| Shared code-entry owner | `Panel_PCStuffScript.DoRedeemWindow`, window 99; `CnGuiChat.GetInputGUIStyles` | Owner-addressed text input, maximum 32 UTF-16 code units; no implicit protocol authority |
| Overlay/panel | Full-screen black texture tint alpha .75; centered 526×164 direct texture | Shared modal tree, reused `CSDeleteWindow` payload |
| Text/control geometry | Title (176,15,173,17), instruction (118,36,288,17), input (75,73,378,24), cancel (40,125,152,27), submit (335,125,152,27) | Reuse existing Buddy adapters for the same Transparent3/DeleteText/Cancel/QuitButton styles and approved fonts |
| Commit | Length <3: Yes_Button, keep open. Space: error box and close. Valid: free-chat `/redeem … ` request, close. Cancel: clear and No_Button | Existing Enchant runtime remains the validator and packet producer; shared surface only returns the draft |

Strict raw object reports are under `work/legacy-sources/whole-client-audit-20260905/`: `combi-camera.evidence.json`, `enchant-camera.evidence.json`, `combi-camera-render.evidence.json`, `chat-input-owner.evidence.json`. The chat owner is MonoBehaviour 1564 and resolves texDelBG 49, texDelWin 97, popSkin 1382 and MonoScript 988 in the same serialized asset.

Additional strict reports `service-{1495,1483,1480,1482,1494,1154}.evidence.json` cover both waiting controllers, remaining camera objects and the resolved `cnCharRenderCamera` MonoScript. All cameras use 45°/0.2/5; Enchant's waiting controller matches its primary values, while Combi's waiting values differ as recorded above.

The old GUISkin is serialized as MonoBehaviour. Strict traversal of skin 1382 cannot resolve its built-in script in `library/unity default resources`; it was not accepted as a fresh complete pointer graph. No new texture/font payload was published. The current native Buddy adapters are reused because the recovered code requests those exact shared style roles. A new complete 20260821 skin/pixel comparison remains outstanding.



## Help and popup evidence

Additional managed evidence from the same hashed primary assembly:

| Owner | Recovered behavior | Native follow-up |
| --- | --- | --- |
| `cnVendor.ClickHelp` | `(2,21),24` | FirstUse entry 24 selects absolute Help page 25 |
| `cnGuideMode.ClickHelp` | `(2,21),44` | FirstUse entry 44 selects absolute Help page 5 |
| WorldMap help | `(2,21),15` | FirstUse entry 15 selects absolute Help page 23 |
| `cnHelpMode.InitMode3` | FirstUse `m_iHelpMain/m_iHelpSub`, subtract selected topic's `m_iTitleStartString` | Validated native mapping; unknown entries do not redirect to a fabricated page |
| `EquipPopup.ShowPopup` | 310×449, inventory (610,60), outside (200,60), positive integer viewport offsets from 1020×638 | Preserve separate purchase/inventory placement; no centered substitute |
| `GumPopup.ShowPopup/InitCacu/CacuNums` | 310×355, inventory/outside y90, calculator (87,162,136,109), decimal digits capped to maximum, zero initially | Separate bounded UI draft, no packet or inventory authority |
| `EquipPopup` buyback | EquipPopup even when restored item is General; button (160,380,130,28) | Preserve layout and BUY BACK label |
| Shared buttons | Existing native item-popup button style, border 6/6/6/4 | Reuse published payload and nine-slice contract; initial stretched-button capture rejected |

## Bank reachability

InventoryManagerScript.OneClickItem routes Bank slot types 1/3 to SendBank; SendBank chooses the
first empty opposite slot. Right-button release is handled in its slot control path. Left drag
routes explicit source/destination locations through PutItemInEquipmentSlot and the bank target.
Native reachability is NpcIconMode service categories 12/50 -> gameplay_ui_actions/npc.rs ->
BankUiState.begin_open; OpenFusion Items.cpp registers itemBankOpenHandler and item move handling.
This is an implemented gameplay service, not an asset-only historical mode.

## Bank Help and redeem

Primary `cnBank.ClickHelp` sends FirstUse3; native table main6/absolute24/relative0.
`Panel_PCStuffScript.DoRedeemWindow` is shared, not dependent on an Enchant session.
OpenFusion CustomCommands registers `/redeem` at access100. Preserve 3..32 UTF-16
units, literal-space rejection and the exact trailing-space free-chat request.
The reusable source geometry/styles are in the recovered-contract table above.

## Bank popup evidence

`PopupControll.OpenPopup` case 1 selects TuringPopup for
chests and EquipPopup for other equipment; **GeneralPopup case 1 also selects EquipPopup**.
The bank general-item path therefore has no GumPopup calculator or stack splitting.

| Reachable role | Primary code | Native geometry/action |
| --- | --- | --- |
| Equipment and general inspection | EquipPopup.ShowPopup/UpdateBut, popupType 1 | 310x449; inventory x610 / bank x200, y60; positive integer half-offset from 1020x638 |
| Chest inspection | PopupControll case 1, TuringPopup uiMode 1 | 310x235; same x, y90; bank transfer takes precedence over OPEN |
| Inventory -> vault | EquipPopup.rectBankBut / TuringPopup.rectBankBut | (140,380,150,28) / (140,180,150,28); full-stack SendBank |
| Vault -> inventory | same, out branch | x-70, width+70; first empty opposite slot |
| Close | popup close callback | close inspection; keep bank open |

## Bank deletion source

EquipPopup.UpdateBut enables Trash only for eIn inventory, with uiMode 1 allowed; rectTrashBut
is (10,375,32,32). TuringPopup uiMode 1 likewise enables inventory Trash at y180, while the vault
branch offers transfer only. cnBank.DeleteItem opens message 153, copies the selected icon and
uses iOpt as iDeleteNums for General items. DeleteItemOk sends inventory location 1 / selected
slot, packet 318767129, and enters the send lock. No deletion from vault slots is authorized by
this source path.

## Vendor chest source

PopupControll.cs
case 3 routes non-buyback Chest to TuringPopup.ShowPopup(0, 2, inout, ...). TuringPopup.UpdateBut
uses OPEN for popupType 2/inventory, rectEquipBut (160,180,130,28), Container_Open01 and SendUse(0).
Bank mode takes precedence and retains transfer. ChestCheck==0 triggers FirstUse 8; it does not
remove OPEN. FirstUse integration is outside this action slice and remains unresolved.

## Card text identity

Primary managed evidence remains retrobution-20260821. EquipPopup.TryOn requires an independent
avatar copy, item-slot appearance replacement, camera distance 2.2, stand animation and a separate
215x375 panel. It remains unimplemented; no inert TRY ON button was added. This pass instead
corrects the confirmed shared card text defect: native semantic content-row resolution had been
restricted to chests. EquipPopup and GumPopup consume cnItemDisplayInfo name/description; general
card description geometry remains (12,95,280,40). Existing native general-card uppercase styling
is retained as a post-localization component, with removal on the equipment/chest transition.
Native output contains no new binary assets or legacy identities. EN/RU binding and GPU acceptance
results follow after execution.
Try-on follow-up contract: TryonRender uses +/- smoothDeltaTime * fSpeed for rotation, unlike
UserEquip's frame-step rotation; reuse of its UI rotation behavior would change source timing.
EquipPopup shifts the item window left by 200 while preview is open and restores it on preview
close. The current native sync_native_player_preview owns selection/creation/UserEquip and has
no Vendor branch; a future extension must explicitly select a Vendor appearance override and
independent presentation state without mutating inventory or the world avatar.

## Try-on source

Recovered current primary sharedassets0.assets GameObject 1283 TryOn -> Camera 1431 and script
1432. Script stores fHeight=.7, vAngle=(-10,0,0), distance2.3; EquipPopup.TryOn overrides distance
to2.2. AvatarUtil.DrawCamera renders on GUI repaint, so the serialized fRenderInterval does not
justify throttling this direct render. Source panel215x375/render210x375/offset(0,40), card shift
-200; left/right RepeatButtons use +/-100*smoothDeltaTime. EnableTryEquip checks appearance sex
and mentor, including combined upper option bits, but ignores level. Source inventory/buyback,
General/Chest and vehicle paths do not expose this Vendor action.

## Equipment-card combined badge and vehicle fields (20260906)

Evidence question: which equipment-card overlays and detail rows are actually drawn for combined
items and vehicles in primary Retrobution 20260821, consumed by Bank/Vendor item popups?
EquipPopup.cs SHA256 3d0db1a1034eb1f1c56d5b96c981fa817aaea352bc69195f61e0803543775ad4,
from the established hashed main assembly above, DrawWindow lines 1639-1648: item types through
Foot (0..3), combine enabled and positive signed-short high option word draw the combi style at
icon+(36,36), size 26x26. Native card icon starts at (16,16), so badge is (52,52,26,26).
The already-published user-equip/combined.png and its existing byte contract are reused unchanged;
no payload publication or Enchant branch is part of this change.
DrawWindow lines 1624-1627 and 1683-1693 omit equipinfo and three combat ratings for Vehicle.
Lines 1747-1763 replace Range with Speed and display m_iUp_runSpeed plus Class. Native item detail
already carries vehicle_speed_class; both cards now bind semantic localized Speed/Class keys,
restoring Range when the same entities switch back to ordinary equipment. Status is also hidden
for vehicles. Rental dates/durations remain a separate open slice, not accepted by these changes.
Acceptance requires production binding transitions (combined, vehicle, weapon, invalid high word,
general, chest), EN/RU text resolution, and Bank/Vendor GPU frames for combined and vehicle cards.

## Equipment type/range and combined text correction (20260907)

Primary authority remains the hashed Retrobution 20260821 main assembly above.
cnItemDisplayInfo.cs sha256 f0807e9b65b985226656b7dca53a4601405c92b7c2f02bd93e0fb196c003a56b,
lines 125-158 map protocol types 1..6 to Body, Legs, Shoes, Hat, Glasses, Backpack.
The native helper incorrectly used equipment display-slot order. Its three consumers (UserEquip,
Bank, Vendor) now use protocol item IDs. Five new semantic EN/RU labels preserve existing slot
labels and fonts. In the combined branch lines 160-180, the base row supplies level/ratings while
appearance row supplies name/comment; Bank/Vendor now select the appearance text identity without
changing base statistics. Lines 182-210 and 258-286 restrict range to item type 0; Bank/Vendor
previously applied the weapon equip-type map to clothing. UserEquip already had this range gate.
Native byte payloads unchanged. Long-title fitting and rental periods remain separate open work.

## Item-card completion pass (20260907; acceptance pending)

Scope: finish the previously open Bank/Vendor/UserEquip equipment-card presentation, including
rental duration/absolute expiry, timed ordinary items, long localized names, price, combined
metadata and mode/language transitions. Existing transaction owners remain unchanged.
Primary EquipPopup.cs (hash above) Init/DrawWindow establishes name (82,16,170,40), level y50,
comment (12,95,280,40), rental title (10,138,200,20), rental value (5,150,300,100), price y348.
The catalog Vendor eOut/popupType3 path interprets positive vehicle time_limit as duration, omits
zero day/hour/minute/second components and appends From time of purchase. Other vehicle cards
interpret it as Unix absolute expiry converted to local time. Nonvehicle timed items append the
local expiry to the item name. Native timestamp conversion uses chrono and displays an explicit
UTC offset. The native localization adapter uses numeric day.month.year, 24-hour time, and places
ordinary item expiry below level at (82,72,210,20), keeping the source title and primary controls
unchanged and avoiding the source name/date overlap. This is an explicit readability adaptation,
not a claim that the original date string or overlapping layout is pixel-identical.
Names use existing UiTextAutoFit inside the original width, capped before the level baseline;
Bank/Vendor allow 31 px, inventory child 28 px after its existing 3 px source padding. Descriptions
fit inside their existing 280x40 content rectangle. Price is a localized centered amount/currency
line at y348; the primary coin/price-label decoration is represented by the semantic currency text.
This avoids new unverified image extraction, but is an explicit presentation adaptation.
Inventory now resolves key-first item name/comment and combined appearance icon/rarity/trade,
matching the already-corrected Bank/Vendor bindings. Combined badge is present in the inventory
card. Level is yellow, unavailable range blue, and trade red/green in all equipment-card surfaces.
Try-on arrows now use their existing source hover-state textures as well as their normal states.
No binary payload publication in this pass. Sources/raw assemblies remain in Editor work.

Completion-pass visual correction: the first price candidate accidentally referenced the existing
full Taros counter atlas. Review rejected it; the final code selects the existing Vendor TarosIcon
payload used by the source vendor image contract. The amount is centered with a 5px gap and 32px
coin at price y348 (32px row y343), replacing the earlier candidate currency-word line. No new
payload or visually similar asset was published. Inventory-specific content offsets (+14 Equip,
+13 Unequip) are retained for rental/expiry and combined badge. Decorative card labels and images
pass Bevy focus, including action-button text; real-pointer EQUIP acceptance is required below.



## Historical verification

Source reports/hashes: `artifacts/shared-ui-owners/manifest.json`. Native checks and
capture hashes: the same directory's runtime, vendor-help, bank-pointer, bank-shared-redeem,
bank-item-popup, bank-delete, vendor-chest, vendor-card-localization and try-on acceptance
JSONs. Later item-card completion supersedes earlier rental/long-name pending notes.
These were native/offline checks, not live-shard transactions or complete Unity golden
acceptance. Unresolved built-in references remain triage; prior failures were not made
passing by this documentation cleanup. Use the matching record only for the selected issue.
