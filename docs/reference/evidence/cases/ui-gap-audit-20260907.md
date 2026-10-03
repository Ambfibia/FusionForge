# Повторный аудит пропусков UI — 2026-09-07

Источник: Retrobution 20260821, canonical primary. Это адресная повторная проверка текущего кода после завершения карточек, а не повтор старого списка жалоб и не доказательство полного пиксельного паритета. Код игры и ассеты не менялись. Enchant исключён по указанию владельца; шейдеры и языковые настройки — намеренные изменения.

## Подтверждённые разрывы

| ID | Приоритет | Что отсутствует / последствие | Проверка текущего native-кода | Primary-контракт |
|---|---|---|---|---|
| U01 | P1 | Стрелки и перетаскиваемый ползунок прокрутки банка и магазина. Колесо работает; видимые детали полосы декоративны. | bank_ui.rs:2401–2434: BankDisabledControl и Pickable::IGNORE; vendor_ui.rs:3436–3467: нет InteractiveControl/Interaction; enum VendorInteractiveControl:2930 и input collector не обрабатывают эти роли. | Panel_BankScript.cs:212 и Panel_Vendor.cs:481 вызывают GUI.BeginScrollView; native сохраняет изображения полосы, но не её полный интерактивный контракт. |
| U02 | P1 | Неполное перетаскивание предметов: банк умеет переместить на отпущенный слот, но нет иконки под курсором/анимации drop; Vendor не имеет соответствующего mouse-drag маршрута. | BankPointerDraft остаётся Local внутри collect_bank_slot_input:2188; ни один renderer его не получает. Vendor input:3011 и enum:2930 обслуживают карточку/quick actions, не drag/drop. Не считать весь bank transfer отсутствующим. | Panel_BankScript.DragButton; Panel_PCStuffScript; InventoryManagerScript.cs:1999–2040 рисует drag icon, scale/fade/drop. |
| U03 | P2 | Портрет продавца скрыт. | vendor_ui.rs:2600 устанавливает npc_preview_supported=false, :4278 скрывает элемент. app/service_portrait.rs обслуживает Combi/Enchant, не Vendor. | Panel_Vendor.cs:302 вызывает AvatarUtil.DrawCamera(ownposition,cNPC). |
| U04 | P1 | Combi: кнопка справки не открывает страницу, анимация работы NPC не запускается. | app/mod.rs:5410 сохраняет last_animation и пишет COMBI_ANIMATION_OWNER_GAP; :5426 HelpRequested только пишет COMBI_HELP_OWNER_GAP. Камеры Combi уже существуют — это не повторная задача на портрет. | cnCombiMode.CombiWaiting:766 запускает SetEventAni2("in_make_out"). Native CombiRuntime отдельно выдаёт HelpRequested. Точную страницу помощи следует перепроверить перед реализацией. |
| U05 | P1 | Обмен игроками: ADD TAROS не проходит, портреты участников отключены; chat capabilities также остаются false. | Pc2pcBackendCapabilities по умолчанию false; production не устанавливает ресурс с разрешениями. pc2pc_ui.rs:2330 возвращает NumericPopupBackendUnavailable; :4623–4647 используют portrait_backend. app/mod.rs:17773 открывает сессию, но не подключает эти владельцы. | Panel_Trade.cs:409–417 открывает денежный PopupControll; :422 и :500 рисует две камеры. Нельзя считать наличие пакетов/окна завершённым обменом. |
| U06 | P1 | Рекорды гонок нельзя открыть из игрового меню. | app/gameplay_ui_actions/npc.rs:214 отклоняет RaceRank и возвращает меню; app/mod.rs:7172 отклоняет Http intent и закрывает rank. Обычный Race уже подключён, его не объявляем отсутствующим. | cnRaceRankMode.cs:987/1030/1119 — WWWForm и асинхронный запрос. Нужен совместимый доступный provider; старый www.fusionfall.com не считать готовым сервисом. |
| U07 | P1, после проверки живого NPC | Barber UI и пункт услуги не реализованы, хотя поддержка пакетов есть. | NpcServiceKind в gameplay_ui.rs:2259 не имеет Barber; поиск Barber/barber по client/src не находит consumer. ffone-protocol уже имеет PcBarberOpen/Confirm. Native таблица содержит категорию 28 для ID 3470,3477,3480,3483,3486. | NpcIconMode.cs:733–735 добавляет BARBER для категории28, :1856/1892 открывает mode31. GameFrame создаёт BarberMode/BarberGui. Primary NPC IDs3431,3438,3441,3444,3447. Это существующие данные, но присутствие NPC на работающем сервере отдельно не проверялось. |
| U08 | P2 | Часть обучающих FirstUse-сигналов сохраняется без исполнения. | app/mod.rs Combi:5372/5454, Race:7164, NanoFreeTuning:10020 добавляют номера в first_use_checks. По текущему app-коду нет consumer этих накопленных векторов, кроме очистки. | cnCombiMode.cs:883 CheckCondition(3); прочие условия требуют отдельной сверки состояния/порядка, не следует просто открывать справку на каждый номер. |

