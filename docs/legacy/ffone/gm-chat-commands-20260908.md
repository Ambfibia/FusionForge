# Native GM chat commands

Evidence question: which client-side chat commands produce GM packets, and which
server acknowledgements must FFOne consume? Consumer: native gameplay chat,
inventory and player status. Acceptance: exact packet fields, coordinate scaling,
access checks, EN/RU feedback and a real client build.

Primary source: `retrobution-20260821`, `main.unity3d` (8221718 bytes), SHA-256
`01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
Exact `level0/Assembly - CSharp.dll` payload (1762816 bytes), SHA-256
`0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
Both hashes rechecked against the raw build and materialized assembly on 2026-09-08.
Existing strict receipt:
`work/legacy-sources/gm-chat-commands-primary/reports/assembly-csharp.evidence.json`.
Managed interpretation: same directory's `decompiled/CnGuiChat.cs`,
`CheckCheatKey`, lines 2180 onward. No alternate donor used.

Replay from Editor:

```powershell
./fusionforge.cmd export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d "Assembly - CSharp.dll" --level 0 --out work/legacy-sources/gm-chat-commands-primary/reports/assembly-csharp.evidence.json --payload-out work/legacy-sources/gm-chat-commands-primary/managed/Assembly-CSharp.dll
dotnet run --project vendor/FFSpy/ICSharpCode.Decompiler.Console/ICSharpCode.Decompiler.Console.csproj --framework net6.0 -- work/legacy-sources/gm-chat-commands-primary/managed/Assembly-CSharp.dll -o work/legacy-sources/gm-chat-commands-primary/decompiled -p --preserve-iterator-state-machines -r work/legacy-sources/gm-chat-commands-primary/managed
```

Recovered contract (account level <= 50):

- `/health`, `/batteryW`, `/batteryN`, `/fusionmatter`, `/taros`, `/jump`
  send set-value kinds 1, 2, 3, 4, 5, 7 with local PC ID and an i32 value.
- `/warp x y` sends GOTO `((512*x+256)*100, (512*y+256)*100, 10000)`.
- `/goto x y [z]` sends GOTO `(100*x, 100*y, 100*z)`, default z=100.
- `/itemN type id count [time-left]` sends GIVE_ITEM, eIL=1 and a free
  inventory slot. `/itemQ id count` uses eIL=2 and type=8.
- Existing `/speed` and `/nano` handling remains in place.

Native extensions: aliases `/taro`, `/fm`, `/hp`, `/item`; checked numeric
overflow/arity, localized errors instead of managed parse exceptions.

Server contract inspected in `OpenFusion/src/BuiltinCommands.cpp`,
`setValuePlayer`, `gotoPlayer`, `itemGMGiveHandler`, and
`Missions.cpp::findQSlot`. SET_VALUE replies carry server-capped values. Normal
GIVE_ITEM replies replace the selected slot; quest GIVE_ITEM echoes an increment,
and the server selects an existing matching stack before the first empty count.
No client-side balance or item grant is applied before a server response.

Validation: native `gm_commands_standalone.rs` tests exact ABI fields, alias
equivalence, coordinate scaling, overflow rejection, account gate, inventory
exhaustion and production localization keys/placeholders. Live server gameplay
acceptance remains separate from these structural tests.

2026-09-08 verification: all six standalone command tests pass; `cargo check -p
ffone-client --bin ffone-client` passes. The current native `src/main.rs` was
also compiled directly against the completed development libraries into
`FFOneClient/target/performance/gm-commands/ffone-client.exe` (opt-level=0 for
this diagnostic binary). `--help` and `--asset-root assets/game --validate-assets`
both exit successfully; the latter reports `status: ok`. Normal optimized Cargo
builds encountered concurrent UI/audio changes and shared build-directory locks;
the diagnostic executable is not a performance measurement or release artifact.
