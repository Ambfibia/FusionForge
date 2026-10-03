# Item rating text binding

Evidence question: why are weapon/armor comparison values uncolored in FFOne
inventory and vendor cards despite the existing comparison function?

Authority is primary Retrobution 20260821, using the accepted managed assembly
chain in `docs/reference/evidence/cases/shared-ui-owners-20260905.md`. Rechecked
`work/legacy-sources/whole-client-audit-20260905/main-decompiled/EquipPopup.cs`:
SHA-256 `3d0db1a1034eb1f1c56d5b96c981fa817aaea352bc69195f61e0803543775ad4`.
`ValueColor` compares point/group/defense ratings against the same equipment
slot. Lines 91/93 define green `(0,1,0)` and red `(0.9,0.2,0)`; equal values
retain the base tint. `DrawWindow` lines 1685–1689 applies the corresponding
color independently to each rating. No new payload or source substitution.

The native comparison function already implements this contract. Inventory
`spawn_bound_text_styled` places `TextColor` on the child referenced by
`UserEquipBoundTextTarget`; `bind_user_equip_ui` was querying the layout parent,
which has no `TextColor`. Move rating updates into the existing child text
binding system. Vendor and bank cards keep their direct text-entity bindings.
Geometry, fonts, localization keys, table ratings and network authority remain
unchanged.

Regression coverage uses the production inventory text spawner and production
tables for weapon, upper, lower and foot slots, selecting better, worse and
equal items in sequence. Vendor's production binding test checks the actual
text components across item and EN/RU changes. Pure numeric comparison tests
retain empty-slot and equipped-item coverage.

The real Dev client and both GPU preview executables built successfully; the
Dev binary completed an offline world smoke run. Reviewed native GPU captures
show red point/group damage in inventory and vendor, green defense with an
empty slot, and normal defense with equal equipment. Replay settings and exact
capture SHA-256 values are recorded in FFOne `docs/native-ui.md`, section
"Item comparison colors (2026-09-12)". Captures are native fixture evidence,
not a live-server or running-Unity pixel comparison.

The two rating tests and vendor production card test passed in the freshly
built no-default-features library test binary (the crate has no default
features). Production EN/RU key/placeholder parity passed. The wider localization
filter recorded 39 passes, four failures outside this change and one ignored
benchmark; details are in the native UI acceptance note. A subsequent Cargo
test rebuild encountered concurrent NPC compilation errors after the successful
Dev build. No claim is made that all unrelated workspace tests pass.
