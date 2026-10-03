# Taros/FM reward panels and full inventory

Question: why are the two currency reward panels and persistent inventory-full
badge absent in FFOne? Native consumer: `gameplay_ui::rewards` and the validated
world reward ingress. Authority: primary Retrobution 20260821, no donor.

The existing native slice implemented only the crate/potion/boost queues and
quest-item notices. The currency and full-inventory branches were missing.

## Recovered contract

Rehashed raw `main.unity3d` (8,221,718 bytes):
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
Rehashed exact assembly (1,762,816 bytes):
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The existing strict owner artifact
`artifacts/retrobution-ui-completion-20260907/combat-notice.json` binds
`sharedassets0.assets` MonoBehaviour 1620 to GameObject 1361, script 1030,
skin 1372 and local texture pointers FM 376, Taros 579, FullInven 526.

Replay of the exact managed assembly:

```
dotnet run --no-build --project vendor/FFSpy/ICSharpCode.Decompiler.Console/ICSharpCode.Decompiler.Console.csproj --framework net6.0 -- work/cases/mob-reward/managed/Assembly-CSharp.dll -t cnDisplayMapName -o work/cases/reward-status-20260910/managed
fusionforge export-ui-interaction-evidence work/cases/mob-reward/status-interaction.request.json --out work/cases/mob-reward/status-interaction.json
```

The new request adds `RewardFM(sP_FE2CL_REP_REWARD_ITEM)` and `RewardFull()` to
the earlier OnOwnGUI/Update/QuestItem/RewardItem roots. Static IL remains a
candidate call inventory; the source below establishes the state contract.
Tracked decompilation, interaction results and hashes are under
`artifacts/reward-status-20260910`.

| Role | Method / geometry | State and timing |
| --- | --- | --- |
| FM | `DoRewardGUI`: 172x63 at W/2-172,H-63-quickOffset | RewardFM sets target to authoritative FM. Current begins at pre-reward balance; if spending lowered balance, clamp current before retargeting. Step toward target by clamp(abs(delta)/5,1,10), on shared >0.1 second clock. Positive reward arms two-second hold/fade. |
| Taros | 170x64 at W/2,H-64-quickOffset, painted after FM | Same counter timing. Preserve the source's asymmetric alpha gate: FM timer selects whether Taros timer supplies alpha. |
| Digits | Nine rects (6+12*i,31,10,20), centerbox3 | Decimal quotient/subtraction, leading zeroes. Reuse accepted JEFFE14 vector font/vertical-scale adapter. EN/RU semantic `{digit}` template. |
| Full inventory | Crate image 178x142 in lane 0; badge 40x41 at 64,66 | Poll authoritative inventory every >1.5 seconds only when crate queue is empty. False-to-true arms original two-second cubic-sine bounce. Remains opaque and visible until a slot becomes empty. Crate notification takes priority. |

All use the existing bottom-center UI scale pivot, optional 40px quick-slot
offset, HUD visibility and settled chat-editing gate. Timers advance while
hidden. Full badge has no text. Source image labels use native size, explicit
Stretch, single mip, bilinear/repeat, aniso 1; source RGBA orientation is
vertically flipped only for PNG top-left origin.

Native inventory projection reads `LocalInventoryRuntime`, independent of
whether the inventory window is open. Tutorial/session exit provides no world
inventory. It uses the runtime's canonical empty-slot predicate. Currency
notifications are fed only after reward item validation and before replacing
runtime balances; ordinary purchases, wallet initialization and repainting do
not synthesize rewards. Existing session reset clears all transient state.

Publication is reproduced twice using `publish-mob-rewards.py`; all six output
payloads match independently and are pinned by receipt
`recipes/native/ui/reward-status-icons-20260910.json`, superseding only the
earlier three-icon receipt. Original three outputs retain identical bytes.

Scope: these changes restore the requested visual branches. They do not add
the separate RewardFull event chat line or currency chat/sound side effects.

## Verification

Nine focused reward tests passed, including currency accumulation, capped FM,
spending between rewards, expiry, inventory polling/crate priority, clearing,
actual ECS node visibility, localization and production payload hashes.
GPU and real development build verification are recorded after completion.
