#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const REPOSITORY_ROOT = resolve(process.env.FFONE_CLIENT_ROOT ??
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../FFOneClient"));
const CLIENT_SOURCE_ROOT = join(REPOSITORY_ROOT, "crates", "ffone-client", "src");
const DEFAULT_FILE_LIMIT = 5_000;
const DEFAULT_FUNCTION_LIMIT = 300;

// Existing oversized feature files are architectural debt, not a precedent. Their exact
// current sizes are ratchets: they may shrink, but cannot grow without changing this review gate.
const FILE_DEBT_LIMITS = new Map([
  ["crates/ffone-client/src/app/mod.rs", 25_258],
  ["crates/ffone-client/src/character_creation_ui.rs", 5_018],
  ["crates/ffone-client/src/combi_ui.rs", 5_483],
  ["crates/ffone-client/src/email_ui.rs", 6_333],
  ["crates/ffone-client/src/enchant_ui.rs", 5_875],
  ["crates/ffone-client/src/legacy_model_material/mod.rs", 7_486],
  ["crates/ffone-client/src/gameplay_ui.rs", 7_480],
  ["crates/ffone-client/src/native_terrain.rs", 5_830],
  ["crates/ffone-client/src/option_ui.rs", 8_286],
  ["crates/ffone-client/src/pc2pc_ui.rs", 6_020],
  ["crates/ffone-client/src/tutorial_choreography.rs", 6_205],
  ["crates/ffone-client/src/tutorial_effects_runtime/tutorial_native_effects.rs", 6_313],
  ["crates/ffone-client/src/tutorial_mission_content.rs", 7_441],
  ["crates/ffone-client/src/mission_ui.rs", 8_101],
  ["crates/ffone-client/src/user_equip_ui.rs", 9_710],
  ["crates/ffone-client/src/user_store_ui.rs", 5_967],
  ["crates/ffone-client/src/vendor_ui.rs", 5_750],
  ["crates/ffone-client/src/world_behaviour.rs", 6_123],
]);

// Filled only for functions that predate this gate. Like file budgets, these are non-growth caps.
const FUNCTION_DEBT_LIMITS = new Map([
  ["crates/ffone-client/src/app/gameplay_ui_actions/npc.rs::handle_npc", 569],
  ["crates/ffone-client/src/app/mod.rs::consume_nano_free_tuning_production", 317],
  ["crates/ffone-client/src/app/mod.rs::drive_dexter_ship_cutscene", 439],
  ["crates/ffone-client/src/app/mod.rs::collect_world_npc_interactions", 587],
  ["crates/ffone-client/src/app/mod.rs::enable_player_after_native_collider_ready", 329],
  ["crates/ffone-client/src/app/network_ingress.rs::handle_world_ready", 543],
  ["crates/ffone-client/src/app/network_ingress.rs::handle_gameplay_frame", 1_267],
  ["crates/ffone-client/src/app/network_ingress.rs::dispatch_network_events", 323],
  ["crates/ffone-client/src/app/schedule.rs::run", 1_229],
  ["crates/ffone-client/src/app/tutorial_choreography.rs::apply_tutorial_choreography_events", 416],
  ["crates/ffone-client/src/app/tutorial_presentation.rs::sync_gameplay_hud", 322],
  ["crates/ffone-client/src/app/tutorial_presentation.rs::tutorial_ui_for_stage", 302],
  ["crates/ffone-client/src/app/tutorial_runtime.rs::drive_local_tutorial", 402],
  ["crates/ffone-client/src/cashmall_ui.rs::bind_cashmall_ui_0104", 326],
  ["crates/ffone-client/src/character_creation_ui.rs::spawn_appearance", 332],
  ["crates/ffone-client/src/character_selection_ui.rs::spawn_character_selection_ui", 577],
  ["crates/ffone-client/src/combi_ui.rs::spawn_combi_main_group", 444],
  ["crates/ffone-client/src/combi_ui.rs::bind_combi_ui", 529],
  ["crates/ffone-client/src/enchant_ui.rs::spawn_enchant_main_group_0104", 363],
  ["crates/ffone-client/src/enchant_ui.rs::bind_enchant_ui_0104", 490],
  ["crates/ffone-client/src/gameplay_audio.rs::drive_gameplay_audio", 310],
  ["crates/ffone-client/src/legacy_model_material/mod.rs::for_shader", 346],
  ["crates/ffone-client/src/legacy_model_material/mod.rs::canonical_expected_passes", 327],
  ["crates/ffone-client/src/pc2pc_ui.rs::bind_pc2pc_ui", 306],
  ["crates/ffone-client/src/transportation_ui.rs::spawn_transportation_presentation", 371],
  ["crates/ffone-client/src/tutorial_effects_runtime.rs::process_one", 569],
  ["crates/ffone-client/src/tutorial_logic.rs::evaluate_mission", 334],
  ["crates/ffone-client/src/tutorial_logic.rs::evaluate_infection", 340],
  ["crates/ffone-client/src/tutorial_mission_content.rs::compact_document", 374],
  ["crates/ffone-client/src/tutorial_mission_content.rs::real_assets_game_table_set_has_exact_tutorial_provenance_when_available", 490],
  ["crates/ffone-client/src/mission_ui.rs::spawn_journal", 724],
  ["crates/ffone-client/src/mission_ui.rs::bind_mission_ui", 1_191],
  ["crates/ffone-client/src/tutorial_player_rig_runtime.rs::prepare_tutorial_player_animation_adapter", 380],
  ["crates/ffone-client/src/tutorial_player_rig_runtime.rs::finalize_tutorial_player_rig_readiness", 314],
  ["crates/ffone-client/src/tutorial_player_rig_runtime.rs::consume_tutorial_player_presentation", 334],
  ["crates/ffone-client/src/user_equip_ui.rs::spawn_user_equip_ui", 511],
  ["crates/ffone-client/src/user_equip_ui.rs::spawn_item_popup", 326],
  ["crates/ffone-client/src/user_equip_ui.rs::bind_user_equip_ui", 1_607],
  ["crates/ffone-client/src/user_store_ui.rs::bind_user_store_ui_0104", 413],
  ["crates/ffone-client/src/vendor_ui.rs::bind_vendor_ui", 353],
  ["crates/ffone-client/src/world/collision.rs::resolve_authored_world_ground", 383],
  ["crates/ffone-client/src/world_behaviour/streaming.rs::materialize_world_behaviour_batch", 540],
  ["crates/ffone-client/src/world_map.rs::spawn_world_map_presentation", 333],
]);

