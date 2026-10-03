---
name: reverse-engineer-unity-client
description: Use only for legacy Unity research/import or bugs requiring old-client evidence; not native features, ordinary bugs or CLI refactoring.
---

# Unity research

Work from FusionForge. Bound the question to one source, owner and native consumer.
Use `inspect` with an object/type/class/method filter and `--limit`; expand only if
that query cannot answer it. Never decompile/index the whole build by default.

Select only the needed reference (paths from the FusionForge root):
- Source/donor choice: `docs/source-contract.md`.
- Ownership, evidence or managed behavior: `docs/investigation.md`.
- Conversion: `docs/conversion-workflow.md`; UI adapter details: `docs/ui-conversion.md`.

No recursive reading, manual conversion calculations or staged JSON assembly.
Unsupported behavior stays explicit. Preserve source provenance outside runtime files;
validate the affected native consumer. JSON/Markdown reports are optional final output,
not a prerequisite or a cache. Do not claim static evidence proves runtime behavior.
