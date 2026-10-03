# Academy Nano voice coverage, 2026-09-14

The follow-up audit covers all 67 current gameplay identities (66 distinct native
models), including the Academy additions. It found another incomplete import:
native Johnny Bravo 38 had `call` and `skill1`, but no `skill2` or `skill3`.
The existing voice table already declared the B/C power recordings.

The exact `OG-academy` donor has both missing clips. The new Editor pipeline
command `append_model_animations` appends their curves and events to the existing
native GLB. It maps full hierarchy paths, checks every bone rest transform, and
copies only animation buffer views. Original nodes, meshes, materials, textures,
skins, animations and binary prefix remain identical. The explicit
`--retain-static-root-scale` option preserves the accepted native size override
only when selected donor clips do not animate the root. All other rest-pose
differences fail closed, as do attempts to replace an existing clip.

`skill2` requests `JBravo_NanPwrB0(RAND:1-3).wav` at 0.25 seconds; `skill3`
requests `JBravo_NanPwrC0(RAND:1-2).wav` at 0.25 seconds. Native identity 38,
skill assignments and the previous call/skill1 behavior are unchanged. This is
an explicit alternate-source extension, not primary parity.

Validation:

- Two independent raw-source exports and publications produce identical final
  GLB bytes: `449f2c4877f0516cee70824ed6c05cb09d5ae301a4fe0b171b46a98d9d51390a`.
- The 60-test production Nano/audio suite passes. Its new full-roster check
  requires call and all three power event sets except explicitly incomplete
  source models, then sends real GLB voice events through the runtime in EN/RU
  and checks localized playback creation and the resolved file.
- All 762 distinct resolved call/power voice files decode and contain nonzero
  samples. This proves file integrity, not the spoken meaning of each recording.
- Actual GPU preview of native Nano 38 powers 2 and 3 loads and animates the
  model; both captures were visually inspected. The preview does not play audio.
- Three Editor command tests cover native scale preservation, rejection of
  animated-root/bone pose differences, and refusal to replace existing clips.

The newer render validator reports four triangles opposing authored normals in
the pre-existing Johnny Bravo model. The output retains exactly the same finding
and byte-identical mesh data. This animation repair does not change its geometry.

Remaining source limitations are explicit: Titan power 3 refers to
`Titan_NanPwrBRockets`, which has no recorded clips in the exact `OG-academy`
Nano_048 bundle; Ghostfreak and Upgrade have no declared voice recordings;
Ben and Cheese retain only one authored power animation; Holonano has unresolved
placeholder power cues. These are not counted as successful voiced cases or
filled with another character's voice. The earlier Flapjack, Chowder, Zak and
Jack O'Lantern repairs remain covered by the suite.

See the [full model/event audit](../../../docs/reference/evidence/cases/academy-nano-voice-20260914.json)
and [publication receipt](../../../recipes/native/audio/academy-johnny-bravo-power-voices-20260914.receipt.json).
Detailed decode results and original/extracted inputs stay below
`work/cases/academy-voice-20260914`; native test logs and GPU captures are under
the client's ignored `target/performance/academy-voice-20260914` directory.
