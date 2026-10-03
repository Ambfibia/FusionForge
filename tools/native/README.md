# Native checks

Optional checks run from FusionForge, targeting sibling FFOneClient:

```powershell
node tools/native/check-client-architecture.mjs --boundary-only
node --test tools/native/test-localization-terminology.mjs
powershell -File tools/native/test-character-creation.ps1
```

Architecture/terminology checks accept `FFONE_CLIENT_ROOT`; the PowerShell runner accepts
`-ClientRoot`. `-Gpu` / `-Server` opt into production-rendering / historical OpenFusion checks;
they are not default gates for every task. Native GPU fixtures may write requested captures
to client `target/performance`, not conversion scratch.

`audit-render-duplicates.mjs <asset-root> [report.json]` writes a report; its legacy default
is `work/reports/render-duplicates.json`. Do not invoke that default. Run only for an explicit
audit with a requested final report path. `generate_character_runtime_gpu_audit.mjs` uses
historical routes and is not a current visual acceptance gate.

Conversion/repair support and remaining staged paths: [native-cli](../../docs/native-cli.md).
These validators do not establish parity or replace the converter. Do not generate baseline
reports or galleries unless the task requires them.
