use std::{collections::BTreeSet, fs, path::Path};

use super::*;

#[test]
fn table_data_icon_routes_have_exact_proven_closure() {
    let nano = VERIFIED_ICON_ROUTES
        .iter()
        .filter(|route| route.kind == TableDataIconKind::Nano)
        .map(|route| route.icon_number)
        .collect::<Vec<_>>();
    let skill = VERIFIED_ICON_ROUTES
        .iter()
        .filter(|route| route.kind == TableDataIconKind::Skill)
        .map(|route| route.icon_number)
        .collect::<Vec<_>>();
    assert_eq!(nano, NANO_ICON_NUMBERS);
    assert_eq!(skill, SKILL_ICON_NUMBERS);
    assert_eq!(nano.len(), 42);
    assert_eq!(skill.len(), 37);
}

#[test]
fn table_data_icon_routes_are_unique_and_semantic() {
    let mut names = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    for route in VERIFIED_ICON_ROUTES {
        assert!(names.insert(route.legacy_name));
        assert!(sources.insert(route.source));
        assert!(destinations.insert(route.destination));
        let digits = format!("{:02}", route.icon_number);
        let stem = match route.kind {
            TableDataIconKind::Nano => "nanoicon_",
            TableDataIconKind::Skill => "skillicon_",
        };
        assert_eq!(route.legacy_name, format!("{stem}{digits}"));
        assert_eq!(
            route.source,
            format!(
                "icons/{}/{stem}{digits}.png",
                match route.kind {
                    TableDataIconKind::Nano => "entities/nanos",
                    TableDataIconKind::Skill => "skills",
                }
            )
        );
        assert_eq!(
            route.destination,
            format!(
                "nano/icons/{}/{stem}{digits}.png",
                match route.kind {
                    TableDataIconKind::Nano => "nano",
                    TableDataIconKind::Skill => "skill",
                }
            )
        );
        assert_eq!(route.expected_blake3.len(), 64);
    }
    assert_eq!(names.len(), 79);
    assert_eq!(sources.len(), 79);
    assert_eq!(destinations.len(), 79);
}

#[test]
fn checked_in_icon_bytes_match_the_exact_route_hashes() {
    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("game");
    for route in VERIFIED_ICON_ROUTES {
        let path = asset_root.join(route.source.replace('/', std::path::MAIN_SEPARATOR_STR));
        let bytes = fs::read(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert_eq!(
            blake3::hash(&bytes).to_hex().as_str(),
            route.expected_blake3,
            "{}",
            path.display()
        );
    }
}
