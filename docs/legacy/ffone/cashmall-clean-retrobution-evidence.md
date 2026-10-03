# Clean Retrobution Cash Mall evidence

This tranche reproduces the standalone UI behavior of the hidden Cash Mall in
`retrobution-20260613`. It intentionally does not turn the retained shell into
a functional modern store.

## Authorities

- Clean archive: `builds/retrobution-20260613/main.unity3d`
- SHA-256: `59788201962B6A1737B114486C361FE74EEF69F507D1D125CA3171377EEC602F`
- Serialized inventory: `work/ffone/legacy-work/retrobution-sharedassets0-objects.json`
- Managed sources: `work/ilspy-retrobution-csharp-20260728/Panel_Cashmall.cs` and
  `work/ilspy-retrobution-csharp-20260728/cnCashmallMode.cs`
- Managed-source SHA-256 values:
  - `Panel_Cashmall.cs`: `A1EC212735BF3A8171218BAC15A3757AFD35BB0C5DAC0071D11DD54BB30F2D47`
  - `cnCashmallMode.cs`: `74B6E6C515D9D91AA3E232199B15998B016696F0E38DA968D6DFE01E622241C4`

The serialized Cash Mall object is GameObject `1359`, Transform `1243`, with
mode/panel/PC Stuff/equip components `1573` through `1576`, scripts `1120`,
`1077`, `1045`, and `1027`, and skin `1376`. Cash is texture `551`, info is
texture `493`, and `itemBar` is null (`0`). The serialized Stuff rect has
`y=586`, but `Panel_Cashmall.Start` overwrites it to `y=595`.

## Reachable behavior

- The only observed entry is `CnGuiChat`'s hidden `/cashmall` command. It emits
  local events `(2,0),27` and `(2,3,0)` and enters game mode `27`.
- The opening animation lasts one second and uses `sin(t * pi/2)`. At
  1264x681, the final Cash Mall/PC Stuff/equip origins are `(122,21)`,
  `(707,21)`, and `(626,21)`. Backplates begin at `(114,14)` and `(699,14)`.
- `Panel_Cashmall.DoSlot` scans fixed inventory type `9`, IDs `0..19`. All five
  tabs (`New`, `Scroll`, `Potion`, `Equipment`, `Etc`) draw the same scan.
- The selected tab draws its raw enum name; inactive tabs request localization.
  The selected button return is discarded. Inactive hover and press share the
  hover texture. A real tab change resets only the Cash Mall scroll and plays
  `Tab_Click01`.
- Primary row activation calls the existing local `VendorClickItem` popup
  boundary. Right-clicking an item with type below 7 emits retained local event
  `(2,3,1)`. Neither path establishes a Cash Mall purchase packet contract.
- `GO TO MY STUFF` emits `(2,3,5)`, changes to mode 6 with init argument 2 and
  first-use condition 3, then emits `(11,18)`.
- Exit uses UI input value 10, restores the old cursor lock, emits `(2,1)`,
  stops UI-mode audio, and requests asset collection.

## Retained dead paths

`cnCashmallMode.ReceivePacket` is empty. `iUserCash` is never assigned, so the
display is always nine zero digits. `SetVendorItem` has no caller, leaving the
cached item count and scroll content height at zero even while `DoSlot` draws
rows. `FreeAssets` has inverted null checks and does not clear successfully
loaded textures. PC Stuff sends `ClickHelp`, but none of the four Cash Mall
components implements that receiver. The Cash Mall inventory mode is not
initialized in this flow, and the slot-12 recent-buy branch is unreachable from
the fixed type-9 scan.

## Native Cash Mall-owned exports

| Clean texture | Path ID | Native path | Size |
|---|---:|---|---:|
| `backbar` | 62 | `ui/cashmall/back-bar.png` | 486x13 |
| `cash` | 551 | `ui/cashmall/cash.png` | 149x33 |
| `firsttab` | 648 | `ui/cashmall/first-tab.png` | 44x29 |
| `firsttabbut` | 626 | `ui/cashmall/first-tab-button.png` | 44x29 |
| `firsttabbutover` | 584 | `ui/cashmall/first-tab-button-hover.png` | 44x29 |
| `secondtab` | 106 | `ui/cashmall/second-tab.png` | 81x29 |
| `secondtabbut` | 465 | `ui/cashmall/second-tab-button.png` | 81x29 |
| `secondtabbutover` | 189 | `ui/cashmall/second-tab-button-hover.png` | 81x29 |

All remaining images are reused through the semantic Vendor and UserEquip
asset contracts. Runtime code has no Unity-container dependency.
