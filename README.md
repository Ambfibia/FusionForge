# FusionForge

Rust tools for reading legacy FusionFall Unity assets/assemblies and converting supported
content directly into FFOneClient's editable files. Decompiled code is recovered evidence,
not original source or automatically translated game behavior.

```powershell
cargo fusionforge --help
cargo fusionforge inspect <container> --type GameObject --limit 20
cargo fusionforge convert-native-model <bundle> <exact-route> <family> <native-output-root>
```

`crates/` contains owned Rust code (default CLI: `crates/fusionforge`); `assets/unity-types/`
is parser metadata; `recipes/` holds accepted conversion rules; `tools/` holds maintenance
and not-yet-integrated helpers; `tests/` holds integration fixtures; `vendor/` retains upstream
sources/licenses; `target/` is Cargo output. No conversion scratch directory is required.

[Conversion and limits](docs/conversion-workflow.md) · [Task map](docs/navigation.md).
CLI flags: `.cargo/config.toml` and command-specific `--help`.

[Source ownership and focused checks](docs/source-map.md).
