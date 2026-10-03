# FFOne против всего Retrobution 20260821: статический аудит

Дата: 2026-09-05. Источник: `retrobution`, canonical role `primary`.

Историческая ревизия полного состава клиента; статусы W01..W20 не являются текущей
очередью задач. Часть UI была исправлена позднее. Реестр 89 сообщений владельца —
`ffone-completion-plan-20260905.md`; он не повторяется здесь.

**Граница результата:** выполнены полный инвентарный проход по сырым контейнерам, обеим игровым managed-сборкам и нативному дереву, структурная сверка данных и адресное исследование найденных разрывов. Это не доказательство эквивалентности каждой инструкции, всех PPtr-графов, пикселей и сетевых сценариев. Автоматические лексические совпадения отдельно помечены кандидатами. Неустановленная достижимость не превращается ни в «не реализовано», ни в «паритет».

## 1. Охват и воспроизводимость

| Слой | Реально выполнено |
|---|---|
| Сырые исходники | Все 361 контейнер из checked inventory открыты FusionForge; 361/361 успешных завершений. Каждый исходный файл прочитан для SHA-256. Размеры совпали с inventory/index. |
| Сериализованные объекты | Свежий raw listing содержит 1 819 064 объекта, количество совпадает с индексом. Сохранены container + serialized asset + type + pathId. |
| Маршруты AssetBundle | 84 196 индексных route-записей; это ссылки/повторы, не число уникальных моделей. Дополнительно заново прочитан raw route-каталог Effects. |
| Игровой managed-код | Заново декомпилированы основная DLL и first-pass: 857 + 232 = 1089 C#-файлов, 179 135 строк. Для каждого записаны хеш, объявления, lifecycle, пакетные токены и кандидаты нативных модулей. |
| Полнота связей MonoScript | В raw есть 589 MonoScript-объектов с 257 именами; 28 имён не найдены среди объявлений двух игровых DLL. Это кандидаты старых/внешних/удалённых скриптов, а не 28 обязательных нативных систем. |
| FFOne Rust | Прочитаны все 288 `.rs` в crates, 496 388 строк; сохранены хеши и 60 кандидатов явных незакрытых границ. Количество включает тесты/кодеки и не является метрикой готовности. |
| Нативные ассеты | Прочитаны/хешированы все 65 919 файлов, 3 424 495 312 байт; разобраны 15 886 JSON и заголовки/JSON всех 12 005 GLB. |
| Материалы и анимации GLB | 10 065 material-записей, 6874 animation-записи, 1806 skin-записей. Внешние material-контракты static-world учитываются отдельно: отсутствие inline `extras.ffone` не объявлено пропуском шейдера. |
| Ссылки изображений | Проверены 10 910 внешних GLB image URI и mip URI в inline-контрактах. Найдены четыре отсутствующих изображения, они же отсутствуют в mip-контрактах. |
| Таблицы | Строго извлечён объект `xdtdatas`; структурно сравнены все 41 раздел XDT с текущим consolidated native table-set. Отдельно строго извлечены `all_hnpc`, `worldname`, `clientnpc`. |
| Аудио | Все 13 436 индексных AudioClip сопоставлены с 10 200 native catalog entries; наличие каждого файла каталога проверено. Name lookup повторяет lowercase нормализацию runtime, но не подменяет проверку байтов. |
| Мир | Разобраны все 170 native behaviour-документов; дополнительно заново выполнен raw behaviour-export Map_01_13. Полная новая PPtr/transform-сверка всех 173 карт не выполнялась. |
| Локализация | EN/RU: по 65 835 ключей, различий наборов и template placeholders нет. Это не оценка качества перевода. |
| Сборочный граф | Повторный `cargo run -p xtask -- assets --source assets/game --full` завершился ошибкой владельца HNPC-текстуры. Rust-компиляция xtask актуальна, ошибка повторена после проверки через Cargo. |

Точное основание:

- primary main SHA-256: `01b544976b2d54355507cf30fe6dfada2b476b92b209a3d47c1499669ed9b4ef`;
- основная DLL: `0f2513c2303cffc88c541a90b1b8fad1b73f60d5218830be72d0502533bf5792`;
- first-pass DLL: `afff470bec30bf06703eb0664de364ce37ea7b8092c848420a92fba63e91da81`;
- Editor HEAD: `86324d05303a076b9814bddff800635cb2f73954`;
- FFOne HEAD: `232a41ece84bfb26d18e902f93fd5068588cad40`;
- оба дерева содержат незакоммиченные изменения; хеш-инвентари описывают именно исследованные файлы, не только HEAD.