function repositoryPath(path) {
  return relative(REPOSITORY_ROOT, path).replaceAll("\\", "/");
}

function rustFiles(root) {
  const files = [];
  for (const entry of readdirSync(root)) {
    const path = join(root, entry);
    if (statSync(path).isDirectory()) {
      files.push(...rustFiles(path));
    } else if (entry.endsWith(".rs")) {
      files.push(path);
    }
  }
  return files;
}

function lineNumberAt(source, offset) {
  let lines = 1;
  for (let index = 0; index < offset; index += 1) {
    if (source.charCodeAt(index) === 10) lines += 1;
  }
  return lines;
}

// This deliberately conservative scanner is sufficient for enforcing a size budget. It locates
// Rust function bodies and balances braces while ignoring comments and string/character literals.
function functionBodies(source) {
  const bodies = [];
  const declaration = /\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\b/g;
  let match;
  while ((match = declaration.exec(source)) !== null) {
    let cursor = declaration.lastIndex;
    let bodyStart = -1;
    let state = "code";
    let blockDepth = 0;
    for (; cursor < source.length; cursor += 1) {
      const char = source[cursor];
      const next = source[cursor + 1];
      if (state === "line-comment") {
        if (char === "\n") state = "code";
      } else if (state === "block-comment") {
        if (char === "/" && next === "*") {
          blockDepth += 1;
          cursor += 1;
        } else if (char === "*" && next === "/") {
          blockDepth -= 1;
          cursor += 1;
          if (blockDepth === 0) state = "code";
        }
      } else if (state === "string") {
        if (char === "\\") cursor += 1;
        else if (char === '"') state = "code";
      } else if (state === "character") {
        if (char === "\\") cursor += 1;
        else if (char === "'") state = "code";
      } else if (char === "/" && next === "/") {
        state = "line-comment";
        cursor += 1;
      } else if (char === "/" && next === "*") {
        state = "block-comment";
        blockDepth = 1;
        cursor += 1;
      } else if (char === '"') {
        state = "string";
      } else if (char === "'" && source[cursor + 2] === "'") {
        state = "character";
      } else if (char === ";") {
        break;
      } else if (char === "{") {
        bodyStart = cursor;
        break;
      }
    }
    if (bodyStart < 0) continue;

    let depth = 1;
    state = "code";
    blockDepth = 0;
    cursor = bodyStart + 1;
    for (; cursor < source.length && depth > 0; cursor += 1) {
      const char = source[cursor];
      const next = source[cursor + 1];
      if (state === "line-comment") {
        if (char === "\n") state = "code";
      } else if (state === "block-comment") {
        if (char === "/" && next === "*") {
          blockDepth += 1;
          cursor += 1;
        } else if (char === "*" && next === "/") {
          blockDepth -= 1;
          cursor += 1;
          if (blockDepth === 0) state = "code";
        }
      } else if (state === "string") {
        if (char === "\\") cursor += 1;
        else if (char === '"') state = "code";
      } else if (state === "character") {
        if (char === "\\") cursor += 1;
        else if (char === "'") state = "code";
      } else if (char === "/" && next === "/") {
        state = "line-comment";
        cursor += 1;
      } else if (char === "/" && next === "*") {
        state = "block-comment";
        blockDepth = 1;
        cursor += 1;
      } else if (char === '"') {
        state = "string";
      } else if (char === "'" && source[cursor + 2] === "'") {
        state = "character";
      } else if (char === "{") {
        depth += 1;
      } else if (char === "}") {
        depth -= 1;
      }
    }
    if (depth === 0) {
      const startLine = lineNumberAt(source, match.index);
      const endLine = lineNumberAt(source, cursor);
      bodies.push({ name: match[1], startLine, lines: endLine - startLine + 1 });
      declaration.lastIndex = cursor;
    }
  }
  return bodies;
}

