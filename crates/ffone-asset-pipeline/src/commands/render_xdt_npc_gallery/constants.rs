
pub(super) const SCHEMA: &str = "ffone.xdt-npc-render-gallery.v1";

pub(super) const HELP: &str = r#"FFOne XDT NPC/HNPC render gallery

Usage:
  render_xdt_npc_gallery --xdt <PATCHED_XDT_JSON> --asset-root <ASSETS_GAME> \
    --output <FRESH_OR_RESUMED_DIR> [OPTIONS]

Options:
  --preview <EXE>         logical_model_gpu_preview executable; omit for an audit-only gallery
  --legacy-plan <JSON>   clean-primary logical-model export plan used to classify missing routes
  --blocker-report <JSON> later source/batch report whose blockers override plan-ready routes; may be repeated
  --texture-metadata <JSON> exact primary Texture2D sampler metadata; may be repeated
  --texture-catalog-output <JSON> write the resolved runtime NPC texture contract
  --frames <COUNT>       per-model GPU frame budget (default: 900)
  --timeout <SECONDS>    per-model GPU timeout (default: 45)
  --max-models <COUNT>   render only the first COUNT unique runtime appearances (smoke testing)
  --resume               reuse completed model renders and existing entity image links
  -h, --help             show this help
"#;
