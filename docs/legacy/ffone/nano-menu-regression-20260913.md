# Nano summon threshold and Academy station regression

The report concerned summoning tired Nanos and station equip/replacement for
accepted Academy IDs 60+. The owner clarified that the station report came from
another tester and may already be fixed; the current station must be exercised
before changing an unproven ID gate.

## Summon authority

Canonical primary is Retrobution 2026-08-21. Reuse the exact assembly extraction
recorded in `nano-acquisition-20260908.md`: `main.unity3d` SHA-256
`01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`, assembly
SHA-256 `0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`.
The focused `cnOwnAvatarStatus` decompilation at
`work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/decompiled/cnOwnAvatarStatus.cs`
has SHA-256 `46a0fdf6427688985576edf8b362d9f24fda541524637634b815d6e7bcccc5e6`.
Its normal input path tests `stamina / m_iNanoBattery1 > 0.2`, plays
`Nano_FailSummon` on rejection, and separately permits recalling the active slot.

FFOne's request gate previously checked ownership and a selected skill but no
summon-energy threshold. The native gate now uses the production table maximum
and the exact integer comparison `stamina * 5 > maximum`. This avoids boundary
rounding and preserves server authority for an already active Nano below 20%.
Failure SFX resolves by semantic name through the existing gameplay UI audio
route at its 0.7 gain. No localized text or binary payload changes are needed.

## Station checks

The current station routes requests by native ID and rejects unowned/untuned
entries, pending requests and an active destination slot. The latter is the
accepted primary station gate described in `nano-station-equip.md`, not an
Academy ID limit. Preserve it.

The isolated server checks cover IDs 60–66 and all three abilities (21 cases):
client table projection, authoritative equip and saved skill after reconnect.
Logs are `../OpenFusion/work/nano-relogin-20260913/menu-nano-<id>.log`.
No test-server player data is copied into the normal server.

The native station regression test projects every accepted ID 48–70 from the
production table with a deliberately reversed owned bank. It checks exact
request identity for replacement of each of the three occupied inactive slots.
The GPU fixture accepts `FFONE_NANO_STATION_ID` and `FFONE_NANO_STATION_CLICK`;
the latter clicks the actual localized second slot label through Bevy focus,
checks the exact emitted native-ID action and verifies no predicted slot change.
Captures and logs belong under `work/cases/nano-menu-regression-20260913`.

## Completed verification

- Six station unit tests pass, including the production roster and reversed bank.
- The normal-world shortcut regression passes: maximum 150 rejects stamina
  -1, 0, 1, 29 and 30 and accepts 31; maximum 100 rejects 20 and accepts 21;
  an exhausted active Nano can still be recalled.
- Eight GPU label-click cases pass for IDs 60–66 and 70, alternating RU and EN.
  `station-click-<id>-<locale>.log` records each exact action assertion; the
  corresponding PNG records the visible station. Nano 60 RU was visually reviewed.
  These fixtures verify real Bevy focus/input and emitted requests; the separate
  21 isolated-server cases above verify authoritative equipment and persistence.
- The client and GPU example build successfully. The rebuilt client's
  `--validate-assets` returns `status: ok`, and a full application acquisition
  fixture completes successfully (`final-client.log`, `final/nano-acquisition.json`).

No Academy-specific station refusal was reproduced, so its production equip
logic was preserved. This verification does not establish completion of the
separate early acquisition-visibility or every-Nano voice reports.
