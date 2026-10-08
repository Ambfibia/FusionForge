//! Add Academy equipment to the current native catalogs without replacing accepted assets.
//! Recovery, dependency resolution and publication are one in-memory operation.
use super::native_publication as p;
use ffone_asset_pipeline::{
    AvatarItemCategory as Category, AvatarModelReference,
    CharacterCreationAssetReference as Reference,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

mod icons;
mod resources;
mod retention;
mod sharing;
use resources::Resources;

const CATALOG: &str = "characters/player/items/catalog.json";
const AVATAR: &str = "data/character_creation/avatar_items.json";
const RUNTIME: &str = "data/character_creation/runtime_textures.json";
const CATEGORIES: [(Category, &str); 8] = [
    (Category::Back, "back"),
    (Category::Glasses, "glass"),
    (Category::Hat, "hat"),
    (Category::Shirt, "shirts"),
    (Category::Pants, "pants"),
    (Category::Shoes, "shoes"),
    (Category::Vehicle, "vehicle"),
    (Category::Weapon, "weapon"),
];

pub(super) fn reference(route: &str, bytes: &[u8]) -> Value {
    json!({"path":route,"bytes":bytes.len(),"blake3":blake3::hash(bytes).to_hex().to_string()})
}

pub(super) fn put(
    files: &mut BTreeMap<String, Vec<u8>>,
    route: String,
    bytes: Vec<u8>,
) -> Result<(), String> {
    if let Some(old) = files.get(&route) {
        if old != &bytes {
            return Err(format!("conflicting planned resource {route}"));
        }
    } else {
        files.insert(route, bytes);
    }
    Ok(())
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("missing {key}"))
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    if !(args.len() == 2 || args.len() == 3 && args[2] == "--check") {
        return Err(
            "convert-academy-items <Academy-raw-root> <native-asset-root> [--check]".into(),
        );
    }
    let source = Path::new(&args[0])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let root = Path::new(&args[1])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if root.starts_with(&source) {
        return Err("native output is inside source".into());
    }
    let table_path = source.join("TableData.resourceFile");
    let retention: Value = serde_json::from_str(include_str!(
        "../../../../recipes/native/characters/academy-equipment.json"
    ))
    .map_err(|e| e.to_string())?;
    if p::digest(&fs::read(&table_path).map_err(|e| e.to_string())?)
        != retention["tableSha256"]
            .as_str()
            .ok_or("Academy table pin")?
    {
        return Err("expected the accepted OG Academy TableData donor".into());
    }
    for (name, hash) in retention["sourceBundles"]
        .as_object()
        .ok_or("Academy bundle pins")?
    {
        if p::digest(&fs::read(source.join(name)).map_err(|e| e.to_string())?)
            != hash.as_str().ok_or("bundle digest")?
        {
            return Err(format!(
                "Academy equipment source differs from accepted donor: {name}"
            ));
        }
    }
    let assets = super::direct_input::read_assets(&table_path)?;
    let env = super::unity::UnityEnvironment::from_assets(assets);
    let donor = super::cli::unity_to_json(&env.read_object(super::unity::ObjectKey {
        asset: 0,
        path_id: 2139558964,
    })?);
    let xdt = p::read_json(&root.join("data/tables/xdt.json"))?;
    let mut catalog = p::read_json(&root.join(CATALOG))?;
    let mut avatar = p::read_json(&root.join(AVATAR))?;
    let mut runtime = p::read_json(&root.join(RUNTIME))?;
    let mut preimages = BTreeMap::new();
    for route in [
        CATALOG,
        AVATAR,
        RUNTIME,
        "data/character_creation/name_wheel.json",
        "data/character_creation/appearance.json",
    ] {
        preimages.insert(
            route.to_owned(),
            fs::read(root.join(route)).map_err(|e| e.to_string())?,
        );
    }
    if catalog["schema"] != "ffone.player-item-set-catalog.v1"
        || avatar["schema"] != "ffone.character-creation.avatar-items.v1"
    {
        return Err("unsupported native equipment catalog".into());
    }
    let mut files = BTreeMap::new();
    let mut textures = BTreeMap::<String, Vec<Reference>>::new();
    for row in array(&runtime, "textures")? {
        add_texture(
            &mut textures,
            row["trueName"].as_str().ok_or("texture name")?,
            &row["nativeAsset"],
        )?;
    }
    for item in array(&avatar, "items")? {
        if let Some(icon) = item["icon"].as_object() {
            for candidate in icon["candidates"].as_array().into_iter().flatten() {
                add_texture(
                    &mut textures,
                    icon["trueName"].as_str().ok_or("icon name")?,
                    candidate,
                )?;
            }
        }
    }
    let mut models = model_index(&catalog)?;
    let existing = array(&avatar, "items")?
        .iter()
        .map(|i| {
            (
                i["category"].as_str().unwrap_or("").to_owned(),
                i["itemNumber"].as_u64().unwrap_or(0),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut additions = Vec::new();
    let mut blocked = Vec::new();
    let mut unsupported = BTreeMap::<String, String>::new();
    for (category, bundle_category) in CATEGORIES {
        let category_name = serde_json::to_value(category)
            .map_err(|e| e.to_string())?
            .as_str()
            .unwrap()
            .to_owned();
        let table = &xdt[category.table_name()];
        let donor_names = array(&donor[category.table_name()], "m_pItemStringData")?
            .iter()
            .filter_map(|v| v["m_strName"].as_str())
            .filter(|n| !n.trim().is_empty())
            .collect::<BTreeSet<_>>();
        let candidates = array(table, "m_pItemData")?
            .iter()
            .filter(|r| {
                let n = r["m_iItemNumber"].as_u64().unwrap_or(0);
                let name = table["m_pItemStringData"]
                    [r["m_iItemName"].as_u64().unwrap_or(0) as usize]["m_strName"]
                    .as_str()
                    .unwrap_or("");
                n > 0
                    && !existing.contains(&(category_name.clone(), n))
                    && donor_names.contains(name)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            continue;
        }
        let resources = Resources::open(&source, bundle_category)?;
        println!(
            "{}: checking {} missing native item rows",
            category_name,
            candidates.len()
        );
        let mut selected = BTreeSet::new();
        for row in candidates {
            let number = row["m_iItemNumber"].as_u64().ok_or("item number")?;
            let name = table["m_pItemStringData"]
                [row["m_iItemName"].as_u64().ok_or("string index")? as usize]["m_strName"]
                .as_str()
                .ok_or("item name")?;
            let mesh =
                &table["m_pItemMeshData"][row["m_iMesh"].as_u64().ok_or("mesh index")? as usize];
            let mut required_models = BTreeSet::new();
            let mut required_textures = BTreeSet::new();
            for gender in ["M", "F"] {
                if row["m_iReqSex"].as_u64() == Some(if gender == "M" { 2 } else { 1 }) {
                    continue;
                }
                for suffix in ["MeshModelString", "TextureString", "TextureString2"] {
                    let field = format!("m_pstr{gender}{suffix}");
                    if let Some(n) = mesh[&field]
                        .as_str()
                        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null"))
                    {
                        if suffix == "MeshModelString" {
                            required_models.insert(n.to_ascii_lowercase());
                        } else {
                            required_textures.insert(n.to_ascii_lowercase());
                        }
                    }
                }
            }
            let absent_models = required_models
                .iter()
                .filter(|n| {
                    !models.contains_key(&(category.equipment_category().to_owned(), (*n).clone()))
                        && !resources.models.contains_key(*n)
                })
                .cloned()
                .collect::<Vec<_>>();
            let absent_textures = required_textures
                .iter()
                .filter(|n| !textures.contains_key(*n) && !resources.textures.contains_key(*n))
                .cloned()
                .collect::<Vec<_>>();
            if !absent_models.is_empty() || !absent_textures.is_empty() {
                blocked.push(json!({"category":category_name,"id":number,"name":name,"models":absent_models,"textures":absent_textures}));
                continue;
            }
            let keys_before = files.keys().cloned().collect::<BTreeSet<_>>();
            let model_count = array(&catalog, "models")?.len();
            let set_count = array(&catalog, "sets")?.len();
            let mut conversion_error = None;
            for n in required_models {
                let key = (category.equipment_category().to_owned(), n.clone());
                if models.contains_key(&key) {
                    continue;
                }
                let result = if let Some(error) = unsupported.get(&n) {
                    Err(error.clone())
                } else {
                    resources.install_model(
                        &n,
                        category.equipment_category(),
                        &root,
                        &mut files,
                        &mut catalog,
                    )
                };
                if let Err(error) = result {
                    unsupported.insert(n.clone(), error.clone());
                    conversion_error = Some(error);
                    break;
                }
                models = model_index(&catalog)?;
            }
            if let Some(error) = conversion_error {
                files.retain(|route, _| keys_before.contains(route));
                catalog["models"]
                    .as_array_mut()
                    .unwrap()
                    .truncate(model_count);
                catalog["sets"].as_array_mut().unwrap().truncate(set_count);
                models = model_index(&catalog)?;
                blocked.push(
                    json!({"category":category_name,"id":number,"name":name,"unsupported":error}),
                );
                continue;
            }
            for n in required_textures {
                if textures.contains_key(&n) {
                    continue;
                }
                let owner = owner_set(&catalog, category.equipment_category(), mesh)?;
                let route = format!(
                    "{}/textures/{n}.png",
                    owner["definition"]["path"]
                        .as_str()
                        .ok_or("set definition")?
                        .trim_end_matches("/set.json")
                );
                let contract = resources.install_texture(&n, &route, &root, &mut files)?;
                add_texture(&mut textures, &n, &contract["nativeAsset"])?;
                runtime["textures"]
                    .as_array_mut()
                    .ok_or("runtime textures")?
                    .push(contract.clone());
                append_set_texture(
                    &root,
                    &mut files,
                    &mut preimages,
                    &mut catalog,
                    owner["id"].as_str().ok_or("set id")?,
                    contract["nativeAsset"].clone(),
                )?;
            }
            selected.insert(number);
            additions.push(json!({"category":category_name,"id":number,"name":name}));
        }
        // Use the same table resolver as the established native character-creation importer.
        let resolved = ffone_asset_pipeline::build_item_category(
            xdt.as_object().ok_or("native tables")?,
            category,
            &textures,
            &models,
        )
        .map_err(|e| e.to_string())?;
        for mut item in resolved {
            if selected.contains(&(item.item_number as u64)) {
                for visual in [&mut item.male, &mut item.female] {
                    if let Some(name) = &visual.source_model_true_name {
                        for row in array(&catalog, "models")?.iter().filter(|r| {
                            r["category"] == category.equipment_category()
                                && r["trueName"]
                                    .as_str()
                                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
                        }) {
                            let asset: Reference = serde_json::from_value(row["model"].clone())
                                .map_err(|e| e.to_string())?;
                            if !visual
                                .models
                                .iter()
                                .any(|m| m.native_asset.path == asset.path)
                            {
                                visual.models.push(AvatarModelReference {
                                    true_name: name.clone(),
                                    exact_route: format!("wear/{name}.nif"),
                                    native_asset: asset,
                                });
                            }
                        }
                        if visual.models.len() > 1 {
                            visual.model_status =
                                ffone_asset_pipeline::NativeLookupStatus::VerifiedVariants;
                        }
                    }
                }
                avatar["items"]
                    .as_array_mut()
                    .ok_or("avatar items")?
                    .push(serde_json::to_value(item).map_err(|e| e.to_string())?);
            }
        }
    }
    icons::complete(
        &source,
        &root,
        &donor,
        &xdt,
        &retention,
        &mut avatar,
        &mut files,
    )?;
    let retained_models = retention::complete(
        &root,
        &retention,
        &mut files,
        &mut preimages,
        &mut catalog,
        &mut runtime,
    )?;
    if additions.is_empty() && files.is_empty() {
        println!(
            "No recoverable Academy equipment remains; {} rows lack exact dependencies",
            blocked.len()
        );
        return Ok(());
    }
    sharing::normalize(
        &root,
        &mut files,
        &mut preimages,
        &mut catalog,
        &mut avatar,
        &mut runtime,
    )?;
    catalog["models"].as_array_mut().unwrap().sort_by(|a, b| {
        (
            a["category"].as_str(),
            a["trueName"].as_str(),
            a["sourceRoute"].as_str(),
            a["model"]["path"].as_str(),
        )
            .cmp(&(
                b["category"].as_str(),
                b["trueName"].as_str(),
                b["sourceRoute"].as_str(),
                b["model"]["path"].as_str(),
            ))
    });
    // Refresh only derived counts; keep all accepted item rows, variants and text intact.
    let items: Vec<ffone_asset_pipeline::AvatarItemLookup> =
        serde_json::from_value(avatar["items"].clone()).map_err(|e| e.to_string())?;
    let counts = ffone_asset_pipeline::avatar_item_counts(&items);
    avatar["lookupComplete"] = json!(
        counts.model_references == counts.resolved_models
            && counts.texture_references == counts.resolved_textures
            && counts.icon_references == counts.resolved_icons
    );
    avatar["counts"] = serde_json::to_value(counts).map_err(|e| e.to_string())?;
    let catalog_bytes = p::encode(&catalog)?;
    let proof = reference(CATALOG, &catalog_bytes);
    avatar["provenance"]["playerEquipmentCatalog"] = proof.clone();
    runtime["provenance"]["playerEquipmentCatalog"] = proof.clone();
    runtime["coverage"]["avatarTextureReferences"] = avatar["counts"]["textureReferences"].clone();
    let mut verified = BTreeSet::new();
    let published = array(&runtime, "textures")?
        .iter()
        .filter_map(|t| t["nativeAsset"]["path"].as_str())
        .collect::<BTreeSet<_>>();
    let mut deferred = BTreeSet::new();
    for item in &items {
        for visual in [&item.male, &item.female] {
            for texture in [&visual.primary_texture, &visual.secondary_texture]
                .into_iter()
                .flatten()
            {
                if texture.status == ffone_asset_pipeline::NativeLookupStatus::VerifiedUnique
                    && texture.candidates.len() == 1
                {
                    let route = &texture.candidates[0].path;
                    verified.insert(route.clone());
                    if !published.contains(route.as_str()) {
                        deferred.insert(route.clone());
                    }
                }
            }
        }
    }
    runtime["coverage"]["avatarVerifiedUniqueRoutes"] = json!(verified.len());
    runtime["coverage"]["avatarPublishedRoutes"] = json!(verified.len() - deferred.len());
    runtime["coverage"]["avatarDeferredVerifiedRoutes"] = json!(deferred.len());
    for route in [
        "data/character_creation/name_wheel.json",
        "data/character_creation/appearance.json",
    ] {
        let mut doc = p::read_json(&root.join(route))?;
        doc["provenance"]["playerEquipmentCatalog"] = proof.clone();
        put(&mut files, route.into(), p::encode(&doc)?)?;
    }
    put(&mut files, CATALOG.into(), catalog_bytes)?;
    put(&mut files, AVATAR.into(), p::encode(&avatar)?)?;
    put(&mut files, RUNTIME.into(), p::encode(&runtime)?)?;
    let mutable = files
        .keys()
        .filter(|r| r.ends_with(".json"))
        .cloned()
        .collect::<BTreeSet<_>>();
    // Existing descriptors are extended, but existing binary assets never receive replacement consent.
    for (route, bytes) in &files {
        if !mutable.contains(route)
            && !retained_models.contains(route)
            && root.join(route).exists()
            && fs::read(root.join(route)).map_err(|e| e.to_string())? != *bytes
        {
            return Err(format!("preserving conflicting accepted asset {route}"));
        }
    }
    let output = files
        .into_iter()
        .map(|(r, b)| (PathBuf::from(r), b))
        .collect::<Vec<_>>();
    for (route, bytes) in preimages {
        if fs::read(root.join(&route)).map_err(|e| e.to_string())? != bytes {
            return Err(format!("target changed during conversion: {route}"));
        }
    }
    ffone_asset_pipeline::direct_output::check_with_permission(&root, &output, true)?;
    if args.len() == 2 {
        ffone_asset_pipeline::direct_output::install_with_permission(&root, &output, true)?;
    }
    println!(
        "{} {} Academy items, {} final files; {} rows unavailable in Academy",
        if args.len() == 3 {
            "Preflighted"
        } else {
            "Installed"
        },
        additions.len(),
        output.len(),
        blocked.len()
    );
    for item in additions {
        println!(
            "{} {}: {}",
            item["category"].as_str().unwrap(),
            item["id"],
            item["name"].as_str().unwrap()
        );
    }
    for item in blocked {
        println!("Unavailable: {item}");
    }
    Ok(())
}

fn add_texture(
    index: &mut BTreeMap<String, Vec<Reference>>,
    name: &str,
    value: &Value,
) -> Result<(), String> {
    let value: Reference = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    let rows = index.entry(name.to_ascii_lowercase()).or_default();
    // Identical copies have one rendering identity; preserve different accepted artwork.
    if !rows
        .iter()
        .any(|r| r.blake3 == value.blake3 && r.bytes == value.bytes)
    {
        rows.push(value);
    }
    Ok(())
}

fn model_index(
    catalog: &Value,
) -> Result<BTreeMap<(String, String), AvatarModelReference>, String> {
    let mut out = BTreeMap::new();
    for row in array(catalog, "models")? {
        let name = row["trueName"].as_str().ok_or("model name")?;
        let key = (
            row["category"].as_str().ok_or("model category")?.to_owned(),
            name.to_ascii_lowercase(),
        );
        let value = AvatarModelReference {
            true_name: name.into(),
            exact_route: format!("wear/{name}.nif"),
            native_asset: serde_json::from_value(row["model"].clone())
                .map_err(|e| e.to_string())?,
        };
        // Preserve accepted multi-model variants; the table resolver starts with the canonical row.
        out.entry(key).or_insert(value);
    }
    Ok(out)
}

fn owner_set(catalog: &Value, category: &str, mesh: &Value) -> Result<Value, String> {
    for field in ["m_pstrMMeshModelString", "m_pstrFMeshModelString"] {
        let Some(name) = mesh[field].as_str() else {
            continue;
        };
        if let Some(model) = array(catalog, "models")?.iter().find(|r| {
            r["category"] == category
                && r["trueName"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        }) {
            return array(catalog, "sets")?
                .iter()
                .find(|s| s["id"] == model["resourceSet"])
                .cloned()
                .ok_or("missing model resource set".into());
        }
    }
    Err("item has no native resource owner".into())
}

fn append_set_texture(
    root: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    preimages: &mut BTreeMap<String, Vec<u8>>,
    catalog: &mut Value,
    id: &str,
    texture: Value,
) -> Result<(), String> {
    let set = catalog["sets"]
        .as_array_mut()
        .ok_or("catalog sets")?
        .iter_mut()
        .find(|s| s["id"] == id)
        .ok_or("set missing")?;
    let route = set["definition"]["path"]
        .as_str()
        .ok_or("definition path")?
        .to_owned();
    let bytes = files
        .get(&route)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| fs::read(root.join(&route)).map_err(|e| e.to_string()))?;
    if root.join(&route).exists() && !preimages.contains_key(&route) {
        preimages.insert(route.clone(), bytes.clone());
    }
    let mut document: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let rows = document["textures"].as_array_mut().ok_or("set textures")?;
    if !rows.iter().any(|r| r["path"] == texture["path"]) {
        rows.push(texture);
    }
    set["textureCount"] = json!(rows.len());
    let bytes = p::encode(&document)?;
    set["definition"] = reference(&route, &bytes);
    files.insert(route, bytes);
    Ok(())
}
