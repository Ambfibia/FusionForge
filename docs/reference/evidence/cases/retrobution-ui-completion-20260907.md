# Retrobution 3.0 UI: accepted corrections, 2026-09-07

Historical evidence: source retrobution, primary20260821; June13 is comparison only,
not a donor substitution. Raw main.unity3d=8,221,718 bytes,
SHA-256 `01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
Assembly owner dumps and request/result records are in
`artifacts/retrobution-ui-completion-20260907/`. FFGUIUtility damage history belongs
to firstpass, not the main assembly. No required work/cache/replay pipeline is defined
by this historical record. Runtime screen contracts are indexed by FFOneClient
`docs/native-ui.md`; use this table only when investigating these accepted changes.

## Accepted native behavior contract

| Request | Proven owner and native implementation |
| --- | --- |
| Red damage history | FFGUIUtility: hold 0.4 s, Lerp speed 5, snap below .001; repeated hits do not restart the hold. Player, target, Nano info, three wheel slots, group PC and escort bars use the same native helper. Rebinding and hidden ancestors reset the history; every placement remains independent. |
| Three-button exit dialog | cnQuit.OnGUI: height188; Change Character y16, Quit y69, Cancel y122. The old logout action can remain a compatibility event, but has no rendered control. Button visuals resolve by kind after removal. |
| F1 notice | cnAvatarAttack + cnDisplayMapName: existing F1 gates and sound; two-second horizontal slide, source sine easing, enabled/disabled direction, quarter-screen height and fade. Exact English strings "Combat mode enabled" / "Combat mode disabled", keyed EN/RU text and shadow. |
| Creation colors | CnCharCreationMode, cnGUICharCreation, UserContainer: skin36 creation/48 total, hair54, eyes10; pages12/18/5. Initial random skin is first page; source integer exclusive upper bounds retained. Fifty new eye images cover five faces for both genders. Runtime palettes are shared with actual player appearance. |
| Map filters | WorldMapMode: twelve exact icon groups; only World View is filtered. My View and minimap retain all icons. Mission question marks precede available-mission exclamations. |
| Transportation | WorldMapMode/TransportCache: exact warp/Monkey location tables, registration bits, destination rows and Monkey move-type2 gate. Hover has enlarged/top-layer icon and colored outline; available curved routes pulse in green/purple (Woosh peach), unavailable segments are dashed/translucent. Unregistered map icons use a grayscale image variant. |
| Custom waypoints | WorldMapMode: right-click add/remove, six reusable HSV hues, local settings persistence, world-map and minimap projection. User waypoints are independent of mission markers and retain the current player's height. |
| Respawn highlight | Existing authoritative map/XCom lookup, source nearest-NPC selection, animated green/pink spiral behind the selected Resurrect'Em. |
| Map drag | Pointer delta normalized by the actual logical picture dimensions and target view range, independent of frame delta. |
| Minimap | Native cnGUINanocom ratio range4..16, default8; +/-1 controls at130,68 and130,92, with17x16 images centered in18x18 hit boxes. Mission availability does not depend on the old mission-finder preference. |
| Offset corrections | Mission banner y31; mission portrait content offset+1; both inventory fades3,34,350,503; guide confirmation art source+2; enemy type icon runtime override25,55,21,21. |
| Text and color | Panel_Cashmall spelling correction; five other string cleanups already present. Panel_UserClothes Adaptium color005AFF also feeds the Nano station. |
| Service cameras | Existing production service-portrait layer ownership covers the requested NPC only; separate case `vendor-portrait-20260907.md` verifies all five slots and final EN/RU Vendor frames. No camera/model/material clone added here. |

World-map icon variants are cached over the finite resident UI set, with no idle
pixel rescanning. Damage bars reuse their source image, keep independent state,
and only dirty changed widths/display/layers. New static text uses semantic
LocalizedText keys; EN/RU key and placeholder parity is mandatory.


## Verification boundary

The retained records describe native GPU fixtures and scoped validation, not a complete
running-Unity or live-shard comparison. Read the selected artifact for historical hashes
and failures; old capture counts are not current test results. The recovery/correction
contract above must not be replaced by texture-name or visual-similarity guesses.
