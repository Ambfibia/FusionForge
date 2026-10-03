# Male SCAMPER pilot dialogue paths

This is a path-only repair of existing native Russian OGG files. No extraction,
conversion, audio payload modification, or new legacy parity claim is involved.
The adjacent JSON records the original basename, final native path and unchanged
SHA-256 for each of ten greeting/farewell files belonging to m_kndtransport1/2.
The target's native_asset_routes already declares the final paths for EN and RU.

Replay from FFClientEditor:

```powershell
./tools/native/repair-scamper-pilot-paths.ps1 -TargetRoot ../FFOneClient/assets/game
```

The publisher verifies hashes and is idempotent. The production-bundle regression
test `scamper_pilot_voice` passed: both owners, two greetings and three farewells,
both locales, exact path resolution with no fallback, and non-silent OGG decoding.
It was compiled directly against the current prebuilt foundation/Bevy dependencies
because a concurrent client build held Cargo's shared build lock.

Greeting03 and ClickWarp02/03 remain untouched. They are not declared by the
existing greeting / ClickMove routes. No evidence established a ClickWarp to
ClickMove mapping; do not infer that mapping or substitute take numbers.