В main найдены три DLL: две игровые и System.dll. System.dll — платформенная библиотека, не отдельный игровой модуль для переноса в Rust. Нативные внутренности UnityPlayer не были предоставлены как исходники; engine-семантика требует явных контрактов Bevy.

353 дополнительные строки raw listing не разобраны как самостоятельные TSV-записи: в некоторых именах/декодированном выводе имеются управляющие символы. Они сохранены как unresolved. Совпадение общего количества объектов не доказывает правильное имя каждого объекта. 283 Shader-объекта имеют пустое имя в этом представлении; нельзя заключать из listing, что их shader programs отсутствуют.

## 2. Подтверждённые разрывы, найденные вне списка жалоб

### W01. Парикмахерская — отдельный режим Retrobution, потерянный при сравнении enum

`main-decompiled/GameFrame.cs:1012–1026` создаёт GameObject Barber, добавляет BarberMode/BarberGui, копирует камеру/GUI skin и назначает `GameModes[31]`. В том же файле около 1232 подключены ReceiveEvent и ReceivePacket. `eGameMode` заканчивается Max, поэтому перепись только enum пропускает этот режим.

В FFOne есть четыре wire-типа Barber в `ffone-protocol/src/wire_0104.rs`, но поиск Barber по полному native runtime не находит режима/адаптера. В проверенном OpenFusion нет REGISTER_SHARD_PACKET для BARBER. Нужны UI + preview + подтверждение внешности + серверный контракт; это не только отрисовка ещё одной панели.

### W02. Банк: typed-операции существуют, реальные drag/popup-контролы не подключены

`ffone-client/src/bank_ui.rs:1926` явно перечисляет BankDisabledControl: bank/inventory/equipment slots, active item tab, redeem, trash, help и scrollbar controls; они создаются с `Pickable::IGNORE`. `collect_bank_ui_input` обрабатывает закрытие и колесо. Наличие BankUiState/request и сетевого bank_runtime не подтверждает полный пользовательский путь перемещения вещей мышью. Адресно проверить внешние observers, затем подключить недостающих владельцев взаимодействия; не дублировать уже имеющийся typed runtime.

### W03. Общая система предметных popup не завершена

`app/mod.rs:20643` оставляет Vendor Equip/Gum/Turing popup без native owner и не отправляет соответствующую транзакцию. В Enchant около 5475–5481 и 6274–6322 явно отсутствуют redeem text-entry window 99, item preview и Help. Те же владельцы нужны Cashmall/Bank/другим окнам. Это общий функциональный пакет, который стоит завершать до отдельных косметических исправлений каждого окна.

### W04. Combi/Enchant: сетевой runtime уже есть, но анимация и камеры NPC отсутствуют

`app/mod.rs:3126–3129` и `5475–5481` фиксируют отсутствующих владельцев Combi camera, event-animation и Help; Enchant camera с framing 1300/550 Neck, event-animation, Help и popup. Продюсеры этих intent существуют и доходят до сообщения о gap. Старое утверждение «Combi/Enchant вообще не подключены к сети» неверно; пробел нужно закрывать на уровне конкретных выходов runtime.

### W05. Рейтинг гонок не имеет рабочего HTTP-пути

`cnRaceRankMode` оригинала содержит LoadWww/WWWForm; нативный `app/mod.rs:6383,7148` явно блокирует режим без ordered HTTP POST owner. Обычная RaceMode и RaceRankMode — разные системы. Реализация рейтинга требует действительного backend endpoint/совместимого сервиса, а не выдуманных результатов локального UI.

### W06. Уличные лавки недоступны несмотря на наличие UI

`user_store_runtime.rs:264–306`: обе точки входа registry-probe проверяют пакет и возвращают ProductionOwnerBridgeUnavailable; текущий backend также не регистрирует нужный entry request. `app/mod.rs` содержит guard/отбрасывание не принадлежащих production команд. Для полного продукта требуется решение о поддержке сервером и полноценный owner жизненного цикла лавки.

### W07. Buddy warp между шардами не завершён

