# NanoCom login and mission notice evidence

This ledger covers the passive type-9 NanoCom notice shown at world entry and
on mission start/success/failure edges. It does not authorize runtime access to
any legacy build.

## Authority and extraction

- Source alias: `primary`.
- Raw owner: `builds/retrobution-20260613/main.unity3d`, 7,000,415 bytes,
  SHA-256 `59788201962B6A1737B114486C361FE74EEF69F507D1D125CA3171377EEC602F`.
- Recovered managed owner: `Assembly - CSharp.dll`, 1,517,568 bytes,
  SHA-256 `33D6F70216B1C7BA05BCC0F270FBA97E767B129159755AF4C8835922E60ACADB`.
- Extraction/decompilation: the primary `main.unity3d` was extracted to the
  ignored `work/legacy-sources/nanocom-primary-main` evidence area, then
  decompiled with `ilspycmd 11.0.0.9375` into the ignored
  `work/legacy-sources/nanocom-primary-code/Assembly - CSharp.decompiled.cs`.
  Decompilation command:
  `work/legacy-sources/ilspy-tool/ilspycmd.exe -o work/legacy-sources/nanocom-primary-code "../builds/retrobution-20260613.ffclient/cache/extracted-bundles/ed793e024e70bdfc/Assembly - CSharp.dll"`.
  The patched cache was used only as the indexed extraction copy; its bundle
  index maps this object back to the checked primary raw container.
  The checked assembly hash above is the acceptance identity; generated files
  are offline evidence and are not runtime inputs.

## Recovered behavior

- `cnGUINanocom.SetMessageBox` resolves type 9 from the NPC TableData name,
  portrait and `NpcStringTable.m_strComment2` voice owner. Passive messages
  live for `fNanoMessageTime * 0.5`, exactly 10 seconds.
- `cnMissionManager.ReceiveStartGames(null)` skips tutorial entry, adds
  currently available Guide/Nano mail, checks whether a mode-2 Guide mail is
  present, and selects `GuideTableElement.m_iLoginMail` or
  `m_iLoginNomail`. Guide row 5 selects NPC 730 (Computress), string 19 when
  mail exists and string 14 otherwise. The reference screenshot is string 19:
  `Welcome back. Please check your email to learn about an important mission from me.`
- `cnMissionManager.SetMissionMessage` sends the passive panel only when
  `messageType & 2 != 0`, the NPC and string IDs are positive, and the resolved
  mission string has more than one character. `TASK_START_SUCC` uses the
  `m_iSTMessage*` fields, `TASK_END_SUCC` uses `m_iSUMessage*`, and terminal
  `TASK_END_FAIL` codes 1, 11 or 12 use `m_iFMessage*`.
- `AvatarUtil.CallVoicePlay(..., eVoice.NanoComMessage)` reduces owners with
  three or more underscore-separated segments to the first two segments, then
  selects `<owner>_CommOut01..03`. Native playback resolves that true name
  through `NativeAudioCatalog` and carries `LocalizedVoice`; no locale path is
  constructed by NanoCom code.
- `cnGUINanocom.SetMessageBox` mirrors every accepted message into chat as
  `TitleString + ": " + OutString`. Button types below 11 call
  `CnGuiChat.AddEventString(4, ...)`, gated by `bEventMessage1` (NPC/events in
  chat); interactive types call `AddEventString(1, ...)`, which `AddChatString`
  retains in ALL and GROUP. Checked on 2026-09-11 in the primary decompilation
  `work/projects/retrobution-ui-20260821.ffclient/reports/avatar-animation-primary/decompiled/`
  beside `Assembly-CSharp.dll`
  `0F2513C2303CFFC88C541A90B1B8FAD1B73F60D5218830BE72D0502533BF5792`:
  `cnGUINanocom.cs` lines 568–690, SHA-256
  `E207DA3DBC5D6AE29E095478AC34C9521FFD4C0EDDF48471C2EEF630760278B1`, and
  `CnGuiChat.cs` lines 991–1041, SHA-256
  `23E17B181947608A01769703F017A60436A1C0023C526FE76B8149EB29F47A39`.
  Native `NanocomMessageUiModel::enqueue` records the same echo and
  `receive_nanocom_chat` writes it once World or Tutorial owns the chat. The
  tutorial auxiliary `SendMessageBox` adapter keeps its existing voice chat line
  and discards the duplicate echo.

