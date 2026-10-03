# Nano Rocket and Recall runtime gaps

Question: why do upward Rocket powers consume an activation without lifting
the native player, and why does Recall move the player to the origin?
Consumer: FFOne normal-world Nano action dispatch, movement controller and
Recall registration ingress. The owner confirmed upward Rocket (not a weapon
projectile) and Recall landing near Johnny Bravo at the origin.

## Authority

Current `primary` is Retrobution 20260821. Raw `main.unity3d` is 8,221,718
bytes, SHA256 `01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`;
its hash was checked again for this investigation. The exact
`level0/Assembly - CSharp.dll` is 1,762,816 bytes, SHA256
`0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The reused payload hash was checked against the existing tracked exact-entry
receipt `docs/reference/evidence/cases/artifacts/nano-travel-20260905-assembly.json`.
The extraction and pinned FFSpy replay command are retained in
`docs/reference/evidence/cases/nano-travel-20260905.json`. The corresponding decompilation is
under `work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/decompiled`.
No historical, patched or alternate donor defines this change.

## Proven behavior and native correction

`cnAvatarAttack.NanoSkillUse`, effect target 2 / skill type 29, calls
`cnAvatarThirdPersonMove.Jump` with the captured base jump attribute plus the
skill's four-tier ValueA. It also emits the usual self Nano request. The
movement impulse is local; Nano stamina remains server-owned. FFOne retained
neither skill type nor ValueA in its runtime projection and emitted only the
request. The native projection now retains those fields, and successful
outgoing requests launch the controller for every published type-29 power.
The launch replaces an airborne fall, preserves live horizontal input and
uses the normal collision/jump packet pipeline. Passive Jump does not compound
the base rocket launch. Boost values are addressed by the server's slot buff
IDs 21–23, with bounded authored tier lookup. This corrects the primary
caller's inconsistent bit/index expression to the protocol's slot ownership.

Primary also calls `TempDisableExtraG(1.5)`. Its coroutine suppresses the
optional variable-jump gravity override; it does not disable normal gravity.
The native controller has no variable-jump override and already uses constant
10-unit gravity. No zero-gravity period is invented. Type 40 forward Dash is
a separate mechanic and is outside the owner's clarified upward-Rocket report.

`NpcIconMode.StartRXCom` is the category-17 auto route. It checks all three
equipped Nanos for skill type 27/28 and sends `REGIST_RXCOM` with the runtime
NPC ID. The old native route was an explicit no-op. The replacement sends the
typed request and retains the server's map and coordinates only on a valid,
correlated registration reply. `cnAvatarAttack.NanoSkillUse` rejects Recall
outside an instance or without a registered point (messages 217/218). Native
Recall now applies those gates before sending or starting its cooldown.
Instance exit, map replacement and session reset invalidate registration.

The local server's `Nanos::nanoRecallRegisterHandler` writes recall coordinates
from the NPC and returns them in `REGIST_RXCOM`; `Abilities::handleSkillMove`
uses those coordinates for self Recall through `PlayerManager::sendPlayerTo`.
Without registration they can remain zero. The client never fabricates a
destination and continues to consume the ordinary authoritative warp packets.
Self planning also no longer rejects unrelated nearby NPCs whose numeric ID
equals the player's ID: they are distinct protocol namespaces.

## Presentation adapter and limits

Registration success is announced after the server reply rather than the
primary's optimistic send edge. Existing localized NanoCom type-9 notices
carry the registration and eligibility copy; no new window geometry, fonts,
images or audio is published. This is an explicit native notification adapter
for the recovered type-6 producer, not a claim of recovered type-6 geometry.
New copy has semantic EN/RU keys. Messages 217/218 use the existing native
TableData localization keys. A `recall-compact` stage in the existing NanoCom
GPU harness exercises the longest success text in both locales.

Passive speed/jump/slow already have a separate numeric server-buff consumer.
Their packet-to-controller regressions must be rerun; this investigation does
not equate those three effects with completion of all passive abilities.
Group Recall invitations and forward Dash are not included in the owner's
reported origin-teleport and upward-Rocket correction.

## Verification

The following focused checks passed on 2026-09-10:

- 43 coordinate/movement tests, compiled directly from the current production
  `coordinates.rs` and `movement.rs`, including Rocket ascent, airborne
  replacement and retained jump packet launch velocity.
- Six production Nano planner tests, including a self Recall beside an NPC
  with the same numeric ID as the player; one compact native-content fixture.
- Five app movement-buff tests, including actual 60 Hz displacement, buff
  add/change/delete, GM base retention, and all nine published Rocket skill IDs
  across all three equipped slots and authored power tiers.
- Three Recall tests for registration correlation, malformed replies,
  instance refresh/replacement and production localization key parity.
- Independent production-bundle check: all 66,846 EN/RU keys and template
  placeholders match.

Results are below `FFOneClient/target/performance/nano-abilities`. The app
tests used a generated current-source snapshot with only the separate
`app::tests` module disabled: its pre-existing tutorial test refers to the
removed `sync_tutorial_nanocom_message_context`. Production app code and the
new inline module tests remain intact. `app-tests-rustc-args.json` retains the
dependency arguments; native sources, two absolute include-byte routes and
the test-module exclusion are the only snapshot inputs/adaptations. No
source test expectation was relaxed.

The broader historical-content assertion
`real_assets_game_table_set_has_exact_tutorial_provenance_when_available`
still fails: Nano 48 expects old tune IDs 199/200/201, while the existing
native data supplies 228/229/230. The production table opens successfully;
Rocket and Recall do not change this mapping.

The existing NanoCom GPU harness passes both text-bound audits at 1264x681:

| Capture | SHA256 |
| --- | --- |
| `recall-en.png` | `035c6b64f41c43d53d6cf9dbc877420847ad52746c338752ddbaf8c638555c3d` |
| `recall-ru.png` | `49f5fbae23b8f5ccd4ac24e05e48fe4b072c1cb8f8d0ca5e88b5732a364f3eb7` |

Both were visually reviewed. English uses four line boxes, Russian six;
all glyphs fit and retain their semantic localization keys. The standalone
audio-catalog shim was updated to the current optional-path API so the
existing harness compiles against the actual presentation module.

No live-server encounter is claimed from static inspection or an offline
movement fixture. Normal development build and full-client smoke results are
recorded below when finished.

The full native application completed the opt-in NPC performance smoke and
emitted `target/performance/nano-abilities/client-smoke/report.json`, with
30,306 entities and 744 visible objects, without a schedule panic. This is
an offline startup/schedule check, not a Recall server round trip or a visual
parity acceptance. EN/RU notice captures above remain the visual checks.

The smoke reported missing RXcom geometry for NPC type 1386. This is already
classified as intentionally invisible in
`docs/reference/evidence/legacy/ffone/models/published-asset-census-20260902.json`: no
publishable mesh exists. Native interaction uses the NPC lifecycle entity;
the service-icon owner supports a root attachment for unresolved visuals.
No replacement model was invented. Confirmed registration now retains the
pending NPC identity and selects the existing registered-point effect 811
instead of leaving the unregistered effect 446 permanently active, matching
primary `NpcMoveController.MakeGameIcon`'s `iRXCom == status.iID` comparison.
The registration regression also checks this identity and its invalidation.

The regular Cargo development executable `target/debug/ffone-client.exe`
(built at 19:31:50 local time) also completed the same offline smoke. Its
report is `target/performance/nano-abilities/dev-client-smoke/report.json`;
no ERROR or panic was logged. No performance improvement is inferred from
these startup runs. Final focused Recall and movement-buff reruns pass
3/3 and 5/5 tests, including the confirmed-point icon identity checks.

The requested `cargo build -p ffone-client --bin ffone-client --locked`
completed successfully in the shared target directory at 19:33:47 (8m52s,
including build-lock waiting); log: `target/performance/nano-abilities/dev-build-final.log`.