`app/social_ingress.rs:1110` принимает WarpOtherShardSuccess, очищает pending, но выводит сообщение о непроверенном shard handoff. Same-shard ветка обрабатывается отдельно. Это конкретный незаконченный переход, который не обнаруживается одиночным входом и локальным телепортом.

### W08. Recall Point: нет принятого состояния регистрации и части входа сервиса

`app/mod.rs` около 17424 блокирует Recall Nano auto-route категории 17 без typed RXCom owner; около 17940 передаёт `false` как признак регистрации для иконки ES446. Поэтому наличие модели/иконки XCom не означает работоспособную регистрацию точки.

### W09. Обновление заданий после смены гида не подключено полностью

`app/mod.rs:21098`: после authoritative Guide change ветка refresh_guide_missions сообщает, что world Guide-mission refresh недоступен. Это отличается от отсутствующей приветственной реплики: сначала нужно завершить обновление игрового состояния.

### W10. Shiny/яйца: транспорт сущностей есть, consumer взаимодействия не найден

`entity_lifecycle.rs` принимает SHINY_AROUND/ENTER/NEW/EXIT и создаёт NetworkShinyAppearance0104. По всему gameplay runtime не найдено использования этой appearance вне самого lifecycle, либо SHINY_PICKUP/ShinyPickup. Это адресный разрыв между сетевой сущностью и видимым/подбираемым объектом; не нужно сначала повторно реализовывать уже имеющийся сетевой spawn.

### W11. Видеотекстура телевизора Юстаса не воспроизводится

У `characters/player/items/back/back_eustacestv/models/back_eustacestv/model.glb` материал 1 `_MainTex` содержит DynamicTextureBinding MovieTexture. Оригинальный AutoplayMovie.Start вызывает Play на соответствующей текстуре. `legacy_model_material/mod.rs:2220` сохраняет/валидирует данные, но до появления gameplay video owner использует объявленный fallback. Это подтверждённое отсутствие runtime-воспроизведения, не потеря Ogg payload.

### W12. Базовое движение расходится с заданным primary

`m_pAvatarTable/m_pAvatarData/1/m_iRunSpeed`: primary **800**, native **600**. `movement.rs:68` дополнительно фиксирует baseline 600, есть тест с таким значением. Это реальное отличие контракта данных/кода. Автоматически менять скорость нельзя: отдельно проверить серверную конфигурацию и является ли это дополнительным намеренным изменением владельца.

### W13. Неполная таблица состояний анимации

Primary avatar table: 214 строк, native: 205. Отсутствующий хвост: `report`, `coin`, `observe`, `scratch`, `knockdown`, `stickreadyspell`, `talkquestion`, `talkexclamation`, `talk`. Primary nano table: 34, native: 33; отсутствует `spellcasting`.

Это доказанный пропуск таблицы относительно 20260821, не утверждение, что все соответствующие кривые отсутствуют в GLB. Следует проверить consumers номера состояния и clip lookup отдельно; сперва восстановить семантическую таблицу и доказать поведение.

### W14. Warp-таблица и клиентские координаты NPC не полностью обновлены до 20260821

- `m_pWarpData` и `m_pWarpNameData`: по 323 записи в primary, по 321 в native; отсутствуют warp 321/322, связанные с source NPC 3434/3435.
- Warp 143 имеет изменённые XYZ: `(325987,163638,-2109)` → `(323800,163068,-1602)`. Намеренность не установлена.
- `clientnpc`: 2907 исходных строк против 2903 native; 117 отличающихся общих позиций массива. По реальному first-ordered-match lookup получаются 12 source-ID кандидатов отличий: 704,796,975,976,3431,3434,3435,3437,3438,3441,3444,3447.
- Для 3430–3450 обязательно применить принятую source→native схему 3469–3489 перед окончательным решением об отсутствии. Сырой ID нельзя использовать для перезаписи авторских NPC. См. npc-repair-20260905 receipt.

### W15. Расхождения создания персонажа, справки и правил

`CreationItemData`: 52 против 32; это не 20 автоматически отсутствующих моделей, а различающийся набор вариантов/строк. RulesData: 4 против 3, RulesString: 34 против 23. HelpTable имеет сокращения в массивах. Все значения/длины и различающиеся поля сохранены в полном table-delta-details.json; нужна классификация по UI consumer, а не слепая замена всей таблицы.

### W16. Неполная публикация эффектов — есть проверенная связь с gameplay

