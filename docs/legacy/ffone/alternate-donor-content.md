# Alternate donor build: what is genuinely new, and why it cannot be merged by ID

Source: `builds/6543a2bb-d154-4087-b9ee-3c8aa778580a` (`alternate`), compared against the installed
`retrobution-20260613` table set and world registry.

Machine-readable evidence: `work/legacy-sources/alternate-6543a2bb.donor-delta.json`, produced from
`work/legacy-sources/alternate-6543a2bb.xdt.json`
(`fusionforge dump-xdt TableData.resourceFile 2139558964`).

## New world content

Map bundles present only in `alternate`:

| Bundle | `Map_*.unity3d` | `DongResources_*` | Reading |
| --- | --- | --- | --- |
| `Map_00_03`, `Map_00_04` | 157 KB, 48 KB | 55 KB, 59 KB | one small new area |
| `Map_00_12` … `Map_00_15` | 203/70/85/62 KB | 8.0/5.6/4.2/5.4 MB | one large new area |

Map bundles present only in `primary`: `Map_00_08`, `Map_01_09`, `Map_11_15`.

`TrainingGrounds.resourceFile` (49.5 MB) and `TrainingGroundsMusic.resourceFile` exist only in
`alternate` and have no `primary` counterpart. Together with the two tile groups above this is the
third distinct new location.

The alternate instance table adds 29 warp destinations that `primary` does not have, including
`Albedo's Lair`, `Dexlabs`, `Fusion Lab`, `Ice Dungeon`, `Ice King's Castle`, `Mystery Dungeon`,
`Null Void Hunting Grounds`, `Rainicorn Road`, `Ship's Interior` and `Vilgax's Ship`.

Most other `alternate`-only `.resourceFile` names (`CharacterSelection_wear_*`,
`Character_Texture_*`, `UI_*`, `GameMusic`) are a different bundle split of content `primary`
already ships as `CharacterSelection`, `CharTexture`, `NpcTexture` and the three music bundles.
They are not new content.

## New table content, by name

Counted on name-keyed rows, so a row is "new" only when no installed row carries that name.

| Table | New names in `alternate` | Rows whose index already belongs to a different `primary` row | First divergent index |
| --- | --- | --- | --- |
| `m_pNpcTable` | 360 | 174 | 36 |
| `m_pShirtsItemTable` | 166 | 185 | 2 |
| `m_pShoesItemTable` | 92 | 107 | 3 |
| `m_pHatItemTable` | 91 | 117 | 101 |
| `m_pPantsItemTable` | 91 | 100 | 13 |
| `m_pSkillTable` | 61 | 25 | 198 |
| `m_pChestItemTable` | 45 | 10 | 1181 |
| `m_pWeaponItemTable` | 41 | 43 | 165 |
| `m_pVehicleItemTable` | 41 | 62 | 10 |
| `m_pGeneralItemTable` | 37 | 36 | 1 |
| `m_pBackItemTable` | 31 | 44 | 57 |
| `m_pNanoTable` | 20 | 9 | 37 |
| `m_pGlassItemTable` | 15 | 20 | 61 |
| `m_pQuestItemTable` | 13 | 5 | 193 |
| `m_pShinyTable` | 2 | 0 | – |
| `m_pFaceItemTable`, `m_pHeadItemTable` | 0 | 0 | – |

The 20 nanos only in `alternate`: Alien X, Ampfibian, Ben Tennyson, Cheese, Chowder, Darwin, Finn,
Gumball, Ice King, Jake, Johnny Test, Mordecai, P.Bubblegum, Rath, Rex, Rigby, Titan, Unstable
Nano, Van Kleiss, Zak Saturday. Their models are the `nano_*` meshes in `Nano_001` … `Nano_057`.

## Why this is not a merge

The two builds are divergent forks that reuse the same numeric ID space. Nano ID 37 is `Flapjack`
in the installed Retrobution tables and `Finn` in `alternate`; shirt IDs diverge from index 2, NPC
IDs from index 36. Copying `alternate` rows into the final XDT at their own indices would silently
redefine hundreds of Retrobution items, nanos and NPCs, and every one of them is a server-visible
semantic ID.

Any accepted addition therefore needs:

1. a freshly allocated ID above the installed maximum for that table, never an in-place index;
2. the same allocation applied to the OpenFusion `tdata` tables, since IDs are server-visible — an
   XDT-only merge desyncs inventory, nano and mission state on the first login;
3. its own asset closure (mesh, textures, icons, sounds) exported from `alternate` and published
   under the normal audited path;
4. an explicit extension record, because `alternate` is not evidence of original Retrobution
   behaviour.

That allocation policy is a product decision, not something the extraction can infer, so no
`alternate` row has been written into the installed table set.
