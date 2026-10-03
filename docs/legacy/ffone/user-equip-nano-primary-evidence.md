# UserEquip Nano tab — primary evidence and asset ledger

This ledger records clean `retrobution-20260613` protocol-0104 behavior first. Its raw
`TableData.resourceFile` (784,963 bytes, SHA-256
`6d4cea151152e2ab75b7d16590bda00fba172600a163e0b3a5318564df0d7e3b`) owns 42 active Nano
identities and their exact `m_iNanoSet` order. The explicit Academy extension is then formed from
`alternate/TableData.resourceFile` (751,481 bytes, SHA-256
`d724c9417de5ff35e9bdb7bdfcf9bb93b4c8f715f42623fa2e1834d835be60a5`) by name: only its 21
names absent from Retrobution are appended. Numeric Nano IDs and icon numbers are not cross-build
identity keys. Adding the source-backed but unpublished Ben Tennyson objects produces a 64-entry
gallery. The fixed 37-entry protocol bank remains
ownership authority and does not limit which catalog rows are visible.

Finn has no named active row in Retrobution TableData, but the raw primary Icons archive does own
the actual Finn portrait: `nanoicon_46` PathID 2868 (5,782 PNG bytes, SHA-256
`37aeaf6085aaf827ece7cc92fc94e23fafb60785a281af5c925018ea997cfb5f`) and `nanoready_46`
PathID 2869. A fresh export from the raw container visually and byte-wise identifies Finn. The
previous runtime `nanoicon_46` was instead an Academy Jake publication that had silently occupied
the same numeric filename. Runtime identity is now semantic: Retrobution 46 publishes as
`nanoicon_finn.png`, while Academy Jake remains `nanoicon_jake.png`.

Academy icon donor: `alternate/Icons.resourceFile`, 6,054,910 bytes, SHA-256
`b3995181bc9e917d0bf2b35e38361e0e18eed6f14c95aab52eb5cfd81f056cea`.
Each normal and ready icon was exported with `fusionforge export-exact-texture`; full reports are
`work/legacy-sources/user-equip-ilspy/texture-academy-nanoicon-*.json` and
`texture-academy-nanoready-*.json`. They publish in the common catalog as semantic character paths
`icons/entities/nanos/nanoicon_<slug>.png` and `icons/entities/nanos/ready/nanoready_<slug>.png`,
so cross-build numeric collisions cannot overwrite Retrobution characters. All primary portraits
use the same character-name contract; the runtime tree contains no numeric Nano filenames.
Retrobution wins collisions for Finn (46), Ben Tennyson (37), Johnny Bravo (38) and Flapjack (41).
The `nano_ben` model and unused Academy Nano string row 42 (`Ben Tennyson`, `Cosmix`, `Coming soon
from DexLabs, Inc.`) prove that 37 is Ben rather than Rex. Rex therefore retains its distinct
Academy `nano_rex`/icon-41 publication. Cheese and Jake retain their Academy publications. The two Academy aliases `Unstable Nano` and
`Van Kleiss` share the single `van-kleiss` asset publication. Earlier numeric catalogs are retained
only as ignored recovery archives under `work/legacy-sources` and are not runtime sources. Primary
`nanoready_37` through `nanoready_49` are separately published from the raw primary archive; this
restores the World/Event Nano silhouettes omitted by the earlier 0–36-only publication.