Raw Effects содержит ES[0]…ES[871], всего **872**. Каталоги `map/shared/effects/catalog.json` и `map/shared/projectiles/catalog.json` покрывают **236** разных ID; **636** не имеют записи в этих каталогах.

Не каждый исходный ES обязан использоваться. Но в первичной SkillBuffData 20 ненулевых ссылок в `m_iBuffEffect`/`m_iBuffEffectInstant`; **17 ссылок на 13 ID** не представлены в каталогах: 368,370,411,412,429,431,434,435,436,441,447,714,805.

`Status.cs:703,840,1912` читает эти поля, а ProcessInstantBuffEffect при ненулевом значении запускает LoadBuffEffectPriority. Raw routes ES368/429/714 повторно проверены в Effects. Это конкретная незакрытая цепь эффектов статуса/баффов. Числовой серверный бафф и его визуальный эффект — разные критерии готовности.

Дополнительная автоматическая выборка по всем XDT полям с `Effect` даёт 105 разных ID без записи каталога. Она намеренно помечена кандидатами: не каждое поле Effect имеет семантику ES-ID, потребитель обязателен.

### W17. Четыре GLB с разорванными texture/mip-ссылками

Проверка всего native GLB-дерева нашла ровно четыре отсутствующих image URI:

- melee_eduardoclub → `melee_eduardoclub.textures/ToonRamp9.png`;
- melee_razor → `melee_razor.textures/ToonRamp9.png`;
- rocket_mojotankcannon → `rocket_mojotankcannon.textures/ToonRamp9.png`;
- shattergun_wilt → `shattergun_wilt.textures/ToonRamp9.png`.

В каждом случае это относительная ссылка внутри соответствующего `characters/player/items/weapon/.../models/.../model.glb`. Та же ошибка присутствует в inline mip-контракте. Исправление должно воспроизводиться publisher-ом в Editor с сохранением правильного общего ToonRamp9 и sampling/color semantics. Не копировать наугад первый одноимённый PNG.

### W18. Граф нативных ресурсов не проходит полную валидацию

Свежая проверка Cargo/xtask остановилась на `textures/hnpc/m_face_006_a.png has no declared runtime owner group`. Отдельный GLB-аудит не заменяет packaging owner graph. Пока этот слой не исправлен и полная проверка не пройдена, «все ассеты валидны» неверно.

### W19. Общая Help/Computress и вспомогательные владельцы UI

`app/mod.rs:22179` отмечает отсутствие отдельной Help/Computress surface. Vendor, Combi, Enchant, Guide и WorldMap возвращают соответствующие недоступные Help-ветки. Существующий game_guide_ui не доказывает, что все эти страницы/события связаны с ним. Нужен единый аудит страницы→событие→владелец, отдельно от удаления shortcut по предпочтению владельца.

### W20. Группы/социальное: часть мутаций намеренно отвергается

`group_runtime.rs:102` выделяет UnsupportedGroupAction0104: KickMember, TransferLeader, MutateBlockList; unsupported_action возвращает ошибку. Приглашения/вступление уже имеют работающие typed пути. Проверить достижимость каждой команды в Retrobution и поддержку сервером: не переносить весь набор вслепую. Блокировка сообщений по уже загруженным данным не равна операции изменения block list.

## 3. Что не следует ошибочно объявлять пропуском

