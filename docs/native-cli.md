# Native CLI contracts

Use cargo fusionforge --help or target/debug/fusionforge.exe. The direct model route is
specified in conversion-workflow.md. native calls the pipeline library; utility exposes
retained specialized operations; reference-audit runs parity auditing in process.

## Accepted domain rules retained during integration

The tables below describe existing recipes/implementations, not a mandatory multistage
workflow. repair-native still contains staged code and must be migrated
before they satisfy the direct-output contract. Do not execute historical repairs blindly.

| Recipe in `recipes/native/cli` | Output |
| --- | --- |
| `trigger-use-icon.json`, `new-mail-icon.json` | Exact scoped gameplay icons |
| `minimap-zoom.json`, `mob-rewards.json`, `tryon-background.json` | Accepted UI PNGs under `ui/en` |
| `game-guide-controls.json`, `game-guide-screenshots.json` | Primary guide images, without the unresolved `lteminfo.jpg` substitution |
| `hnpc-palette.json` | Complete zero-based skin and hair palettes |
| `audio-gaps-20260908.json` | Scoped OGG recovery and native TableData audio rows |
| `nanomachine-collision.json` | Exact collision GLB and validated visual/collision bindings |
| `traversal-trolley.json` | Audited logical model and all accepted dependent textures |
| `creation-expansion.json` | Fifty face PNGs and accepted customization data |
| `vehicle-trails.json` | Three exact trail textures and accepted native trail parameters |
| `dexter-placeholder-routes.json` | Only NPC 753/754 mesh routes, with scoped primary evidence and exact native mesh guards |
| `item-icons-primary.json`, `item-icons-alternate.json` | 198 primary and 6 explicit Academy item-icon restorations; additive publication refuses different existing artwork |

| Operation | `--target-root` | Additional inputs / contract |
| --- | --- | --- |
| `banker` | Native asset root | `--source-root` primary, optional `--navigation-root`; recover three parts and three textures, update only matching female slots 144/145/176/177/178 |
| `hnpc-clips` | Native asset root | `--source-root` primary; optional `--clips stand2,talk` and `--semantic-catalog <relative-json>`; scoped owner lookup and append-only preservation of native rigs |
| `player-damage-animation` | Native asset root | Rust HNPC pipeline with `woundupper` and `characters/player/shared/damage_animations.json`; clamp playback; no navigation dump needed |
| `player-emote-events` | Client repository | `--source-root` primary; 46 owner-validated clips produce native Rust event contracts; requires pinned `rustfmt`; source identities remain in Editor evidence |
| `nano-voice-events` | Native asset root | `--source-root` Academy, `--primary-root` Retrobution, `--navigation-root` Academy index; exact scoped events, Chowder clip ownership/rest-pose checks, current TableData Pirate SFX route |
| `deduplicate-textures` | Native asset root | Complete mip bytes, color and sampler equality; code-literal consumers are excluded; dependent hashes updated; removed originals retained in the case for rollback |
| `nano-identities` | Client repository | `--server-xdt`, `--source-root` primary; accepted Nano additions, quest/model guards, EN/RU parity |
| `unstable-powers` | Client repository | `--server-xdt`, `--source-root ../builds/retro-010920`; pinned Retro donor rows |
| `nano-tuning` | Client repository | `--server-xdt`, `--source-root ../builds/OG-academy`; accepted Cheese skill and unique tuning IDs |
| `sync-nano-tuning` | Client repository | `--server-xdt`; synchronize only the accepted native tuning repair, preserving other server fields |
| `retire-nano-alias` | Client repository | `--server-xdt`; reserved alias 52 only, preserving 41 and 66 ownership |
| `unstable-power-icon` | Client repository | `--server-xdt`; skill 122/icon 68 plus the hash-pinned native PNG |
| `fred-skin` | Native asset root | `--input <GLB> --expected-input-sha256 <hash> --output <relative-GLB>`; only first-two-influence weight bytes change |
| `vehicle-routes` | Native asset root | Native tables/catalogs; reports reserved rows and unresolved extensions |
| `area51-vortex` | Native asset root | Exact twelve-shell controller and tracks; independent repeat with only proven closing samples |
| `candy-cove` | Native asset root | Three foliage ownership fixes, two redundant visual shells, dependent counts and hash bindings |
| `audio-paths` | Native asset root | `--recipe recipes/native/audio/scamper-pilot-paths-20260908.json` (or the departure plan); hash-checked filename migration with rollback |

Historical `unstable-power-icon` used icon 68; the accepted source-contract.md specifies
67. Inspect current production rows and the latest recipe before changing that identity.

Recipes pin raw SHA-256/size and scoped object ownership. native-json retains field order,
number spelling and explicit LF/CRLF. JSON patches distinguish null from missing and
preserve untouched spans. Client/server tables and EN/RU keys/placeholders are validated
as one change. Server publication targets RustyFusion's configured general.table_data_path.

Banker compatibility is limited to its pinned historical compressed-normal interpretation;
it must not revert the general mesh decoder. Reuse PNG encoding only after exact RGBA
comparison. Output textures belong under characters/hnpc/textures.

Texture sharing requires complete mip, color and sampler equality, preserves placements,
passes, GLB BIN data and domain ownership. Terrain sharing remains within its tile.
Animation appends preserve hierarchy/rest pose, existing resources and clip ownership;
--retain-static-root-scale permits only the established static-root exception.

Mordecai/Titan cel materials preserve alpha, depth, passes, geometry and animation.
Darwin normal repair affects only Bone_Head accessors: equal positions/weights define
adjacency, with area weights and a 60-degree crease; no welding. Other bytes remain intact.

The migration registry is reference/evidence/cases/native-cli-migration-20260917.json. It retains
removed script hashes, replacement commands and comparisons. Its complete flag stays
false while any active publisher still needs migration. Receipt schemas are historical
evidence, not executable CLI recipes or mandatory new output.

## Historical library APIs

`audit_asset_tree`, `validate_route_plan` and `publish_route_plan` belong to the offline
semantic-asset tools. The latter's staged/locked/snapshot publication is migration debt,
not the direct converter contract. `build_retro_world_plan` is pre-migration; its guard
forbids republishing into a world-v1 target. Existing world route tools must preserve
byte/BLAKE3 guards, GLB URI safety, placements, category/identity and ownership.