| Icon | Alternate PathID | Published PNG bytes | Published SHA-256 |
| ---: | ---: | ---: | --- |
| 39 | 2405124265 | 3366 | `a461449f5b6dd423bc9d391799f8856762d79788e37ffa149520f88e17b9b621` |
| 40 | 933898679 | 4097 | `3e29ef6741a10bd7de67e5b9ac85828db1aa804b26ba7ce4084fdd75e8f0a670` |
| 46 | 82486789 | 3629 | `09d93cb0543343f38853335be7a4521e159b530a71bae5c67890575ed329b383` |
| 50 | 3985599638 | 5602 | `52698b33faea3fdab6af1dfb491f9654d0e05da7e6eb2c34cd3e1daa7ce9510f` |
| 51 | 2784108418 | 4517 | `b3dbbcf5716d9eddac38ff004309ed4309fb5344487ea75cece568769384ea18` |
| 52 | 4262908669 | 5789 | `e2bbbfc97711839a3f314795f0472318d2d3212de38e35905c9171a3d5db2797` |
| 53 | 732401998 | 5691 | `dfd5ededc12283170a099a98c95f2bd937bd44665e9231f49a2e2d53a3f3fd13` |
| 54 | 1770160708 | 5628 | `1b89a91f27cc520da0ee8ca2b52f076b60a5f9e405c643717c126c66feb6c643` |
| 55 | 3351054877 | 3209 | `0bd0a3746aabf9a1f2aef1a7770d9eee17b72f6d5c90ee7a77aa9623ca56f120` |
| 56 | 305696074 | 4492 | `d5965fb67f3422ecd307fa721eb81abd28b128444c61033d605ef687c4f4af7c` |
| 57 | 448732432 | 2933 | `a804710393e5de20fa9a2e59b129e60c60d329113dc5232753c684d6ae1d4f6b` |
| 58 | 846631979 | 3817 | `a8743f1d012f59f1d89c3dfc4177517232437156f60f05ac5f674a719caee17a` |
| 59 | 3179899926 | 4374 | `b4c60727321ef8c63a0186cd35281da76e7a05e1eb1a5c7c8f8166d3cb47fd33` |
| 60 | 482564997 | 2227 | `434fed51a320f9eb5b2b0a72a7f0f46393087b3fe9d263de53a3fa22a99ca9cd` |

## Code and interaction evidence

- `Panel_PCStuffScript` uses Item tab texture/hit Rects `(0,0,142,29)` / `(10,5,105,15)`
  and Nano tab texture/hit Rects `(96,0,129,29)` / `(135,5,60,15)`.
- The Nano panel uses viewport `(6,36,366,500)`, five 67-pixel cells at a 69-pixel stride.
  Clean computes its extent from `LastNanoSlot`; the explicit 64-entry merge uses 13 rows and
  an 897-pixel native extent while retaining the same viewport, stride and scroll controls.
- `InventoryManagerScript.GetNanoSlot` scans `SIZEOF_NANO_BANK_SLOT == 37`, excludes index
  zero, filters `m_iNanoSet > 0`, and `SortSlot` orders the gallery by `m_iNanoSet`.
- An owned Nano uses its normal type-1 `nanoicon_XX`. A missing bank entry is reconstructed
  with a forced type-5 `IconElement`. `InventoryManagerScript.GetNanoSlot` retains the
  `m_iIconNumber` resolved through `NanoElement.m_iIcon1`; primary
  `AvatarUtil.GetIconTexture` formats that retained number as `nanoready_XX`. The filename is
  therefore keyed by `IconElement.m_iIconNumber`, not by Nano ID or the `m_pIconData` row index.
  In particular, Nano 34 resolves `m_iIcon1 == 27` to `m_iIconNumber == 0`, so its exact missing
  presentation is `nanoready_00`.
- The gallery and detail-window icon roles are distinct. The locked gallery cell uses the exact
  source-scoped `nanoready`, while `DoNanoPopup` resolves and draws the full-color `nanoicon` even
  for an unowned Nano. Reusing the gallery projection for the popup is not source parity.
- Clean multiplies `Input.GetAxis("Mouse ScrollWheel")` by 200, but `mainData` serializes that
  axis with sensitivity `0.1`. Bevy line deltas bypass Unity's axis stage, so the adapter applies
  `0.1` before the clean multiplier: one ordinary wheel line moves 20 pixels, not 200.
- Nano mode retains its own 397-pixel maximum after the shared item layout is built. Reapplying the
  190-pixel item maximum at that boundary hides the final three Nano rows and is not source parity.
