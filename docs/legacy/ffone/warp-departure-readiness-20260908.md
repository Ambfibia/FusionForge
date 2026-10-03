# Native warp departure readiness

Evidence question: can FFOne send an NPC/transportation warp request before its
departure effect has finished? The owner clarified that the player is visible
again at the destination; the reported defect is disappearance during departure.

This investigation concerns the current native adapter. No raw Unity container,
managed assembly, model, texture, or animation was extracted or published. The
historical `normal-npc-warp-parity.md` describes the earlier recovered 1.5-second
request delay; its June 2026 source is now canonical `previous`, not the current
August `primary`. This note does not establish a new primary parity claim.

## Native diagnosis and contract

Both production routes enqueue ES394 and advance a request timer independently
of the renderer. ES394 is a particle-only effect: its published closure has
`maxTimer = 1.0` and `longestLifeTime = 1.0`, giving the native root a two-second
lifetime. The 1.5-second request delay can therefore expire before the last
particles finish. A fast server reply moves/hides the player during that tail.
Native spawn queuing can make the disagreement larger. Mesh-surface waiting was
an initial general hypothesis; ES394 does not use a mesh scene, so that is not
the specific cause of this report.

The repair keeps 1.5 seconds as the minimum request delay and preserves each
route's comparison operator, but also waits for the exact departure instance to
finish. This intentionally extends the request wait to cover ES394's complete
tail, as requested by the owner; it is not claimed as unchanged legacy timing.
The native clock starts after the renderer opens the playback barrier. The
first observed ready frame contributes no delta because that interval began
before the ready edge. Elapsed time is released to the request timer when the
instance finishes. A new departure resets the clock, and a pending named effect
replacement cannot inherit its predecessor's readiness. Readiness belongs only
to the live runtime instance and is discarded by instance/scene cleanup.

Only the correlated server reply applies the destination. Destination collision
loading, movement suppression, visibility restoration, native assets, UI copy,
voice resolution, and animation curves remain owned by their existing systems.

## Verification

Focused native regressions simulate a five-second cold load followed by the
complete departure interval and still-live tail for NPC warp and transportation. Renderer coverage
checks that the readiness acknowledgement follows the same surface barrier as
effect lifetime; replacement coverage checks cancellation and stale readiness.

Commands from FFOneClient:

```powershell
cargo check -p ffone-client --bin ffone-client
cargo test -p ffone-client --bin ffone-client warp -- --nocapture
cargo test -p ffone-client --lib named_warp_presentation
cargo test -p ffone-client --lib warp_departure_effect_tail
cargo test -p ffone-client --lib mesh_effect_lifetime_begins_only_after_a_renderable_surface_exists
cargo build -p ffone-client --bin ffone-client
```

Verification completed on 2026-09-08:

- `cargo check` and the real `cargo build` above succeeded.
- The binary `warp` filter passed all 16 tests, including both production
  departure paths. Its first run found a missing active NPC session in the new
  transportation fixture; initializing that session fixed the fixture, and the
  complete 16-test filter passed on rerun.
- The three exact library filters above each passed against Cargo's newly built
  `target/debug/deps/ffone_client-6bd263b67a1f7b08.exe` (one test each). They were
  invoked directly while the shared Cargo queue was busy. A queued broader
  `cargo test --lib warp` run was stopped after those focused checks passed to
  avoid an unnecessary repeat build.
- Compilation reported the existing unused `CameraProjection` import warning.

No live-server or GPU departure/destination capture has been accepted for this
repair. The validated result is the native timing/request contract, not a new
visual parity certification.