1. **Модели персонажей/экипировки.** Повторный asset-census не нашёл новых публикуемых из primary пропусков в своих трёх доменах. 395 XDT character references, 19 unresolved имён: 7 donor-only, 11 absent, 1 доказанно невидимый RXcom. Это не проверка всех материалов, скининга, поведения и raw неиспользуемых ресурсов.
2. **Три карты.** 173 исходных позиции против 170 native terrain tiles: 00_08,01_09,11_15 уже доказаны placeholders, заимствующими terrain 00_09. Нельзя создавать фальшивый terrain для выравнивания счётчиков.
3. **WorldName.** В коде указан старый источник, но свежая проверка 94 непустых прямоугольников по порядку даёт точное равенство с 20260821. Нулевая область с именем пути character-creation не является недостающей игровой локацией.
4. **HNPC.** Primary/native содержат по 205 appearance-записей. В пяти сравниваемых числовых атрибутах найдено 17 отличий; их нельзя отменять, не учитывая свежие намеренные исправления/варианты. Mesh/texture parts требуют отдельной семантической проверки.
5. **Кинематические Rigidbody.** В native behaviour есть 1542 rigid body, все имеют isKinematic=true. Один флаг useGravity не доказывает отсутствующую физическую симуляцию платформ; движение управляется специальными владельцами.
6. **Пакеты.** Поиск только P_* ошибочно объявляет отсутствующими mail/race и другие обработчики, использующие локальные имена/числовые ID/typed codeсs. После расширенного поиска остаются 194 кандидата без token/type/numeric references вне protocol; это не 194 доказанно отсутствующих обработчика и не процент покрытия.
7. **Шейдеры.** Разница числа Shader-объектов и native shader names не имеет смысла как оценка полноты. Объекты повторяются, имена могут не декодироваться, static-world использует внешний контракт. Пользовательские шейдеры персонажей остаются намеренными.
8. **Аудио.** NativeAudioCatalog::by_true_name использует lowercase. После этой нормализации остаются 243 исходные AudioClip-записи с 236 уникальными нормализованными именами без прямого trueName-кандидата. Это очередь разрешения aliases/переименований/точных bytes, а не 243 потерянных файла. Все файлы текущего аудиокаталога существуют.

## 4. Все режимы оригинала: карта сравнения

Статус «partial» ниже означает, что найден нативный владелец, но не заявляется полная поведенческая приёмка. Номер — реальный GameFrame mode, а не счётчик Rust-модулей.

| Mode | Primary | Native owner / итог |
|---:|---|---|
| 0 | Null | служебный sentinel; не игровой экран |
| 1 | Login | login_ui + network; partial, проверить ошибки/повторный вход |
| 2 | CharacterSelection | character_selection_ui/portraits/scene; partial, музыка и очистка сеанса отдельно |
| 3 | CharacterCreation | character_creation_ui/data; partial, CreationItemTable различается |
| 4 | NameCreation | часть character_creation_ui; partial, серверные проверки имени |
| 5 | MainGame | app + gameplay/world; partial, полный набор gameplay owners не закрыт |
| 6 | UserEquip | user_equip_ui/runtime + inventory; partial, мышь/popup/подтверждения |
| 7 | MissionSystem | mission_ui/world_mission_runtime; partial, просмотр/принятие/завершение/сохранение |
| 8 | Vendor | vendor_ui/runtime; partial, popup и preview gaps W03 |
| 9 | Pc2pc | pc2pc_ui + app offer integration; partial, камеры/модальные поля/двухклиентный сценарий |
| 10 | NpcIcon | gameplay NPC service dispatch; partial, RXCom W08 |
| 11 | Bank | bank_ui/runtime; partial, W02 |
| 12 | Option | option_ui/user_settings; намеренные языки сохранены, gamepad/presentation branches partial |
| 13 | Launcher | launcher_ui + world triggers; partial, combat-icon/name visibility/escape owners |
| 14 | Resurrect | resurrect_ui + app; partial, прежние scoped исправления не заменяют серверную приёмку |
| 15 | WorldMap | world_map/gameplay_ui; partial, lifecycle/filter/help |
| 16 | RaceMode | race_ui/mode + app network ingress; partial, не путать с рейтингом |
| 17 | RaceRankMode | shell есть; рабочий HTTP-путь blocked W05 |
| 18 | Email | email_ui/runtime + app; partial, полноценный compose/attachment/reward round-trip |
| 19 | Transportation | transportation_ui + transport runtime/portrait; partial |
| 20 | Guide | guide_ui/runtime; partial, refresh W09 |
| 21 | DexterScene | tutorial choreography/presentation; partial, порядок видео/голоса/событий |
| 22 | NanoFreeTuning | nano_free_tuning_ui/runtime; partial, весь набор состояний/ошибок |
| 23 | QuitMenu | quit_menu_ui/runtime; partial, полный session teardown/relogin |
| 24 | Upsell | upsell_ui; billing callback/teardown unavailable, требует продуктового решения |
| 25 | Combi | combi_ui/runtime; partial, W04 |
| 26 | ServerSelection | server_selection_ui; reachable скрытые ветки отдельно, не объявлять обязательными без primary gate |
| 27 | Cashmall | cashmall_ui + local app adapter; shared item owners unavailable, коммерческий backend не выдумывать |
| 28 | UserStore | shell + guard; production entry blocked W06 |
| 29 | Enchant | enchant_ui/runtime; partial, W03/W04 |
| 30 | Rule | rule_ui/runtime; локальный режим без собственного network owner по контракту, таблица различается |
| 31 | Barber | динамически созданный режим; native runtime отсутствует, W01 |

