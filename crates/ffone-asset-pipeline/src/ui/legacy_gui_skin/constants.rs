
pub const LEGACY_GUI_SKIN_SCHEMA: &str = "fusionforge.legacy-unity-gui-skin-candidate.v1";

pub const LEGACY_GUI_SKIN_EVIDENCE_LEVEL: &str = "candidate";

pub(super) const LEGACY_GUI_SKIN_LIMITATIONS: &[&str] = &[
    "generic dump-object-all JSON does not prove raw-container bytes, hashes, or container identity",
    "positive fileId pointers cannot be resolved because the owning serialized asset external-file table is absent",
    "serialized asset names are not stable asset indices; dumps from multiple containers must not be merged",
    "missing or invalid serialized fields use compatibility placeholders and remain unresolved diagnostics",
];

pub(super) const TARGET_GUI_SKINS: &[&str] = &[
    "FusionFallOptionSkin",
    "FusionFallInvenSkin",
    "FusionFallChatSkin",
    "FusionFallMissionSkinR",
    "FusionFallHUDSkin",
    "FusionFallMapSkin",
    "FusionFallSkin",
    "FusionFallNanoSkin",
    "FusionFallSysMessageSkin",
    "FusionFallTransportSkin",
    "FusionFallHelpSkin",
    "FusionFallMissionSkin",
    "FusionFallInteractionSkin",
    "FusionFallGuideSkin",
];

pub(super) const BUILTIN_STYLES: &[(&str, &str)] = &[
    ("box", "m_box"),
    ("button", "m_button"),
    ("label", "m_label"),
    ("textArea", "m_textArea"),
    ("textField", "m_textField"),
    ("toggle", "m_toggle"),
    ("window", "m_window"),
    ("horizontalSlider", "m_horizontalSlider"),
    ("horizontalSliderThumb", "m_horizontalSliderThumb"),
    ("verticalSlider", "m_verticalSlider"),
    ("verticalSliderThumb", "m_verticalSliderThumb"),
    ("horizontalScrollbar", "m_horizontalScrollbar"),
    ("horizontalScrollbarThumb", "m_horizontalScrollbarThumb"),
    (
        "horizontalScrollbarLeftButton",
        "m_horizontalScrollbarLeftButton",
    ),
    (
        "horizontalScrollbarRightButton",
        "m_horizontalScrollbarRightButton",
    ),
    ("verticalScrollbar", "m_verticalScrollbar"),
    ("verticalScrollbarThumb", "m_verticalScrollbarThumb"),
    ("verticalScrollbarUpButton", "m_verticalScrollbarUpButton"),
    (
        "verticalScrollbarDownButton",
        "m_verticalScrollbarDownButton",
    ),
    ("scrollView", "m_scrollView"),
];

pub(super) const STYLE_STATES: &[(&str, &str)] = &[
    ("normal", "m_Normal"),
    ("hover", "m_Hover"),
    ("active", "m_Active"),
    ("focused", "m_Focused"),
    ("onNormal", "m_OnNormal"),
    ("onHover", "m_OnHover"),
    ("onActive", "m_OnActive"),
    ("onFocused", "m_OnFocused"),
];
