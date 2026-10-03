# Repeated Nano acquisition after login

The native client sent Ice King 59 tuning IDs 261/262/263, expecting skills
246/247/248. The installed OpenFusion `bin/tdata/xdt.json` had reverted to the
pre-repair tuning identities. Its row 261 still carried wire ID 220; wire ID
261 referred to a different row and skill. The server rejected the native tune
request. The Nano remained owned with skill zero, so the client's existing login
scan correctly resumed acquisition on every subsequent login.

This reproduces the reported symptom locally; the external tester's server and
binary versions were not available. It is a server/client table compatibility
failure, not evidence that the client reissues `/nano` during login.

## Reproduction and repair

Use the native client acceptance executable `nano_acquisition_client_smoke`
against an isolated database. Its shared wire driver now waits for exit success,
logs back into the same character, checks the saved skill in the server Nano
book, and reseeds the production client bank before checking acquisition state.
For Nano 59, the installed server failed on the first ability with an explicit
NANO_TUNE_FAIL. The same executable and server binary pass with the repaired XDT.

The new `--server-only` mode in `tools/native/repair-nano-tuning.py` replays the
accepted native tuning contract without extracting legacy sources or rewriting
current client tables/localization. It requires matching Nano identities and
existing skill payloads, permits only the previously accepted tuning-number
normalization and Cheese skill repair, stages before/after files, and installs
only the explicitly named server XDT.

Run from FusionForge:

```powershell
python tools/native/repair-nano-tuning.py --target-root ../FFOneClient --server-xdt ../OpenFusion/bin/tdata/xdt.json --server-only --stage work/cases/nano-relogin-20260913/stage
python tools/native/repair-nano-tuning.py --target-root ../FFOneClient --server-xdt ../OpenFusion/bin/tdata/xdt.json --server-only --stage work/cases/nano-relogin-20260913/applied --apply
```

Restart the affected server to load the repaired table. Distribute the repaired
XDT with the matching native client; a client executable update alone does not
repair a remote server. Existing skill-zero Nanos need one successful ability
selection after the server update. No player database rewrite is required for
this failure mode.

## Verification

- Original server SHA-256: `0f83b5fb6e8a06677eb748a2a36d273fe764cc263f0ccf4311e1e4a4a761ab81`.
- Repaired SHA-256: `7fe6fb52824f810f453e5051be8b1813c09290a5313b1a8f3cdeab3c924d54dd`.
- The repaired bytes exactly match the accepted
  `recipes/native/tables/nano-tuning-identities-20260908-server.json` output.
- A second staging run is byte-identical (idempotent).
- All 61 available native Nanos passed all three abilities and relogin:
  183 successful acquisition/tuning/equip/persistence scenarios (IDs 1–36,
  41–46, 48–66). IDs 37–40 and 47 are not available native Nanos; the initial
  numeric-range runner stopped at 37 and then resumed using the actual IDs.
- `python tools/native/test_repair_nano_tuning.py`: repair/idempotency and
  rejection of unrelated Nano/skill edits pass.
- Client bank unit tests: five pass; the separate production localization test
  fails on an existing Russian alias mismatch at `content.nano_tune.4.name`.
  This repair does not modify translations.

Isolated server logs and database are in
`../OpenFusion/work/nano-relogin-20260913`; staged table hashes and backups are
in `work/cases/nano-relogin-20260913`. The running normal server was not restarted
by this test; the isolated server loads the corrected table.
