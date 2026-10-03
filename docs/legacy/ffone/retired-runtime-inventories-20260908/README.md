# Retired runtime inventories

These are snapshots of the former FFOne `_runtime/audio.json` and
`_runtime/characters.json`, preserved before their removal on 2026-09-08.
They describe the last published mappings, including owner-authored corrections.
They are migration documentation, not proof of raw Unity ownership and not inputs
that the native client or release packager may require.

The replacement is the `native_asset_routes` table in native
`assets/game/data/tables/table-set.json`:

- `m_pAudioData`: semantic lookup identity and one path. Voice paths are relative
  to `audio/voice/<locale>/`; non-voice paths are relative to the asset root.
- `m_pCharacterModelData`: stable model identity, exact GLB path, animations and
  optional collision path. Payload hashes were omitted from runtime rows.

`relative-path-moves.json` records 60 Russian female-player voice files whose
filenames were aligned with the same language-independent relative paths as EN.
The listed SHA-256 values verify that only the filenames changed.

The deterministic transition tool is `tools/native/retire-runtime-inventories.mjs`.
It takes an explicit native asset root containing the pre-transition inventories,
archives their bytes, adds the native table without reserializing existing XDT
rows, and performs collision-checked renames. For replay use Editor staging;
do not run it over an already transitioned native table.

Runtime rules and tests now support RU-only lines, new locale folders, sparse
numbering and extra localized takes. Future publication must merge native
TableData routes and keep these independent locales; never recreate the retired
inventories in the client. The snapshots here deliberately remain unchanged.

Verification of the native transition: 21 foundation tests, 54 gameplay/Nano/model
consumer tests and 12 packaging tests passed. The production release validator
checked 43,744 direct references across four root contracts. The client library
and executable compiled and `--validate-assets` opened the new TableData audio
and model routes with both inventory files absent. The native editor passed a
compiler metadata check. No live acoustic or rendered gameplay comparison was
performed for this data-routing change.
