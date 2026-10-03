# FusionForge

Offline Unity inspection/conversion to FFOneClient's native assets. One CLI:
`fusionforge`; owned Rust: `crates/*/src`. Preserve vendor licenses and local edits.

Read immutable containers in memory/streams; write only requested final output.
No `work/`, staging, extraction cache, intermediate JSON/projects or required receipts.
Preflight dependencies, ambiguity, unsupported behavior and conflicts. Preserve user
edits, accepted IDs/donors/remaps, EN/RU and fonts. Native files have no source locators;
`target/` is Cargo output, not conversion scratch.

Search one owner and read matching ranges. `docs/source-map.md` locates code.
`.ignore` excludes generated/fixture/history data; use `rg --no-ignore` with an exact
path when needed. No recursive reference reading, repeated scans, whole-build indexing,
unrequested reports or global suites for local tasks. Widen checks for shared APIs.
Keep cohesive files normally below 1,000 lines; preserve unique regression tests.

| Need | Read |
| --- | --- |
| Convert | `docs/conversion-workflow.md` |
| Source/identity | `docs/source-contract.md` |
| Research Unity | `.agents/skills/reverse-engineer-unity-client/SKILL.md` |

The skill is not needed for native code/CLI refactoring. Conversion acceptance includes
no-write/conflict checks and the affected Bevy loader/interaction. Compilation alone is
insufficient; historical procedures are not current commands.