Системы вне GameFrame modes тоже входят в реестр: movement/camera, animation, streaming, environment, effects, audio/video, event controllers, object pickup, packet dispatch и table adapters. Наличие 31 окна не означает завершённую игру.

## 7. Артефакты и команды

Исторические локаторы (файлы могут отсутствовать в копии): `FFClientEditor/work/legacy-sources/whole-client-audit-20260905/`. Не создавать это дерево для обычной конвертации:

- `summary.json`, `contracts-summary.json`, `raw-objects-summary.json`, `render-summary.json` — сводки;
- `source-containers.tsv` — 361 файл с SHA-256;
- `source-objects.tsv`, `source-script-shader-movie-objects.tsv`, `raw-object-listings/` — полная scoped object inventory;
- `source-routes.tsv`, `effects-raw-routes.txt` — route evidence;
- `managed-files.tsv`, `managed-types.tsv`, `managed-method-candidates.tsv` — оба assembly inventory;
- `native-files.tsv`, `native-explicit-gap-candidates.tsv` — полный native source inventory;
- `packet-numeric-candidates.tsv` — последняя версия пакетных кандидатов с учётом suffix/case/numeric/typed references; более простой packets.tsv не использовать как coverage;
- `native-assets.tsv`, `native-glb-image-refs.tsv`, `native-reference-issues.json`;
- `native-materials.tsv`, `native-animation-clips.tsv`, `native-dynamic-textures.tsv`;
- `primary-xdt.evidence.json`, `tabledata-{6,8,9}.evidence.json`, `table-deltas.tsv`, `table-delta-details.json`;
- `table-effect-references.tsv`, `skill-buff-effect-gaps.tsv`, `effects.tsv`;
- `world-behaviour-per-tile.tsv`, `map-01_13.behaviours.json`;
- `audio-runtime-name-candidates.tsv` — runtime lowercase matching; прежний exact-case audio-candidates.tsv только для истории discovery;
- `artifact-manifest.json` — хеши артефактов/инструментов на момент формирования.

Инструменты `tools/legacy-sources/audit-whole-client*.py` относятся к этому
полному исследованию; некоторые закреплены на source/case константах и записывают
отчёты. Это не стандартная конвертация и не проверка для обычного исправления.
Для нового адресного исследования сначала использовать ограниченный запрос FusionForge.

Дополнительно завершён read-only `node tools/audit-render-duplicates.mjs` из FFOneClient. Отчёт: `FFOneClient/target/performance/render-duplicates.json`. Найдены 3302 повторных PNG-записи по равенству закодированных байтов (45 809 774 повторных байта), 4785 кандидатов повторных статических материалов и 47 кандидатов совпадающих размещений. Это не доказательство равенства полного material/sampler/mip контракта и не разрешение удалять размещения. FPS/потребление GPU этим инструментом не измеряются; выигрыш производительности не заявляется.

Проверки текущего прохода: четыре Python-модуля проходят py_compile; все 361 raw-list вызова завершились успешно; все таблицы извлечены со строгими serialized-asset/type selectors; full asset validation **не прошла** по описанному owner group. Игровой сервер, GPU-приёмка каждого объекта и полный Cargo test suite в этом аудите не запускались. Изменены только Editor-инструменты и отчёты, нативная реализация/ассеты не исправлялись.

## Уточнение объёма UI от владельца, 2026-09-06

Исторические находки по наличию классов/ассетов выше не являются очередью обязательного переноса.
Enchant не используется в этой игре: исключён из дальнейших работ и критериев закрытия пункта 3.
Для Cashmall и остальных спорных режимов сначала требуется доступный игровой вход и поддерживаемый
сценарий. Отсутствие реализации никогда не вызываемого окна не считается незакрытым UI.
Вендор и банк остаются в работе; банк доступен через NPC-сервисы 12/50 и обслуживается сервером.
Актуальные результаты и ограничения пункта 3 находятся в shared-ui-owners-20260905.md.

Поздние кейсы сентября описывают дальнейшие работы с Enchant. Поэтому уточнение
владельца от 2026-09-06 выше — историческое ограничение того прохода, не актуальное
разрешение удалять реализованный режим. Выбирать объём по текущей задаче.
