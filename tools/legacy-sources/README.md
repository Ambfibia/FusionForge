# Source lookup

`sources.json` is the portable registry; paths resolve from FusionForge. Existing
`inventories/*.tsv` locate filenames/sizes without scanning builds. Refresh only changed
sources. Roles/accepted exceptions: [source contract](../../docs/source-contract.md).

```powershell
./tools/legacy-sources/find.cmd -Source retrobution -Query "FutureNano"
cargo fusionforge inspect <container> --asset <name> --path-id <id> --limit 20 --format markdown
```

Inspection writes stdout. Use [investigation](../../docs/investigation.md) only for
ownership/managed details. `find-objects` and staged domain helpers are historical tools,
not required caches or a second direct pipeline. Their unconverted paths must not be
used to reintroduce intermediate files; [coverage](../../docs/conversion-workflow.md) is explicit.
