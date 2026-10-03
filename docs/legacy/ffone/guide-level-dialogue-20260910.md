# Guide level notification and dialogue suspension

Question: why is the Guide's level-up message absent in FFOne, and why can
communicator presentation continue during an NPC dialogue?

Authority is current Retrobution 20260821, canonical `primary`. No donor or
asset publication is involved. The NPC-dialogue suspension remains an explicit
owner-requested correction, as recorded in `docs/reference/evidence/cases/dialogue-journal-20260905.md`.

## Managed evidence

Fresh export from immutable `main.unity3d`:

```powershell
./work/build/release/fusionforge.exe export-managed-assembly-evidence primary ../builds/retrobution-20260821 main.unity3d 'Assembly - CSharp.dll' --level 0 --out work/cases/guide-level-dialogue-20260910/reports/managed-assembly.evidence.json --payload-out work/cases/guide-level-dialogue-20260910/managed/Assembly-CSharp.dll
./work/build/release/fusionforge.exe export-ui-interaction-evidence work/cases/guide-level-dialogue-20260910/interaction.request.json --out work/cases/guide-level-dialogue-20260910/reports/interaction.evidence.json
```

- Raw container: 8,221,718 bytes, SHA256 `01B544976B2D54355507CF30FE6DFADA2B476B92B209A3D47C1499669ED9B4EF`.
- Assembly: 1,762,816 bytes, SHA256 `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`.
- Static interaction report: SHA256 `2C5D986449623250C63481BFE2FC7C52D3AB4F18467265CA8267A647658C7117`.
- Editor HEAD during investigation: `857406bc95b5dd7a5983cefc3838c86b302600a8`.

The request selects `cnMissionManager.ReceiveStartGames(cnEvent)` and
`cnGUINanocom.Update()`, both returning `System.Void`, with direct-call depth 1
and maximum 512 methods. This is a static candidate inventory, not a Unity
runtime capture or a new serialized owner/visual contract.

Reused decompilation under
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/decompiled`:

- `GameFrame.cs`, SHA256 `154D0EE67FE8EBD9B5A495339B462A26C808A50C34F88727A7F63E0CC5F6445C`:
  the Nano-create success branch commits the server Nano/avatar state and sends
  event `(12,19)` (around lines 3044–3061).
- `cnMissionManager.cs`, SHA256 `BD40E292DD98DE45881C530AAF25E10FE9BFC08C9AE805AED1F744B94A9277C0`:
  event slot 19 calls `ReceiveStartGames`; a non-null event selects `m_iLevelUp`
  instead of `m_iLoginMail`/`m_iLoginNomail`, then sends the Guide's type-9 notice
  (around lines 2030–2040). The tutorial branch returns without this notice.

## Native correction

FFOne already loads the level-up strings but only called the login producer.
The new producer uses the authoritative current Guide and the existing keyed
EN/RU table strings, portrait, and semantic CommOut voice owner. It runs after
successful Nano-create commit when level increases. Explicit server level-change
success also uses it as a native extension. Equal/decreased levels do not queue
a duplicate, and malformed short level-change packets cannot change the level.

The native general queue already checked NPC/journal state, but the separate
tutorial context was written by cinematic-only systems. One production context
writer now evaluates the final dialogue state after gameplay UI actions and
before both queues tick. Existing spawned general NanoCom audio also pauses,
including sources awaiting decoding, and resumes without re-spawning. Tutorial
slide sounds respect the same suspension. Geometry and existing assets stay as
previously validated.

## Verification commands

```powershell
cargo test -p ffone-client --bin ffone-client nanocom -- --nocapture
cargo test -p ffone-client --lib nanocom -- --nocapture
cargo build -p ffone-client --bin ffone-client
$env:FFONE_PERF_OUTPUT = 'target/performance/guide-level-dialogue-20260910'
$env:FFONE_PERF_GUIDE_NANOCOM = '1'
./target/debug/ffone-client.exe
```

The opt-in offline production fixture checks message identity, hidden state and
frozen lifetime while the NPC dialogue is open, followed by resumed visibility.
It writes `guide-before-dialogue`, `guide-dialogue-open` and
`guide-after-dialogue` PNG/JSON pairs. Results are recorded after execution;
the source evidence alone does not establish audible runtime parity.
