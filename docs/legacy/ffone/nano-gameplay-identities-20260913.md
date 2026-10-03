# Restored event Nano and active native model identities

The owner requested independent IDs for already installed Nano models, moving
Coop off the Unstable Nano quest slot, and assigning Unstable Nano powers to
Ben, Ghostfreak and Upgrade. This is an explicit native extension over the
current primary tables, not a claim that primary Retrobution already ships the
requested roster.

## Evidence

Source `primary` resolves through `tools/legacy-sources/sources.json` to the
immutable Retrobution 20260821 build. `TableData.resourceFile` is 802,929 bytes,
SHA-256 `bd8767c2d316f60a4b38de320794701b2391d7b40412020bdc1d17e88c7b7212`.
The decisive MonoBehaviour is `xdtdatas`, pathId 7, in serialized asset
`CustomAssetBundle-1dca92eecee4742d985b799d8226666d`. Strict object evidence
resolves its script pointer with zero unresolved pointers.

```powershell
work/build/debug/fusionforge.exe dump-object-evidence primary ../builds/retrobution-20260821 TableData.resourceFile 7 --type MonoBehaviour --out work/cases/nano-identities-20260913/primary-table.evidence.json
```

Primary mission 841 is named **The Unstable Nano** (mission string row 15284).
Tasks 5213, 5214 and 5215 all select `m_iSTNanoID = 41`. Primary Nano row 41
instead selects `nano_coop`, name Coop, with the retained event description
“Changes appearance for special events!”. Previous 20260613 has the same
replacement, so it is not a historical restoration source for this row.

Primary rows 37 and 38 retain Flapjack/Johnny Bravo meshes, tune rows and named
string rows, but set `m_iNanoNumber` and `m_iNanoName` to zero. Johnny Bravo tune
rows 201/202/203 point to raw skill rows that are complete, exact duplicates of
skills 13/4/22 and retain those original skill IDs. Activating Nano 38 without
repairing these references reproduces `MissingTableData(38)` in the native
acquisition projection. References now use canonical skills 13/4/22; no skill
payload changes.

The native model registry already contains all six otherwise unreferenced
models: `nano_ben`, `nano_flapjack`, `nano_ghostfreak`, `nano_holonano`,
`nano_johnnybravo`, and `nano_upgrade`. The existing native editor explicitly
names `nano_holonano` Unstable Nano and uses the accepted `holo-nano` portrait.
No model, material, texture or animation bytes are changed here.

The Academy donor's Unstable Nano and Van Kleiss aliases had previously become
native IDs 52/66. They remain independent IDs over their existing shared Van
Kleiss model. Their prior acceptance is preserved rather than confusing the
Academy alias with the owner's requested primary quest slot restoration.

## Native result

| ID | Result | Power contract |
| ---: | --- | --- |
| 37 | Flapjack activated | Existing primary tunes 198–200 |
| 38 | Johnny Bravo activated | Existing tunes 201–203, canonical skills 13/4/22 |
| 41 | Quest Unstable Nano, `nano_holonano` | Neutral tunes 285–287, effects 210/211/212 |
| 67 | Independent Coop | Existing Coop tunes 210–212 and descriptions |
| 68 | Ben Tennyson | Same tunes as Unstable Nano, owner-selected |
| 69 | Ghostfreak | Same tunes as Unstable Nano, owner-selected |
| 70 | Upgrade | Same tunes as Unstable Nano, owner-selected |

The restored event powers retain the previous slot's exact numeric effects:
bonus Taros, additional Fusion Matter and cone stun. Their neutral EN/RU names
and descriptions are authored for this extension; they are not presented as
recovered historical text. Coop retains his own names/descriptions for those
effects. Ghostfreak and Upgrade retain the explicit missing portrait artwork
already used in the editor; no unrelated Nano portrait is substituted.

Client gallery order is now keyed by native IDs. The three former display-only
supplements are removed, and every gallery entry is backed by the production
table and server bank. The five-column viewport and existing slot dimensions
are retained; the scroll content grows to fourteen rows to expose all 67 IDs.
The duplicate display name at IDs 41/52 does not merge their ownership or icons.

## Replay

Run from FusionForge:

```powershell
python tools/native/restore-nano-identities.py --target-root ../FFOneClient --server-xdt ../OpenFusion/bin/tdata/xdt.json --stage work/cases/nano-identities-20260913/stage
python tools/native/restore-nano-identities.py --target-root ../FFOneClient --server-xdt ../OpenFusion/bin/tdata/xdt.json --stage work/cases/nano-identities-20260913/applied --apply
```

The publisher requires synchronized Nano tables first, validates the expected
preimage and model routes, stages all four outputs, verifies EN/RU keys and
placeholders, and atomically replaces each declared JSON file. Full replays from
the original preimage and from the installed result produce the same bytes.
All non-Nano tables and the other 64 existing Nano rows remain identical.

Receipts:

- `recipes/native/tables/nano-gameplay-identities-20260913-client.json`
- `recipes/native/tables/nano-gameplay-identities-20260913-server.json`

## Validation

- `cargo test -p ffone-client --lib user_equip_ui::tests`: all 35 tests pass,
  including the expanded gallery and its final scroll position.
- `cargo build -p ffone-client --bin ffone-client --example nano_acquisition_client_smoke --example user_equip_ui_gpu_preview --example world_nano_summon_gpu_preview` succeeds.
- `cargo test -p ffone-client --test nano_identity_roster`: two production-bundle
  tests pass, including model/identity separation, gallery ownership, complete
  tuning, EN/RU keys and quest reward IDs.
- `nano_acquisition_client_smoke`: 27 successful ability/equip/relogin cases for
  IDs 37,38,41,67,68,69,70,52,66 using an isolated OpenFusion server and database.
- Real `ffone-client --validate-assets`: passes native domain routing validation.
- Real `ffone-client --network-smoke --login-address 127.0.0.1:23150`: login and
  world bootstrap succeed with zero malformed packets.
- GPU preview completes all 21 model/skill selections for IDs
  37,38,41,67,68,69,70. Captures under
  `work/cases/nano-identities-20260913/gpu/summons` show the restored green
  Unstable model, separate Coop and all five added identities. This checks
  rendering with each selected skill, not the existence of three distinct
  authored animation clips per model.
- EN top, RU bottom and RU Unstable viewer captures under the same `gpu`
  directory confirm the gallery layout, access to its final two entries and
  localized Unstable powers. Ghostfreak and Upgrade intentionally display the
  existing question-mark icons; no dedicated portraits are present.

The local normal server is not restarted by the isolated tests. Deploy both
the client changes and corrected server XDT, then restart the affected server.
No player database is rewritten: existing ID 41 becomes the restored event
Nano, while Coop is separately acquired at ID 67.