## Устаревшие пункты, которые нельзя снова включать в разработку как отсутствующие

- Карточки Inventory/Bank/Vendor, аренда/expiry, длинные названия, combined, TryOn и OPEN: закрыты последними срезами; сохранён отчёт item-card-completion-acceptance.json.
- Вход в Combi из NPC уже есть в app/gameplay_ui_actions/npc.rs:71; камеры реализованы app/service_portrait.rs.
- Вход в Race и Rule уже существует в npc.rs:205/445. RaceRank остаётся отдельным разрывом U06.
- NanoCom SETTINGS уже подключён в modes.rs:56; прежняя запись native-ui.md о неподключённом входе устарела.
- Help карты уже обрабатывается в app/mod.rs:14764 через open_first_use(15).
- Bank drag transfer и однощелчковое перемещение существуют; отсутствует именно полная визуальная обратная связь и часть контролов.

## Что ещё требует проверки, а не утверждения об отсутствии

Обновление миссий после смены гида, instance/episode metadata карты, повторный вход/смена персонажа, alt-tab и конкуренция модальных окон, ошибки/таймауты обмена и групп, размеры окна/масштабирование, все hover/disabled состояния и соответствие primary GUISkin. Старые записи в native-ui.md по этим темам нельзя принимать за текущую истину без consumer trace. Cashmall/платёжные веб-панели не включены автоматически: сначала подтвердить, используются ли они в нашей игре.

Текущая проверка статическая: запуск живого сервера, две игровые сессии и новые GPU golden-сравнения не выполнялись. Имеющаяся проверка 54 сценариев карточек относится к карточкам и не доказывает работоспособность перечисленных сервисов. Нельзя честно вычислить процент готовности всего UI по числу Rust-модулей или PNG.

## Порядок следующей работы

1. U01+U02: банк/магазин — колесо, стрелки, удержание, ползунок, drag ghost/drop, отмена и modal/reset; EN/RU, два разрешения, настоящие указательные события.
2. U04+U03: завершить сервисные окна — помощь Combi, NPC animation owner, портрет Vendor и освобождение камер.
3. U05: обмен — ввод суммы и портреты, затем подтверждение/отмена/изменение предложения/таймаут с двумя клиентами.
4. U06: согласовать и подключить источник рекордов, проверить загрузку/пустой ответ/ошибку.
5. U07: проверить размещение пяти Barber NPC и серверные ответы, после этого переносить реально достижимый mode31.
6. U08 и сквозная матрица переходов/фокуса/локализации.

## Воспроизводимость

Повторно прочитаны и захешированы main.unity3d и TableData.resourceFile. Десять используемых C# файлов совпали по SHA256 с managed-files.tsv исходного аудита. Снимок хешей и исходных ссылок: artifacts/shared-ui-owners/ui-gap-audit-20260907.json. Старый whole-client-static-audit-20260905.md остаётся историческим полным инвентарём, не актуальным списком незавершённого UI.

Повторные команды из Editor: `rg -n "npc_preview_supported|VendorInteractiveControl" ../FFOneClient/crates/ffone-client/src/vendor_ui.rs`; `rg -n "Pc2pcBackendCapabilities" ../FFOneClient/crates/ffone-client/src`; `rg -n "OWNER_GAP|first_use_checks|RACE_RANK_HTTP" ../FFOneClient/crates/ffone-client/src/app/mod.rs`; чтение перечисленных методов primary из work/legacy-sources/whole-client-audit-20260905/main-decompiled. Только статический аудит; runtime-файлы не публиковались.

## Последующее исправление U03

Портрет продавца реализован в следующем проходе 20260907. Подробности и границы проверки: vendor-portrait-20260907.md. Остальные строки аудита этим не закрываются.

## Последующее исправление U01 и банковской части U02

Прокрутка банка/магазина и правого инвентаря, а также визуальная обратная связь bank drag/drop реализованы. 59 профильных тестов и 28 EN/RU GPU-сценариев прошли; финальный Dev прошёл офлайн World smoke. Границы и отдельный результат общего статического теста локализации: service-controls-20260907.md и artifacts/shared-ui-owners/service-controls/acceptance.json. Перетаскивание предметов в Vendor этим не закрывается.
