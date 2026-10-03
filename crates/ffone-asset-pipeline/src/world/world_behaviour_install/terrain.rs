use super::*;

/// Unity 3.x serializes its built-in `Terrain` component through a type tree
/// that some bundles report as `MonoBehaviour` with a null `m_Script`.
/// Identify that exact built-in payload before script resolution. The native
/// terrain exporter already owns all of these fields, so treating it as an
/// unresolved user script creates one false blocker per tile.
pub(super) fn is_builtin_terrain_component(record: &ExportBehaviour) -> bool {
    record.script.is_none()
        && record.fields.contains_key("m_TerrainData")
        && record.fields.contains_key("m_HeightmapPixelError")
        && record.fields.contains_key("m_HeightmapMaximumLOD")
        && record.fields.contains_key("m_DetailObjectDistance")
        && record.fields.contains_key("m_TreeDistance")
}
