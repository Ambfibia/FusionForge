# GameFrame combat and infected-zone primary evidence

This ledger covers the native combat overlay and the environment transition
contract used by `FFOneClient`. Clean `retrobution-20260613` remains the
behavior and serialized-asset authority. The patched project is used only for
its reusable extraction cache.

## Source ownership

- Raw primary container: `primary/main.unity3d`, 7,000,415 bytes, SHA-256
  `59788201962b6a1737b114486c361fe74eef69f507d1d125ca3171377eec602f`.
- Serialized asset: `sharedassets0.assets`, 45,759,086 extracted bytes.
- `GlobalManager.MainFrame` resolves to GameObject `GameModeManager`, PathID
  1304. Its `GameFrame` MonoBehaviour is PathID 1474, script PathID 1075.
- The serialized `CombatTexture` reference is Texture2D PathID 384, Unity name
  `combat`, DXT5, 256x128, bilinear/clamp, one image and no mipmaps. Exact
  source payload: 32,768 bytes, SHA-256
  `fccf841795de32519608f8a90814ecc1ee7e568f0601903aa2974e714b729800`.
- The published PNG is
  `assets/game/ui/gameplay/shared/combat-frame.png`, 1,947 bytes, SHA-256
  `ecbeed2fbc2104a1ae4d4ce56c310faaf545f0bfdf9495972e6ef9ea5a316a36`.
  It is the exporter's vertical-flip-only top-left-origin conversion; it was
  not resized, tinted, repaired or alpha-masked.
- Exact conversion command:
  `fusionforge fusionforge export-exact-texture <primary>/main.unity3d 384 work/legacy-sources/imported/primary/combat-frame-texture-384.json`, followed by byte-for-byte base64 decoding of `payload.dataUrl`.
- Focused serialized dumps are
  `work/legacy-sources/combat-frame-mainData.objects.json`,
  `work/legacy-sources/combat-frame-mainframe.objects.json` and
  `work/legacy-sources/combat-frame-gameframe-1474.objects.json`.

The checked decompilation input is the extracted primary
`Assembly - CSharp.dll`, 1,517,568 bytes, SHA-256
`33d6f70216b1c7ba05bcc0f270fba97e767b129159755af4c8835922e60acadb`.
Decompilation used the vendored FFSpy/ILSpy 7.2 console with
`--preserve-iterator-state-machines`.

## Exact combat presentation and lifecycle

`GameFrame.OnGUI` draws the PathID-384 texture over
`Rect(0, 0, Screen.width, Screen.height)` with `GUI.depth = 2` only while the
global `eC_Combat` condition is active and no tutorial event owns the frame.
Before `GUI.DrawTexture`, white alpha is multiplied by
`(sin(Time.realtimeSinceStartup * PI * 2) + 1) * 0.5`. The two-argument
`GUI.DrawTexture` overload stretches the texture to the screen rectangle.

This full-screen green frame is distinct from `CnGuiMonster_info`'s 302x90
`danger` texture. The latter is a top-center target-panel fallback and is
hidden when the real target info group renders. Substituting or stretching
`danger.png` is not combat-frame parity.

`GameFrame.SendCombatMode` sends `P_CL2FE_REQ_PC_COMBAT_BEGIN` on the inactive
edge, installs the combat condition with the current realtime value, and
refreshes that value on subsequent observations. `EndCombatMode` sends
`P_CL2FE_REQ_PC_COMBAT_END` only after more than five seconds of inactivity.

## Exact infected-zone classification

`cnOwnAvatarStatus.Update` samples `MapAttributeTable.GetAttributes` and
`GetHeight` at the avatar position. Contact is
`cnAvatarAnimation.bWater || abs(avatarY - terrainHeight) < 0.5`. For ordinary
attribute values below 64, bit `0x08` enables poison and bit `0x10` enables
healing. Both are suppressed while `cnAvatarAnimation.iVehicle != 0`. The
clean call order is `EnterHealArea` followed by `EnterPoisonArea`, and each
transition owns an independent one-second gate.

The water renderer/material name, including `ffPoison`, is not an independent
damage flag in this method. It supplies water-contact evidence while the
terrain attribute sidecar remains poison/heal authority. Native water meshes
do not retain Unity trigger volumes, so FFOne uses a documented bounded-depth
surface approximation. It accepts a player first observed already submerged;
the previous implementation incorrectly required an already-true
`in_water` state for that branch.

## OpenFusion compatibility divergence

Clean `GameFrame.ReceivePacket` case `P_FE2CL_CHAR_TIME_BUFF_TIME_TICK`
dispatches time-buff ID 17 to `Status.BuffTimeTick` without calling
`SendCombatMode`. The local OpenFusion server's four-second natural-heal path,
however, checks only `Player::inCombat`; it does not exclude an active
`ECSB_INFECTION` environment buff. That allows a heal to race the two-second
goo-damage cadence.

FFOne therefore treats each validated local infection tick as activity on the
existing five-second combat lease. This is an intentional server-compatibility
extension, not a claim about clean-client call ownership. It prevents natural
server healing while infection ticks continue and emits the ordinary combat
end edge after the player leaves the zone and ticks stop.

## Native acceptance

`FFONE_MISSION_UI_PREVIEW=danger cargo run -p ffone-client --example gameplay_hud_gpu_preview -- target/ui-parity/combat-frame-danger-1264x681.png`
produced the 1264x681 GPU frame with both the full-screen combat texture and
the independent top-center danger group visible. The PNG is 194,374 bytes,
SHA-256
`6c2a3ce2f7780dca450eb89d91a7b5a4ed3016be8db8421b80d9356238ef26e5`.
The real Dev binary was rebuilt after the implementation and published as
`target/debug/ffone-client.exe`.