- The exact exported DXT-origin ready PNGs retain RGB in fully transparent texels. Nano icons use a
  nearest sampler so resizing 64 pixels into the 67-pixel clean cell does not blend that hidden RGB
  into pale fringes around the silhouettes.
- Academy Cheese row 41 supplies `LEVEL 8 NANO` and tunes 211/212/213: `HEAL!`,
  `I LIKE CANDY!` and `GOOD HORSIE!`. These rows are used as the fail-closed supplement when the
  runtime journal lacks the extension's skills.
- `Panel_UserClothes.SetNanoInfo` projects the three equipped slot IDs, bank skill/stamina,
  Nano name, `m_strComment1`, style, maximum stamina and current skill icon. Status Rects are
  `(24,215,126,69)`, `(335,175,126,69)`, `(348,350,126,69)`; style overlays are
  `(79,180,59,61)`, `(389,139,59,61)`, `(402,314,59,61)`.
- A Nano-cell left click enters `NanoMachineScript` popup ownership. `DoNanoPopup` uses the
  360x620 popup at `(Screen.width/2+100, Screen.height/2-310)`; an equipped Nano adds the
  360x633 surround at `(Screen.width/2-402.5, Screen.height/2-315)` and offsets the popup to
  `(Screen.width/2-400, Screen.height/2-297)`. `DoNextNanoPopup` uses the distinct 357x460
  background at `(Screen.width/2+100, Screen.height/2-230)`. Ordinary UserEquip exposes this
  read-only detail viewer; mutation remains owned exclusively by Nano Station / Nano Free
  Tuning modes.

Primary decompilation records are under the offline, ignored
`work/legacy-sources/user-equip-ilspy` workspace. They include `Panel_PCStuffScript.decompiled.cs`,
`Panel_UserClothes.decompiled.cs`, `InventoryManagerScript.decompiled.cs`, and the adjacent exact
texture exporter reports. No runtime path depends on those records.

## Static UI textures

- Source: `primary/builds/retrobution-20260613/main.unity3d`, 7,000,415 bytes,
  SHA-256 `59788201962b6a1737b114486c361fe74eef69f507d1d125ca3171377eec602f`.
- Serialized owner: `sharedassets0.assets`.
- Conversion: `fusionforge fusionforge export-exact-texture <raw-primary-container>
  <PathID> <report.json>`, then byte-for-byte base64 decode of `payload.dataUrl`; no resize,
  recolor, repair, or re-encode.
- Exporter SHA-256:
  `7aef877cba8fb4f51809e030d6e89abfcad0d059d8a16a3b4e33db55fd7861fd`.

