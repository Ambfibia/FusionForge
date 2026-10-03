# Repository layout

The root is a virtual Cargo workspace. crates/fusionforge is the default member;
all owned Rust implementation is under crates/*/src. Shared integration tests stay
under tests and are registered explicitly by the CLI manifest. Cargo output is
target/, never conversion scratch. Toolchain and dependency versions remain pinned.

vendor/ffbuildtool and vendor/FFSpy are third-party submodules. Preserve their local
changes, provenance and licenses; do not rename upstream packages for tool branding.
assets/unity-types contains parser resources. recipes contains executable rules and
accepted exceptions. tools contains maintenance helpers, not another conversion API.

The supported invocation is cargo fusionforge <command> or fusionforge.cmd <command>.
Additional Rust utilities are library-backed subcommands, not independent binaries.

Historical evidence is reference material, not mandatory reading or converter input.
Delete obsolete artifacts only after preserving unique contracts and regression data.
See README for the directory map. No conversion output belongs under target/.
