# Attack during avatar emotes: primary managed-code investigation

Question: does firing immediately cancel a dance/beach emote, and does a rejected
attack attempt cancel it? Source role: primary (Retrobution 2026-08-21).
Native consumers: avatar_action.rs and tutorial_player_rig_runtime.rs in FFOneClient.

## Evidence identity

Reuse the exact-entry extraction receipt at
`../../../docs/reference/evidence/cases/artifacts/nano-travel-20260905-assembly.json`.
Raw container: `main.unity3d`, SHA256
`01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
Entry: `level0/Assembly - CSharp.dll`; payload SHA256
`0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
The cached assembly hash was checked again for this investigation.
Decompiled files below
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/decompiled`:

- cnAvatarAnimation.cs SHA256 `8EF6A22501C8E164BCEB5E95F3E2C3401BCA3A75ABB52A62A4F771ED837805E5`.
- cnAvatarAttack.cs SHA256 `6E799C6D799429BC81ACB86AF0A9625B34CCC4D3409E7765D1C8E6CB20D9767C`.

## Proven control flow

1. `cnAvatarAttack.AttackTarget` (1437 onward) has cooldown, weapon-change,
   special-state and accumulation guards. It does not test bEmote. Normal fire
   calls MakeBullet even without targets, then AvatarAttack; rocket fire calls
   AvatarAttack only when shootable. Grenade preparation calls AvatarGrenadeReady,
   while FireGrenade calls AvatarAttack after sending its packet.
2. `cnAvatarAnimation.AvatarAttack` (1412-1475) does not call EndEmote.
   It crossfades the full attack only if the main animation name contains stand
   or ready. It always tries the upper attack animation. A dance/beach main
   animation therefore does not select that full-body branch.
3. `CrossFadeQueued` (493-517), despite its name, actually calls Animation.CrossFade
   immediately, stores upperAniState and records the current main name in PrevAniName.
   It does not clear bEmote or restore the hand attachment.
4. `EndAnimation` (690 onward) returns immediately while bEmote && bEmoteSend.
   Otherwise it can consume a matching PrevAniName without ending the emote.
   Its ordinary completion path clears the upper state and calls EndEmote (765).
   Thus an attack completion can clear the emote, but cancellation on the input
   frame is not the primary contract and cancellation is not unconditional.
5. EndEmote restores the hand when not in a vehicle, clears both emote flags,
   disables the facial emote and asks the current Nano to stand.
6. AvatarGrenadeReady explicitly refuses its ready transition while bEmote.

## Native comparison and limits

FFOne accepts gameplay attacks during emotes, but
`bridge_legacy_avatar_visual_requests` drains and discards all ordinary visual
requests while adapter.emote_state is true. This suppresses even the upper attack
that primary attempts to play. The native action resolver additionally derives
the full-body decision from locomotion, which does not represent the active emote.
This is a concrete mismatch; blindly allowing the full batch through would not
restore the primary branch behavior.

An immediate full cancellation on every accepted shot is a reasonable possible
product change, but would be an explicit extension, not recovered primary parity.
A rejected attempt does not reach AvatarAttack in the examined source guards.

This document records a code investigation only. No runtime code or payload was
changed for this follow-up; no Unity playback or new GPU test was performed.
Exact rendered mixing and event order remain unverified in this investigation.

## Subsequent owner-authorized extension

The owner subsequently requested changing the code after reviewing this finding.
FFOne now treats a local accepted AttackFull/AttackUpper command as an immediate
player-emote cancellation. This is an explicit extension over primary, not a
revision of the primary conclusions above. Scripted staying/stand-up/forced poses
remain protected. Rejected attack input does not generate the command that
triggers cancellation. The normal centralized combat state owns the replacement
base and upper animations and their completion events.

Cancellation restores hand visibility, clears the emote cursor, discards local
queued repeat requests and queued player-emote poses, and marks successfully sent
continuation requests so their later matching server replies are consumed without
restarting the old emote. Reply suppression is consumed once, and cleared with the
presentation queue on session/scene reset. No wire ABI or binary assets change.

Native regression coverage is in tutorial_player_presentation.rs and
tutorial_player_rig_runtime.rs. The existing dance/beach GPU fixture accepts
FFONE_EMOTE_ATTACK_PROBE=1 to inject a real attack input after two continuation
cycles, inspect actual attack playback, save its frame and deliver a delayed
cancelled repeat reply before completing.

Accepted native validation: ordinary Cargo development binary and GPU example
built successfully with CARGO_INCREMENTAL=0; 41 emote-filtered tests and 25 rig
adapter tests passed. All four male/female dance/beach GPU runs exited successfully
using the production weapon catalog and rifle item 328. Each verified the attack
clip, visible hand weapon, rejection of a delayed cancelled reply and subsequent
rifleready pose. All four final screenshots were inspected. Native resource
validation returned status ok. This is an offline deterministic server-reply
fixture, not a live-shard network test. Artifact hashes are retained in
`../../../docs/reference/evidence/cases/artifacts/player-emotes-20260908/attack-native-validation.json`.
