# Legacy UI audio evidence ledger

This ledger covers the native UI-audio recovery pass. The source alias is
`primary` unless a cache path is explicitly called out. Patched data is used
only as an extraction/navigation cache and does not redefine clean behavior.

## Acceptance source

- Raw container: `primary:main.unity3d`, 7,000,415 bytes, SHA-256
  `59788201962B6A1737B114486C361FE74EEF69F507D1D125CA3171377EEC602F`.
- Managed owner: `Assembly - CSharp.dll`, 1,517,568 bytes, SHA-256
  `33D6F70216B1C7BA05BCC0F270FBA97E767B129159755AF4C8835922E60ACADB`.
- Shared audio owner: `Assembly - CSharp - first pass.dll`, 340,992 bytes,
  SHA-256
  `D5849A0B866DE92740AF11DB620D1687C54C38A5554DAEC23BF4AEB3FC2AC5F3`.
- Navigation cache:
  `patched:cache/extracted-bundles/ed793e024e70bdfc/`. The checked DLL bytes
  above are the extracted managed objects used to navigate the raw primary
  container.
- Decompiler: `ilspycmd 11.0.0.9375`. The main assembly command is recorded
  in `docs/nanocom-login-mission-evidence.md`. The shared owner was inspected
  without publishing another derived tree:

  ```text
  work/legacy-sources/ilspy-tool/ilspycmd.exe -t SoundUtil "../builds/retrobution-20260613.ffclient/cache/extracted-bundles/ed793e024e70bdfc/Assembly - CSharp - first pass.dll"
  ```

No audio was converted in this pass. Every cue resolves through the existing
`NativeAudioCatalog`; its published file hash/size contracts remain the asset
acceptance authority.

## Source call map

Line numbers below refer to the ignored navigation output
`work/legacy-sources/nanocom-primary-code/Assembly - CSharp.decompiled.cs`.

| Owner | Clean call sites | Native route |
| --- | --- | --- |
| `CnGuiChat` | 27398/27786 tabs; 27408/27810 send; 27518/27978 delete; 27538/27998 warp; 27798-27806 Menu/Emote | `Tab_Click01`, randomized `ButtonSound`, and `Click_WindowSlideOut` in source order |
| `CnGuiChat.DoAddWindow` | 26470, 26476 | `No_Button`, `Yes_Button` |
| `cnGUINanocom.SetViewMenu` | 35367, 35371 | `Open_Screen`, `Close_Screen` |
| `cnGUINanocom.RenderMenu` | 37028, 37078 | randomized `ButtonSound` for each menu row and close |
| `cnMissionJournal` | 42868-43462 | `Mission_Decline`, `Mission_Accepted`, `Abandon_Mission`, `Tab_Click01`, or randomized `ButtonSound` at the exact accept/decline/tab/row/category/help/close sites |
| `NpcIconMode` | 95688-96012 | randomized `ButtonSound` for close, mission rows, and service rows |
| `cnSystemMessageManager` | 57531-57633 | primary `Yes_Button`, secondary `No_Button`; delete-mission primary uses `Abandon_Mission` |
| `Panel_Vendor` | 107725-108002 | `Tab_Click01` on actual tab changes and randomized `ButtonSound` on accepted catalog-row activation |
| `Panel_PCStuffScript` / `InventoryManagerScript.DragButton` | 104175/104223 and 85964+ | randomized `ButtonSound` for redeem/help and non-empty inventory interaction |

`SoundUtil.IsPlayable()` and `SoundUtil.IsPlayable(Vector3)` share the clean
`iMaxCount = 12` list. This pass deliberately preserves the existing global
`> 12` admission rule instead of giving UI sounds an invented priority lane.

## Intentional silence and divergences

- `cnQuickSlot` and `ResurrectMode` have no button-audio calls at their native
  interaction sites. They remain silent; there is no global Bevy `Button`
  sound component.
- `Panel_BankScript` does not add per-control clicks. The existing Bank mode
  edge remains owned by `cnBank.ReceiveBankStart`/the native mode shell.
- Vendor `GO TO MY STUFF` and the PC Stuff close control are silent at their
  clean call sites and remain silent.
- The unavailable User Store production session stays fail-closed. Its
  already-typed button cue is not used to make the inaccessible surface appear
  active.
- There are no intentional cue-name, gain, localization, or ordering
  divergences in the recovered routes.