| PathID | Unity name | Published route | Size | PNG bytes | PNG SHA-256 | Source bytes | Source SHA-256 |
| ---: | --- | --- | ---: | ---: | --- | ---: | --- |
| 413 | `nanoback` | `assets/game/ui/user-equip/nano-back.png` | 241x114 | 3674 | `8600cf85bb42028fd4c5cdf6fc6f29dad8dd8dc81726adfcf679e6f4e6b438cc` | 28304 | `f537984ec539dd96738b6c12ee5717d9f22aa37635210baa78990f5ee114b95b` |
| 377 | `nanodlg` | `assets/game/ui/user-equip/nano-dialog.png` | 126x69 | 9318 | `724441fd274e238f97c5ae9e5915197b478c8978a7696e9859c416abb5e740cd` | 9216 | `d535a16ef3d324d487dbe7463c0348383e58949f156e7f547286135e04227b41` |
| 417 | `blue_icon` | `assets/game/ui/user-equip/nano-blue.png` | 57x72 | 3405 | `b7c1ddcb39c44a71c0b37dcab906d9183faaf3554d794c5ff7a6c76db67260d3` | 4320 | `aa38e140bded59c7f78b7c89f31e156b6a647ab2c1302f6191e25c70201adab5` |
| 349 | `red_icon` | `assets/game/ui/user-equip/nano-red.png` | 57x72 | 3347 | `b5d67c14b02ba82fcb91207ec88447c962a3aefdab2f5fe7f64706763bdba1a8` | 4320 | `db9c41f4c5614d6e1da45eb699365b5d547f5f24983a502843fed6ec57f14067` |
| 588 | `yellow_icon` | `assets/game/ui/user-equip/nano-yellow.png` | 57x72 | 3333 | `9f89d636bf296a919cb374422bcd8f727d5fc876fb588e99f71aff1fbc893884` | 4320 | `66e3f62013f51ee1daf9e26bd08e6c5d9c4655eabf58e4eedb58e9330a587e9b` |
| 677 | `NanoPopUp` | `assets/game/ui/user-equip/nano-popup.png` | 360x620 | 23576 | `aacb4deea4b36e99879195bbf7dbfb18293c32e8740d1306e92b7ff987cf2e1c` | 223200 | recorded in `texture-primary-nano-popup-677.json` |
| 609 | `NanoPopUpEquippedBG` | `assets/game/ui/user-equip/nano-popup-equipped.png` | 360x633 | 7264 | `689aa8f43c77f9d8655787184d24ffcf7cc5100fd8e6222031553d54a963f420` | 227880 | recorded in `texture-primary-nano-popup-609.json` |
| 552 | `NanoPopUpNextNano` | `assets/game/ui/user-equip/nano-popup-next.png` | 357x460 | 17305 | `a2c68aae111ece465036ab26fbb140f55b1a46b119e287a58d632b801e6485ae` | 164220 | recorded in `texture-primary-nano-popup-552.json` |
| 661 | Fusion Matter requirement bar | `assets/game/ui/user-equip/nano-popup-fm-bar.png` | 15x5 | 144 | `69439677b96fec427a71542c8a494d48421937faae321c50a5deaaea1cdd7d67` | recorded in extraction report | recorded in extraction report |
| 593 | required-item bar | `assets/game/ui/user-equip/nano-popup-item-bar.png` | 13x4 | 136 | `c53b37eb788b31a86a986c86ba913518827e9e7bc4cdbeb17d26c1a5a1b9d539` | recorded in extraction report | recorded in extraction report |

The viewer requirement denominator is not Nano-row data. Clean
`NanoMachineScript.DoNanoPopup` indexes
`m_pAvatarTable.m_pAvatarGrowData[playerLevel].m_iReqBlob_NanoTune`; level 36 is
`8679`. The selected skill is reordered into the first card, while the other
two powers retain their clean order. Inventory mode exposes this complete
viewer read-only; `ACTIVATE`/`EQUIP`/`UNEQUIP` mutations remain owned by the
Nano Station mode.

The item-popup command controls use `FusionFallInvenSkin.button`: PathID 640
`blue_button_normal` and PathID 309 `blue_button_over`, both nine-sliced with
border `6/6/6/4`. The exact source weapon Rects are `(120,360,170,28)` and
`(120,390,170,28)`; JEFFE 14 uses line spacing 11.3, normal text RGB
`0.9/0.9/0.9`, and hover text RGB `0.23/0.464/1.0`.

`nanoback` is nine-sliced from the serialized clean border `219/10/34/77`; the other four
textures are direct stretched `GUI.DrawTexture` / `GUI.BeginGroup` backgrounds.

## Forced type-5 `nanoready` textures

- Raw source: `primary/builds/retrobution-20260613/Icons.resourceFile`, 5,800,411 bytes,
  SHA-256 `a05602d6e96e2e74ecad207f42e519605259434210de8da19e422b30d642e544`.
- Serialized owner discovered with the mandatory object helper:
  `CustomAssetBundle-784fa24bcf2da4f5eabe9547958616eb`.
- Patched navigation-cache copy (discovery only): 19,984,725 bytes, SHA-256
  `3f8badb9c5292ac5fd0427535003e2d4ca0f9b1fe5f17c2a362ace68874c8f71`.
  Every accepted report below was exported again from the raw primary `Icons.resourceFile`, not
  from the patched cache.