const failures = [];
for (const forbiddenRoot of ["content", "reference", "work"]) {
  const path = join(REPOSITORY_ROOT, forbiddenRoot);
  if (existsSync(path)) {
    failures.push(
      `${forbiddenRoot}/: forbidden repository root; legacy and reverse-engineering work belongs in ../FusionForge`,
    );
  }
}

for (const path of rustFiles(CLIENT_SOURCE_ROOT)) {
if (process.argv.includes("--boundary-only")) {
  if (failures.length > 0) {
    console.error("Client repository boundary checks failed:\n");
    for (const failure of failures) console.error(`- ${failure}`);
    process.exit(1);
  }

  console.log("Client repository boundary checks passed.");
  process.exit(0);
}

  const source = readFileSync(path, "utf8");
  const relativePath = repositoryPath(path);
  const lines = source.length === 0 ? 0 : source.split("\n").length - Number(source.endsWith("\n"));
  const fileLimit = FILE_DEBT_LIMITS.get(relativePath) ?? DEFAULT_FILE_LIMIT;
  if (lines > fileLimit) {
    failures.push(`${relativePath}: ${lines} lines exceeds file limit ${fileLimit}`);
  }

  const forbiddenImport = /^\s*use\s+super::\*\s*;/gm;
  let importMatch;
  while ((importMatch = forbiddenImport.exec(source)) !== null) {
    failures.push(`${relativePath}:${lineNumberAt(source, importMatch.index)}: forbidden use super::*`);
  }

  for (const body of functionBodies(source)) {
    const key = `${relativePath}::${body.name}`;
    const functionLimit = FUNCTION_DEBT_LIMITS.get(key) ?? DEFAULT_FUNCTION_LIMIT;
    if (body.lines > functionLimit) {
      failures.push(
        `${relativePath}:${body.startLine}: fn ${body.name} is ${body.lines} lines; limit is ${functionLimit} (${key})`,
      );
    }
  }
}

if (failures.length > 0) {
  console.error("Client architecture checks failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Static client architecture checks passed.");
if (!process.argv.includes("--static-only")) {
  console.log("Running mandatory ffone-client binary test target...");
  const result = spawnSync(
    "cargo",
    ["test", "-p", "ffone-client", "--bin", "ffone-client"],
    { cwd: REPOSITORY_ROOT, env: process.env, stdio: "inherit", shell: process.platform === "win32" },
  );
  if (result.error) throw result.error;
  process.exit(result.status ?? 1);
}
