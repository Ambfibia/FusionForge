# FFR player animation extension

This extension imports selected player-authored clips from the local Unity Web Player cache as offline conversion inputs. The cache is not a runtime dependency.

## Provenance

- Cache root: `C:/Users/ambfi/AppData/LocalLow/Unity/Web Player/Cache/FusionFall`
- Source bundle: `ffravatar_2eresourceFile/CustomAssetBundle-a5cab62bc14b11c43bf3da0b69ecd68b`
- Source bytes: `46,919,064`
- Source SHA-256: `2C5961DDF656034ACDAD8B4C1A1A10E8AC156EAAD2A1484EDB9BB45E0205A091`
- Parity authority is `builds/retrobution-20260821`.

The cached CharacterSelection bundle was compared with the former 2026-06-13 parity build. All 62 supported clips there were structurally identical after normalized JSON comparison. The 2026-08-21 primary adds a female `knockdown` clip; it remains primary evidence and is not silently sourced from the FFR extension. The primary male `standup` object at PathID `34404` was also exported directly from the 2026-08-21 build: it has zero normalized differences from the cached primary clip and 920 structural differences from the former FFR `mOld` replacement.

## Included clips

Every standard runtime animation, including `stand1`, `staying`, `standup`, the height/shape layers, jumps, rifle states, turns, and swimming, is decoded from the primary CharacterSelection source. The separate `FFRAvatar^Base^mOld.fbx` and `FFRAvatar^Base^wOld.fbx` clips remain offline comparison evidence and are never allowed to replace a primary semantic clip.

The runtime publishes only these 14 additive FFR names for both male and female rigs:

- `ffr_dance_02`, `ffr_dance_04`, `ffr_dance_07`, `ffr_dance_08`
- `ffr_dance_10`, `ffr_dance_13`, `ffr_dance_15`, `ffr_dance_18`
- `ffr_dance_19`, `ffr_dance_20`, `ffr_dance_bully`, `ffr_dance_tellme`
- `ffr_emote_catpose`, `ffr_emote_idolpose`

Female extension clips map all 102 required skeletal paths. The male bully source maps 132 paths from its dedicated male FBX. Other female-authored clips retarget to the 68 shared male skeletal paths; female-only skirt, hair, and helper tracks are intentionally discarded.

Matching bone paths alone is insufficient for the thirteen female-authored male
clips. Their local translations and rotations are transferred relative to the
female rest pose into the male rest pose. This preserves male shoulder lengths
and bind orientations. Cubic translation tangents keep their derivatives;
rotation tangents receive the same quaternion correction as rotation values.
The dedicated male Bully clip needs no cross-gender correction.

`fusionforge convert-player-emotes <FFR-bundle> <native-asset-root> --check
--replace-existing` validates this correction directly from the immutable raw
bundle. Omit `--check` to replace only the thirteen existing male curve payloads.
The command rejects locally edited curves and preserves all other native bytes,
clip indices, timing, events, and the female rig. It creates no intermediate
source dumps or staging tree.

Excluded sources include the obsolete standard replacements, custom drone/run experiments, NPC hammer clips, and test clips.

## Runtime transitions

FFR-only custom emotes enter through a 0.2 second Bevy animation cross-fade. The primary tutorial calls `AvatarEmote("staying")`, `AvatarEmote("standup")`, and `AvatarEmote("run")` preserve the original immediate `Animation.Play` dispatch. Natural completion preserves the exact 0.3 second `EndAnimation` return fade; normal locomotion keeps its 0.15 second movement blend.

Both 2026-08-21 primary `standup` clips carry their source `end` event at `2.0833334922790527` seconds (male PathID `34404`, female PathID `34533`). The native bridge must release the emote at that event and begin the 0.3 second `EndAnimation` fade; waiting for the final GLB playback-completion edge leaves the tutorial player on the trailing recovery pose.

The same primary dump proves that landing transitions also depend on source `end` events rather than the imported GLB duration. Male `jumpend` families end at `0.18333334` seconds; female unarmed, stick, pistol, rifle, and bomb `jumpend` families end at `0.21666667` seconds, while female rocket `jumpend` ends at `0.18333334` seconds. Every male and female `jumplandrun` family ends at `0.18333334` seconds. The unarmed reference objects are male PathID `34224` and female PathID `34626`. If the native state machine waits for Bevy's clamped playback node to report completion, `JumpEnd` can remain on its terminal bent-knee pose indefinitely after the tutorial stand-up sequence.

The tutorial startup ordering can make `JumpEnd` the authoritative destination of `Standup`'s `EndAnimation` return: collision becomes grounded while the direct emote still owns the visible avatar. Ordinary locomotion requests are correctly discarded during that ownership interval, so the native return must recreate both the `JumpEnd` playback request and its pending completion record. Recreating only the visible clip loses the event consumer; the clip then clamps at its last key even though its primary `end` time is known.

## Rebuild

The extracted source JSON is kept under the ignored offline import workspace. Rebuild the native assets from the current primary container with:

```powershell
cargo run -- fusionforge dump-object ..\builds\retrobution-20260821\CharacterSelection.resourceFile all work\projects\retrobution-ui-20260821.ffclient\reports\avatar-animation-primary\CharacterSelection.objects.json
cargo run -p ffone-asset-pipeline --bin publish_player_rig_animations -- ..\FFOneClient\assets\game work\projects\retrobution-ui-20260821.ffclient\reports\avatar-animation-primary\CharacterSelection.objects.json work\ffone\legacy-content\imported\ffr-cache-20260722\ffravatar\player-clips
```

The publisher transactionally refreshes both skeleton GLBs and the domain-owned
`player_rig_contract.json`; the release graph captures their resulting identities.
