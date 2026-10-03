# Player emote continuation and audio

Question: why do native beach/dance emotes return to idle after one clip, and
why are player emote voices and dance accompaniment silent? The native consumer
is the selected-player animation adapter and GameplayAudioRuntime. Acceptance
requires repeated beach/dance cycles, movement cancellation, source event times,
and semantic EN/RU audio resolution.

## Primary contract

Authority is primary Retrobution 20260821, `main.unity3d` SHA-256
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`.
The exact CSharp and firstpass assembly payloads were verified against their
managed evidence records. The scoped event records, managed records and hashes
are in `evidence/cases/artifacts/player-emotes-20260908`.

`cnAvatarAnimation.EndAnimation(string)` does not end every emote:

- Beach sends the same 22/23/24 emote code once, sets `bEmoteSend`, and queues
  the same clip with a 0.15-second crossfade.
- An owned dance chooses uniformly among codes 6, 17, 18, 19, 20, sends that
  request once, and waits for the server echo. `AvatarEmote(int)` clears the
  pending flag and starts the authoritative clip with a 0.15-second crossfade.
- The pending flag prevents repeated requests every idle frame. Movement/jump
  interrupts the emote through the existing EndEmote path.
- Ordinary emotes finish at their `end` animation event and use the existing
  0.3-second return fade. The trailing GLB recovery keys are not the end clock.

The 46 gender/clip event contracts were exported directly from the raw
`CharacterSelection.resourceFile` with exact serialized asset
`CustomAssetBundle-ce09c4c9be8a046ca92e0044f22d1b99`, type AnimationClip, and
gender-specific object IDs. Every selected object name was asserted, and every
clip has exactly one end event. Sound event order is retained, including the
different order of dance vocal/accompaniment events between clips.

`AnimationEventHandler.sound` calls `PlaySound`; random payload tokens are
expanded at the event. `cnAvatarAnimation.IsDance` suppresses avatar sound while
an owned Nano is present, except for `_SFX_Dance` accompaniment. FFOne preserves
this gate and uses its existing semantic character sound route, including
LocalizedVoice and independent VoiceLanguage for catalogued vocal assets.

No binary payload was changed. Existing catalog entries resolve all 110 sound
references after expanding the declared random ranges. No historical or donor
clip replaced a primary clip. Additive FFR clips are outside this recovered
integer-emote contract.

## Reproduction

From FusionForge (build FusionForge first with `cargo build --bin fusionforge`):

```powershell
fusionforge repair-native player-emote-events --source-root ../builds/retrobution-20260821 --work work/cases/player-emote-events-replay --target-root ../FFOneClient
```

The publisher opens each object through `dump-object-evidence` and publishes only
native semantic sound names and times into the native event table. Source routes,
object IDs, hashes and evidence stay in FusionForge.

Native verification commands:

```powershell
cargo test -p ffone-client --lib emote -- --nocapture
cargo build -p ffone-client --bin ffone-client --example tutorial_player_rig_gpu_preview
target/debug/examples/tutorial_player_rig_gpu_preview.exe assets/game target/performance/player-emotes/male-dance.png 2 1 0 dance male
target/debug/examples/tutorial_player_rig_gpu_preview.exe assets/game target/performance/player-emotes/female-dance.png 2 1 0 dance female
target/debug/examples/tutorial_player_rig_gpu_preview.exe assets/game target/performance/player-emotes/male-beach.png 2 1 0 beach male
target/debug/examples/tutorial_player_rig_gpu_preview.exe assets/game target/performance/player-emotes/female-beach.png 2 1 0 beach female
```

The GPU fixture acknowledges only continuation requests emitted by the production
adapter, observes two renewed cycles, captures the pose, then injects movement
and requires release to locomotion. It simulates server echoes and does not claim
live shard or network-latency acceptance. Audio tests enumerate every random
variant and run the actual AudioPlayer spawn path in both EN and RU.

## Verification results

- 37 tests selected by `emote` passed, including production EN/RU audio spawns;
  all 24 selected-player rig tests also passed (the filters overlap).
- The real `ffone-client` development binary and GPU fixture built successfully.
  Native `--validate-assets` returned `status: ok`; the full-client offline world
  fixture exited successfully. Its timing output is not an FPS comparison.
- Both genders completed two dance renewals and two beach3 (code 24) renewals
  with delayed fixture echoes, no duplicate in-flight request, and movement
  returning to `run`. The earlier beach1 fixture also passed.
- Capture timing uses the renewed clip's 1.0–1.5 second interval to avoid
  recording the outgoing final sample or the first frame of the echo transition.
- A second strict export into Editor staging reproduced the native Rust event
  table exactly: SHA-256
  `cd0b467d93b93c49d1343f1578f28295107d3d843518ce4e4b85407a24486b9a`.

Native logs and captures are below `target/performance/player-emotes`; their
acceptance hashes are recorded in `docs/reference/evidence/cases/player-emotes-20260908.json`.
