# Dialogue, regeneration and outgoing chat audio

Source role: primary, Retrobution 20260821. No donor or payload changes.

Question: which calls own the missing NPC-dialogue opening, resurrection and
outgoing-chat sounds, and when should the native runtime enqueue them?

## Evidence

`main.unity3d` SHA-256:
`01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
Its exact level-0 `Assembly - CSharp.dll` entry SHA-256:
`0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
Both hashes were checked against the raw container and existing exported payload.
The export receipt is
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/assembly-csharp.evidence.json`.

Replay the managed export from the Editor root:

```powershell
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp.dll" --level 0 --out work/cases/dialogue-regen-chat/managed.evidence.json --payload-out work/cases/dialogue-regen-chat/Assembly-CSharp.dll
```

The existing `avatar-animation-primary/decompiled` navigation output contains:

| File | SHA-256 | Accepted call |
| --- | --- | --- |
| GameFrame.cs | 154D0EE67FE8EBD9B5A495339B462A26C808A50C34F88727A7F63E0CC5F6445C | CreateGameMode, linked MainGame: Open_Screen; packet 822083607: Resurrectm_Warp |
| CnGuiChat.cs | 23E17B181947608A01769703F017A60436A1C0023C526FE76B8149EB29F47A39 | ReceiveChat, ReceiveMenuChat, ReceiveGroupChat/MenuChat and Buddy receive handlers: Outgoing_chat on local success |

NPC icon mode is GameModes[10]. Its opening uses the linked-MainGame sound,
not the second inventory-animation sound reserved for other mode IDs.
ResurrectMode buttons themselves remain silent: the regeneration success packet
owns the sound, including warp-away regeneration outside the death dialog.
Outgoing_chat is separate from the SEND button's randomized click. It follows
accepted server echo, not keyboard input, a send attempt, an incoming remote
message, a failure packet, or a locally handled GM command.

## Native contract

FFOne resolves all three existing SFX by semantic true name through
GameplayAudioRuntime, using its non-spatial UI gain/channel/admission policy.
NPC camera/interaction ownership supplies a single opening edge and stays
latched through the offer/reward journal. Local accepted chat echoes accumulate
until audio collection, then drain once; session reset clears pending echoes.
Local regeneration-success decoding precedes its audio enqueue.

Unchanged native payload SHA-256 values:

| Semantic name | SHA-256 |
| --- | --- |
| Open_Screen | 0807111730F94BD08F21ECC8A3C38BDD1DA5AED4EBF20DA24155F492CD18721D |
| Resurrectm_Warp | 8DE860F488C368B3B4A8D4B2A3EF7C2F9836A83218364D958AE471AB6C372ACC |
| Outgoing_chat | 7684EE9AB6F2CB73F90DD215AA2B4BF5154FF4EC48D83385D728226E68CE4F0A |

Validation: focused native tests cover opening/idle/closing/reopening, multiple
chat echoes, local/remote and blocked/free-chat/roster gates, and production
catalog resolution in EN/RU. An actual audible gameplay pass is still required
to claim end-to-end acoustic parity.

Executed checks on 20260910:

- `rustc --edition=2024 --test crates/ffone-client/tests/gameplay_event_audio_standalone.rs -C linker=rust-lld.exe -o target/audio-event-checks/events.exe`, then the executable: passed.
- A focused probe against the compiled production `ffone_client_foundation`
  catalog resolved all three exact SFX in EN and RU and checked their files:
  passed (`target/audio-event-checks/catalog.exe` in the consumer workspace).
- `cargo test -p ffone-client --bin ffone-client outgoing_chat_audio --locked`:
  blocked before test execution by unrelated test compilation errors in
  `app/tests/tutorial.rs` (missing `sync_tutorial_nanocom_message_context`)
  and `app/tests/character.rs` (`ItemBase0104` has no `Default`).
- The initial ordinary development build encountered inconsistent concurrent
  edits in vehicle/session modules. A retry waited behind other workspace
  builds; no successful final development build or audible gameplay result
  is claimed for this pass.
