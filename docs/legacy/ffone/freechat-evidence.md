# FreeChat UI and player bubble evidence

This slice uses source alias `primary` for all behavior, ownership and visual
acceptance. The patched workspace is only a navigation/extraction cache and is
never opened by the native runtime.

| Evidence | Source | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| primary bundle | `builds/retrobution-20260613/main.unity3d` | 7,000,415 | `59788201962B6A1737B114486C361FE74EEF69F507D1D125CA3171377EEC602F` |
| primary managed owner | `Assembly - CSharp.dll` in `main.unity3d` | 1,517,568 | `33D6F70216B1C7BA05BCC0F270FBA97E767B129159755AF4C8835922E60ACADB` |

The focused decompile is retained at
`work/legacy-sources/nanocom-primary-code/Assembly - CSharp.decompiled.cs`.
`CnGuiChat.initLargeChatStyle`, `DoLargeWindowChat`, `Chat`,
`ReceiveChat`, `ReceiveGroupChat`, `ReceiveBuddyChat` and `AddChatString`
own the compact geometry, channel routing, history and message-color type.
`PrintName.SetChatBubble`, `PositionUpdate` and `DrawBarker` own player bubble
replacement, sizing, timing, projection and skin selection.

## Native visual contracts

`BubbleChatSkin` is primary `sharedassets0.assets` MonoBehaviour path ID
`1386`. Its `fbBox` style references Texture2D path ID `370`, `fbubble`
(22x19), with a six-pixel border on every edge, ten-pixel padding,
middle-centre alignment, word wrap and black JEFFE text. Its `fbLabel` style
references Texture2D path ID `198`, `fb_009` (5x21), and is drawn as the
5x20 tail centered at `ChatRect.x + width / 2 - 4`.

| Native asset | Primary object | Raw serialized texture bytes | Published PNG |
| --- | --- | --- | --- |
| `../FFOneClient/assets/game/ui/gameplay/speech/freechat_box.png` | `sharedassets0.assets:370`, `fbubble` | 480 bytes; SHA-256 `4CF81918AEA891B1B19DC21FE3E9DA5C76C54248143DE4E5E153EA56C9DA40C6` | 649 bytes; SHA-256 `929F181E84A045D649CB965CD5DA65441E53EB15BE4AE9F0BE50F3D63D220EB2` |
| `../FFOneClient/assets/game/ui/gameplay/speech/freechat_tail.png` | `sharedassets0.assets:198`, `fb_009` | 192 bytes; SHA-256 `0D4F03F51C0116D689833A99909DBDB495E21CBF64729676BC78840518735F25` | 276 bytes; SHA-256 `044EC70857910CE71AC6F6A281D06FDA033CFA30929316F33733F02E47E507E7` |

Focused exact reports live below
`work/legacy-sources/freechat-bubble-primary-v1`. Publication is reproducible
with:

```powershell
cargo run -p ffone-asset-pipeline --bin publish_exact_texture -- `
  work/legacy-sources/freechat-bubble-primary-v1/fbubble.export.json `
  ../FFOneClient/assets/game/ui/gameplay/speech/freechat_box.png
cargo run -p ffone-asset-pipeline --bin publish_exact_texture -- `
  work/legacy-sources/freechat-bubble-primary-v1/fb-label.export.json `
  ../FFOneClient/assets/game/ui/gameplay/speech/freechat_tail.png
```

There is no visual substitution and no intentional source divergence. The
approved Cyrillic-capable native replacement `fonts/jeffe.otf` remains the
runtime font; the serialized Unity Font path ID `903` is metric evidence only.

## Behavior contract

- Only accepted ordinary FreeChat success calls the `FreeChat` bubble route.
  Buddy and ALL GROUP FreeChat update their histories but do not create a
  world bubble.
- Empty text and exactly one ASCII space are rejected. Other whitespace is
  preserved, matching `SetChatBubble`.
- A player has no `NpcMoveController`, so a new message clears/replaces that
  player's current `ChatList`; it is not an NPC-style FIFO.
- Width is clamped to 100..300 pixels. Lifetime is
  `max(7 seconds, rendered height / 5)`. The player anchor is the exact
  `CharacterController.height * 0.9`; native height is the primary prefab's
  validated 1.6.
- The balloon display option gates creation and rendering. Player-authored
  text enters `LocalizedText` only through the `ui.content.passthrough`
  template argument.
- General, Buddy and Group rows resolve the live persisted `cnTextOption`
  palette indices. NPC is white, Receive is `(red + yellow) / 2`, Damage and
  the default/system path are red, and Attack is blue.

The adjacent `MenuChatScript`/MenuChat protocol and its menu/emote selector
remain outside this FreeChat-only slice by request.

## Deterministic GPU acceptance

`gameplay_hud_gpu_preview` was run at the native reference resolution with
`FFONE_FREECHAT_PREVIEW=1`. Its message is the exact Computress copy from the
supplied FreeChat crop; color types remain covered independently by the model
and production-ingress tests. The capture exercises ALL/BUDDY/GROUP, focused
input, the source-overlapped SEND chrome and a projected player bubble:

- `target/ui-parity/freechat-reference-final-text.png`, 151,127 bytes,
  SHA-256 `2227A55A4B0636E5CDAADCB5326CFCAB3FF12C7C73B031764BA5F3C8A5B7EB34`;
- `target/ui-parity/freechat-reference-final-text-crop.png`, 12,194 bytes,
  SHA-256 `6C80B1DA09ABB35876646DBF50F12E7760201AC57CE26483BACB211E2245CA53`.

The primary `DoLargeWindowChat` call order draws SEND before the text field;
the `(78..366)` field therefore hides the first ten pixels of the
`(356..428)` button. Native child order now preserves that overlap. Centered
JEFFE path ID 903 uses the established 14 px horizontal / 0.70 vertical
fixed-raster adapter with 13.71 line height. Chalet Small path ID 1018 uses
the primary-crop calibration of 11 px, 12.072 line height and a three-pixel
bearing offset. In the aligned crop the native Computress text bounds are
`x=8..361, y=36..58`; the supplied reference is `x=8..359, y=36..58`, with
the same line break.

The real `ffone-client` binary is also rebuilt with the Dev profile after the
capture and protocol-to-entity integration tests.