## Native publications used

- Type-9 frame: `assets/game/ui/gameplay/nanocom/nanocom_message_npc.png`,
  321x119, 5,727 bytes, SHA-256
  `A920C06948D67BE1E869D4AB627015E8550A81DCF618F231568B5382E6EC9207`.
- Computress portrait:
  `assets/game/ui/gameplay/guide/compu_icon.png`, 64x64, 7,452 bytes,
  SHA-256 `C0FF729C8704392D237A0820449230EA43467423624B5C39CA8F47BA312E4C85`.
- Text identities are the existing `content.npc.<id>.name`,
  `content.tabledata.guide.guide_string.<id>.sz_string`, and
  `content.tabledata.mission.mission_string.<id>.str_name_string` keys. The
  authored EN/RU bundles and their runtime copies remain byte-identical.
- Primary task 1428's success row sends NPC 790. That NPC references NPC-icon
  row 0 (`m_iIconType = 0`, `m_iIconNumber = 0`), so clean `AvatarUtil` asks
  for `Icons/wpnicon_00`; the checked primary inventory contains no such
  texture. Native parity therefore keeps the type-9 panel and text but hides
  only the absent portrait. A missing source icon never cancels its message.

## Compact display parity

- Clean `cnGUINanocom.OnGUI` calls `RenderNanoMessage(NanoRect)` before
  `RenderMenu(NanoRect)` and `RenderMinimap()`. Native therefore keeps the
  compact panel below the resident HUD; the minimap is expected to cover the
  panel's right edge. The clean `NanoRect`-local frame `(51,3,321,119)`, title
  `(130,2,200,30)`, body `(120,25,180,70)` and portrait `(60,15,64,64)` Rects
  are unchanged. Pixel comparison against the supplied `456x188` primary
  crop adds a 13 px leftward adapter compensation to the compact root: both
  crops then place the frame at `x=13`, the portrait edge at `x=22`, and the
  title at `x=91` relative to the crop.
- The body uses primary `ChaletBook-Regular Small` path ID 1018. The approved
  Chalet replacement is calibrated at 11 px with the recovered
  `12.07199955` line height. In the serialized 164 px content column this
  reproduces the reference breaks: `Welcome back. Please check` / `your email
  to learn about an` / `important mission from me.` Native places the Text
  entity directly at the recovered content Rect `(130,29,164,60)` because
  Bevy Text does not offset its own glyph origin by Node padding. The primary
  and native white-text bounds are both `x=92..235`, `y=30/31..64` in the
  compared crop.
- The clean JEFFE title face displays the localized NPC name in uppercase.
  Native applies that casing only after resolving the semantic localization
  key, so EN renders `COMPUTRESS` and RU renders `КОМПЬЮТЕР` without replacing
  either key or fallback identity.
- Deterministic native `1264x681` text acceptance frames are
  `target/ui-parity/nanocom-computress-second-pass-en-1264x681.png`, 40,459 bytes,
  SHA-256 `E1D362E6C79C4B8BADAF13A95B688B840146B95A1EF7AEA5BB68721AC3270D26`,
  and `target/ui-parity/nanocom-computress-second-pass-ru-1264x681.png`, 44,140
  bytes, SHA-256
  `D01EB9E29886BB8424DC8F241FA957177D569B38981C1E4750E3351EC600239A`.
  These are native harness captures, not primary-live goldens.
- The integrated gameplay-HUD acceptance frame is
  `target/ui-parity/gameplay-hud-computress-second-pass-1264x681.png`, 223,578
  bytes, SHA-256
  `A9C9BC13225C00F7A266E8483A4CD64485596ED63CD19E25E3239C436314ADF5`.
  It proves the compact panel keeps its clean top-right placement while the
  resident minimap paints over the same right-edge area as the primary crop.
  Its direct `456x188` comparison crop has SHA-256
  `B640DAE7B63248476AAF054D88E59A8D1CDE6FF6667947E1BC69B1017816FDD9`.

## Intentional limits

- A Nano mission's first start edge uses clean button type 10 and a Nano
  portrait, not the reached type-9 NPC panel. That distinct branch remains
  fail-closed until its exact assets and presentation contract are recovered.
- Production World still lacks an authoritative event-scene flag. Existing
  modal and tutorial scene suppression remains in force, but this change does
  not invent a World event-scene signal.
