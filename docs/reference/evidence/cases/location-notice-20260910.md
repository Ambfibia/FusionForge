# World location arrival notice

Question: why does FFOne update the minimap location without displaying the
arrival text? Consumer: gameplay HUD projection and the existing combat-mode
notice renderer. Authority: primary Retrobution 20260821; no donor extension.

## Evidence

Rehashed raw `main.unity3d`:
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
Rehashed exact `level0/Assembly - CSharp.dll` (1,762,816 bytes):
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The extraction record is `work/cases/mob-reward/managed-evidence.json`.
Fresh focused decompilation:

```text
dotnet run --no-build --project vendor/FFSpy/ICSharpCode.Decompiler.Console/ICSharpCode.Decompiler.Console.csproj --framework net6.0 -- work/cases/mob-reward/managed/Assembly-CSharp.dll -t cnDisplayMapName -o work/cases/location-notice-20260910
dotnet run --no-build --project vendor/FFSpy/ICSharpCode.Decompiler.Console/ICSharpCode.Decompiler.Console.csproj --framework net6.0 -- work/cases/mob-reward/managed/Assembly-CSharp.dll -t WorldDataContainer -o work/cases/location-notice-20260910
```

Owner and style evidence are already accepted in
`artifacts/retrobution-ui-completion-20260907/combat-notice.json` and
`artifacts/mob-rewards-20260908/styles.json`: `main.unity3d`,
`sharedassets0.assets`, MonoBehaviour 1620, GameObject 1361, MonoScript 1030,
GUISkin 1372, BigFont16, font 1012, centered padding 10/6/4/6, no background.
The existing native JEFFE adapter and shadow are reused without asset edits.

## Native contract

- `WorldDataContainer.Update` waits for ready-for-play, detects first region
  and region exits, and sends `cnMainGame.ReceiveNewMapName`.
- `cnDisplayMapName.ReceiveNewMapName` suppresses tutorial mode; accepted world
  arrivals replace the same display slot used by combat-mode messages.
- Serialized lifetime is two seconds. A single global random draw selects one
  of eight horizontal/vertical straight/wavy trajectories. Preserve both sine
  easing stages, the literal 3.14, fade, UTF-16 width, and +1/+1 black shadow.
- Native presentation observes the final HUD location after its production
  position projection and loading-screen synchronization. It remembers player
  owner plus location, resets at non-world/session boundaries, and leaves an
  arrival pending during loading. Stationary frames do not restart animation.
- Text uses existing semantic `content.location.world.*` EN/RU keys. Language
  changes resolve the active notice again without manufacturing another arrival.
- No texture, font, world geometry, visibility distance or server state changes.

## Verification

Focused ECS tests cover the eight motion branches, localized text identity,
expiration, combat replacement, tutorial suppression, loading deferral,
unchanged-location suppression, character changes, and reentry. GPU fixture:
`FFONE_LOCATION_NOTICE_PREVIEW="Sector V"` with `gameplay_hud_gpu_preview`.
Runtime and capture results will be recorded after execution.
