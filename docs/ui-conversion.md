# Unity UI adapter contract

For actual Unity UI migration or evidence-dependent fixes, not new native screens.
The source-pinned QuitMenu adapter and unsupported coverage are listed in
[conversion-workflow](conversion-workflow.md). Do not infer support for other screens.

FusionForge must perform resolution, measurements and transformations; agents must not
assemble intermediate JSON, calculate coordinates manually or patch exports by eye.
Retained analysis templates/commands serve explicitly requested research only. Until a
direct adapter is implemented and tested, report the gap instead of reviving staged steps.

## Ownership, images and interaction

Resolve the owning GameObject/MonoBehaviour/MonoScript, exact GUIStyle/Font/Texture fields,
visibility/enabled gates and source revision. Follow scoped references, not names:
OptionMode `SkyTex` resolves to `equipbar` pathId 82, not `setting_back_tmp`.

Scale the reference root once. Preserve Rect/GUI.matrix, area/group nesting, scroll viewport
and independently translated content, depth/call order, hit order, Event.Use, hotControl,
focus, state resets and modal rejection. Presentation emits typed drafts/actions; renderer,
mixer, protocol and persistence own effects/commits.

Direct `GUI.DrawTexture` uses its exact overload/ScaleMode. A zero-border style stretches;
a nonzero border uses exact left/right/top/bottom nine-slice. Keep overflow separate from
layout, disabled input/appearance, state textures/colors, multiplicative tint and restoration.
GUIStyle backgrounds cover the full control Rect, independently of `m_Padding`.
For Bevy ImageNode use `VisualBox::BorderBox`, not its default ContentBox; padding
only constrains the text. Preserve this on every state-image rebind and retain the
same hit rectangle. The QuitMenu converter emits `backgroundBox: "border-box"`;
older documents default to that policy and unsupported box policies are rejected.
Negative GUIStyle padding may need measured translation, not invalid Taffy padding. Never
shrink corners silently or combine primary styles with donor textures. `geometryOnly` image
contracts retain `textureSemanticId`; bind its native pixels/sampler/orientation/color/alpha.

## Replacement-font measurement

`control Rect → padding → contentOffset → TextAnchor/layout box → baseline/ink → font compensation`.
Keep approved Cyrillic-capable replacements; a font name/size is not portable metrics.
Bind legacy container/object hashes and Font ID, replacement semantic ID/bytes hash, and
metric capture producer/version/hash outside runtime. Record canvas/root scale, Rect,
padding, offset, anchor, wrapping/clipping, size/line spacing, baseline/ascent/descent.

Measure matching EN labels, longest RU labels, relevant digits/punctuation and wrap-edge
strings: advance/layout size, first baseline, ink bounds, line count and Unicode-scalar
breaks. Units are `referenceCanvasPixelsAfterGlyphRenderScale`: apply glyph renderScale
once before measurement. Bounds/line height/offset/wrap width are already post-scale;
never scale them again. A JEFFE `1.0 x 0.70` glyph scale is not a second layout transform.
The evidence adapter does not invoke a platform-dependent OS rasterizer.

Retain `fusionforge.unity-text-replacement-adapter.v2` evidence and only its nested
`ffone.native-text-replacement-contract.v1` as native semantics. Unity Overflow requires
`preserveSourceClipRect=false`. Unsatisfied output fails; `--allow-unsatisfied` is triage,
not publication consent. Static interaction candidates with `publicationAllowed=false`
do not prove field gates, dispatch, GUILayout geometry or cross-component draw order.

## Focused acceptance

Check the changed reachable states: hidden/modal, enabled/disabled, hover/press/release
inside/outside, focus/toggle, overlaps/topmost hit, reference/required alternate viewport,
EN/RU wrapping. Pair ECS action tests with real Bevy GPU crops and runtime interaction;
inspect seams, corners, baseline/ink/color and clipping. Systematic residuals require an
adapter fix, not global magic offsets. Preserve source quirks and accepted native extensions.
Do not rerun every screen/backend or a full asset audit for an unrelated label change.