- All are exact 64x64 texture-format-11 payloads with one 4096-byte source level. Unity objects
  keep their numbered source names for provenance, but published routes use the resolved character
  slug under `assets/game/icons/entities/nanos/ready/nanoready_<slug>.png`.
- These textures are individually named `Texture2D` objects loaded by the constructed
  `Icons/nanoready_XX.png` route. There is no serialized NanoReady array whose position could
  redefine the mapping; PathID/object enumeration order is not a runtime key.

Clean Nano ID to retained ready-texture number:

| Nano IDs | `m_iIconNumber` values in the same order |
| --- | --- |
| 1–9 | 34, 28, 20, 01, 07, 33, 17, 03, 19 |
| 10–18 | 25, 06, 31, 23, 14, 35, 18, 05, 26 |
| 19–27 | 02, 04, 10, 08, 32, 30, 12, 15, 24 |
| 28–36 | 29, 22, 11, 27, 09, 16, 00, 13, 21 |

| Nano ready | PathID | PNG bytes | PNG SHA-256 | Source SHA-256 |
| --- | ---: | ---: | --- | --- |
| `nanoready_00` | 521 | 3583 | `08304eddc651abdfcdd3609d1005e9b1375b128a7bb5842e5d28682a452dc89b` | `e0d8f27e1467006eab9e396fe69098769583ab29b743daca67c402f6d46a15ce` |
| `nanoready_01` | 2397 | 4136 | `3c69c1132031debba1b96ba1687f561ce32a0f28e2519906a20dea483eec4dd6` | `67f448e0a733885690859fc8d4b20313ddc59ccc4afcca2ca2501fae158a83cf` |
| `nanoready_02` | 1944 | 4112 | `89b8e7005e23598207f4c7be4b029e80ab6f6fd455b988cfee457c3d2c39c1b8` | `ad3e5fb89ffd914144c1313084ee0c79ecd9bf4e3321231400d942c3b6f503d6` |
| `nanoready_03` | 1078 | 4036 | `3496fb4e50a77c952bc5db567ef82ecd6b5ca444e3918175d1697d7ee2f3cd81` | `55c138fcca2911c75247fd140627b01e490d78f5be3a6a828123831910a579a0` |
| `nanoready_04` | 119 | 3770 | `4e36888290e9b9241453458d0f6db90497193eaa5c501a47e669b52d266b9dbf` | `5e8083a92c9241dcffe2e133b6488bf446682357b36d50ab2397a79583e38ec5` |
| `nanoready_05` | 2503 | 4039 | `3d0cbb2c3545a3db5937c512a927c75e1bb2d3d541b611201c404bf87d5608de` | `7a5d60eecb5f0107e9f5c10328f4b109cbf0856d46df7c0e49cf524d19e2de6e` |
| `nanoready_06` | 1128 | 3598 | `b14443333a3883d6e68d688f036617fa37d923343d7dcb2fd21f8f0a622dfb6f` | `320a48a3472bfcdbe80e2d7b261be5ee05b295acadd8fd2df2a246b2a9d5135b` |
| `nanoready_07` | 2381 | 3318 | `855e437de0a590aef39b29237d5443070756f32ba401301552111026edd30bf2` | `30b27aa7de3d7c8123e178c035e03ff0a38d5480076759e718fb2fe0b5fec77b` |
| `nanoready_08` | 717 | 3597 | `d6aedf5721953e8fb1c44eed8850735d56ae0740d587a460ec92a26d1fe09696` | `760ff01bb99ef92168496a7046f8ea9943325bdf7b6a0605bcb2ff200c2556dd` |
| `nanoready_09` | 1968 | 3676 | `39a3243afb0381202a3d0be7c572389f9e13869da892871977c5998db5935793` | `2900a6bed1d175ca2f5992fa8a9829cdcb8fcef9e726d8faaf42af0b3c56d0fd` |
| `nanoready_10` | 1340 | 4269 | `d78e5cf2b6e8663c9b4e778534d979cee922899fdc2025c3f83c0508cc31f7b6` | `45fe3ea7be9fc43605d27cd8cae1cc5356e7c50f06f2318caea830cbaedccf78` |
| `nanoready_11` | 160 | 3658 | `300fceef1fc8dc3169c3c8e8660c667fbf6250d58ccc66a455787351cdd061fb` | `03219cbbc87bdb476ee35129d55eaf680a5a289f76eee32ce486f8a733afe108` |
| `nanoready_12` | 863 | 3808 | `06ed09591215c462bd9f49fd51a65e7ace86034661dceeb6f43613f413d0aa8e` | `b96fbc50840d189b6324b96c51a748785e8fad038b6f753268e52f647a7bfd62` |
| `nanoready_13` | 2302 | 4776 | `680dfbf55c89dcbb6fbc48c02ceaed5131f6b6ea2d7bd64ba03b15f415fd6fce` | `963aafb2c9d8c9080157de198841a74c127bb2b2dc618dbf241cd3c1e3e577c9` |
| `nanoready_14` | 2580 | 3183 | `11adf39e83a1f16f5511ca8acdf3b92f214a6449602efa045af3d4d3611c8aa9` | `0da50d9270187f19740c4ca19881c26cbb97672df4b0fc47977146c84ecb1a55` |
| `nanoready_15` | 1884 | 4133 | `bdcb0b7fd7fbfda0c20b35a937c4f42c8bd40926f4e450f52c0ff04462cf2691` | `d7e2fbf692bb6209dc06f1f3e0df801d79c2354ffe48bd1d8d9cc52b396820f1` |
| `nanoready_16` | 700 | 4854 | `5f0fc35e4f7c71a1c001fc2df217716c104de1c1720cf4a84492ba396c31b1fc` | `08c80e3aff2b58613aa6dcdce167f4b8703088b3ca5258bdf07e1bf823f897b4` |
| `nanoready_17` | 1026 | 4308 | `2c3cd0de6860346bf77fd6b40e833e8eb9d839428aa3148d1ea672a154d73a9c` | `7bd93129e2af6ead38cb290a1442377b9fc1027ee469a2b1e351760380e93d6e` |
| `nanoready_18` | 807 | 3408 | `d70ef9400c627338d810130b6806f91863d36404eeaf4ae001c50cdee4e6f03f` | `12d52485f7a3ab143f4d891e3463739105e412e6beaf6fc26d0fafa6635c8e6b` |
| `nanoready_19` | 364 | 4015 | `b83365706e9c564f8cf7c2b112861c44a8e0b85b4a3ec61b2a6c712947d3787f` | `21e403548c891823648b3ef008dcf6424413d7e04c26cf0770500b277ba4f671` |
| `nanoready_20` | 2771 | 3673 | `bc1ec728fe35be3f0d1074cbac1ba6c215d732b9735018882d6e43ecfff8d901` | `a2ef17257ae63f89dc9673da47a2973e39eb4dd9b972f7b5f5394690c05f981b` |
| `nanoready_21` | 787 | 4112 | `0bf6e4781249bda72559c7f66a520c91901af959107302eeb26a9f25c9674ee7` | `49c9b0e42c6547481f63d9ea25b8f2a0503c2d2f4676ede1ce800059e72f4a4f` |
| `nanoready_22` | 502 | 3571 | `f010861ea063bba334445b07ef3b2aa07e6f44988b30e1650ed2c4406424a0b4` | `417163d49c3dc44a92dd1264e0cd672e40f55d4fa36e8e834f46e823fa5a2161` |
| `nanoready_23` | 1459 | 3971 | `cfbdf1191a7b923633c5a72d038b41c505675d38d094235154716d065099dbf0` | `ec9eea77315452acdc5a13475e095634afb9e774a3e2e5d8c161dced6580e2ba` |
| `nanoready_24` | 2514 | 3470 | `35914944b928550e47c4c41ac31410db53bae3504684b9e8dc03d17372b8b139` | `8cf669da1dbabd9d67984470cffb1e024f26b883d8036fd1ef0d0d4bce183eba` |
| `nanoready_25` | 1297 | 4886 | `a9d17ec92d12270ca3e76c8738ef10fc776e0b99505bc4ef1d359df83778ea07` | `5cdc9decd6585a1e0f7e1591a4c8c85bec549a1cb07bc785b507996ed983aec9` |
| `nanoready_26` | 103 | 3507 | `6c4960ed40148bbde9d0309ecb3a769dd13fe0a105fd711b278e4599182e689f` | `09eae4036a17e2238bc86f50d79a887dcee0a6a30882de589e6f4dccc9f51745` |
| `nanoready_27` | 1518 | 3639 | `0984c0a2f5f85900afae8d4068e0df470a1bb2bbea37dc417443d29fa923833c` | `8b28430b4dae1e0e485dcffc3a8c91fe6b5d3432af795cc689d696bdc8f87abe` |
| `nanoready_28` | 2368 | 4229 | `f3004783e43eb0b389a02ffe16c5f0cf4dae88f45efd053c8c21ca79c9e0e907` | `d1ca7e9d9d48fa8603c44bcc2f228fc13a92c48284cc8728ee5ada40c197f7de` |
| `nanoready_29` | 2693 | 3975 | `7a9ec38f78e5802f70abb9d020331a39d13e49d8fc085134c9151edd4b99d54b` | `cbd0ab0125ef5b431e5e27a4e1ddd538d972a72ff30f2153af1b365834d04fe4` |
| `nanoready_30` | 1524 | 3851 | `9894627d9b0c8384e46fd5e23c4193cb6ff3a6cedd62299406674de60a6374ee` | `86f42f3d09171f15827d175ef164b5db94648d7396c110ece193ff677184cd54` |
| `nanoready_31` | 2363 | 3419 | `b43a99aa5473464321df263d7a4265cff00b7adf048d678f887d0360437d4ee4` | `8b8151b1b4dcf591a903e860eaf3df5a563ca68f61785e68e188a4e716a6aada` |
| `nanoready_32` | 1885 | 4850 | `e85113e15e8f549cae472406d32b6deffea25fd8a3906f5e443ab7bc63233a24` | `36749c060caf64804c22794d0a79b14938aa9e7c1b13e21f2194cf7140add286` |
| `nanoready_33` | 313 | 4051 | `000524420d164bb15c7899c0f8bc9ceb79dfec14c03671ddc1951743ecda626f` | `2da5c29e2d8daeb60dbb48526a0f604039d11214c96cedc50bcf4354826d776d` |
| `nanoready_34` | 511 | 3356 | `72a5499a63efb509e1dd2f41f0ac8fbf201bdd0442971ce7eb0b3cdbe64d2786` | `57f3accfe33a52da5c6e11f4982ffb90eae0225a716fea1318593b24935e8803` |
| `nanoready_35` | 102 | 4519 | `cd3824c952686ce6642687bcbf4849d0dc63a73ef59e7ae582560f56629cef68` | `e5206a7333df34fde04a8c66209ffb75fd5eb4500d2178f8c831bc9eaf3419cf` |
| `nanoready_36` | 1227 | 3726 | `ed59a9c5d1ebd591e9174742e93ffe2a52fbabd8dda2eda362065d328edee117` | `275c3018098ac30b490b62c540378c347efdae37db8e2294829994d2ca1154db` |

The standalone publication test reopens all published Nano PNGs (the static UI textures plus 37 dynamic
ready icons `00`–`36`), verifies exact byte count and SHA-256, and decodes dimensions. The
UserEquip static role contract includes the three popup backgrounds; the semantic Nano icons are content-driven
dynamic assets and are not additional static roles. Runtime content projection admits a semantic
icon path only when the file is present under `assets/game`; missing routes fail closed to the
clean generated checker.
