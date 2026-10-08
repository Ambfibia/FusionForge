use super::*;
use crate::fusionforge::unity::{ObjectKey, UnityEnvironment};
use base64::{Engine, engine::general_purpose::STANDARD};

pub(super) struct Resources {
    pub models: BTreeMap<String, String>,
    pub textures: BTreeMap<String, ObjectKey>,
    env: UnityEnvironment,
    bundle: PathBuf,
}

impl Resources {
    pub fn open(source: &Path, category: &str) -> Result<Self, String> {
        let bundle = source.join(format!("CharacterSelection_wear_{category}.resourceFile"));
        let model_assets = crate::fusionforge::direct_input::read_assets(&bundle)?;
        let mut models = BTreeMap::new();
        for (index, asset) in model_assets.iter().enumerate() {
            for info in asset
                .objects
                .values()
                .filter(|i| asset.object_type_name(i) == "AssetBundle")
            {
                let object =
                    crate::fusionforge::cli::unity_to_json(&asset.read_object(index, info)?);
                for pair in object["m_Container"].as_array().into_iter().flatten() {
                    if let Some(route) = pair
                        .get("first")
                        .or_else(|| pair.get(0))
                        .and_then(Value::as_str)
                        .filter(|r| r.ends_with(".nif"))
                    {
                        let name = Path::new(route)
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_ascii_lowercase();
                        if models.insert(name.clone(), route.into()).is_some() {
                            return Err(format!("ambiguous Academy model {name}"));
                        }
                    }
                }
            }
        }
        // Equipment tables intentionally share atlases across category boundaries.
        let mut assets = Vec::new();
        for bucket in [
            "back", "glass", "hat", "pants", "shirts", "shoes", "vehicle", "weapon",
        ] {
            assets.extend(crate::fusionforge::direct_input::read_assets(
                &source.join(format!("Character_Texture_{bucket}.resourceFile")),
            )?);
        }
        let env = UnityEnvironment::from_assets(assets);
        let mut textures = BTreeMap::new();
        for (index, asset) in env.assets.iter().enumerate() {
            for (id, info) in asset
                .objects
                .iter()
                .filter(|(_, i)| asset.object_type_name(i) == "Texture2D")
            {
                let object = asset.read_object(index, info)?;
                let name = crate::fusionforge::object_name(&object).to_ascii_lowercase();
                if textures
                    .insert(
                        name.clone(),
                        ObjectKey {
                            asset: index,
                            path_id: *id,
                        },
                    )
                    .is_some()
                {
                    return Err(format!("ambiguous Academy Texture2D {name}"));
                }
            }
        }
        Ok(Self {
            models,
            textures,
            env,
            bundle,
        })
    }

    pub fn install_model(
        &self,
        name: &str,
        category: &str,
        root: &Path,
        files: &mut BTreeMap<String, Vec<u8>>,
        catalog: &mut Value,
    ) -> Result<(), String> {
        let route = self.models.get(name).ok_or("model source route")?;
        let source = crate::preview_bundle_container_model_exact(
            self.bundle.display().to_string(),
            None,
            route.clone(),
        )?;
        let family = category.to_ascii_uppercase();
        let package = format!("characters/player/items/{category}/{name}");
        let options =
            ffone_asset_pipeline::LogicalModelPublishOptions::new("in-memory", &family, root)
                .with_semantic_root_layout();
        let (report, converted) = ffone_asset_pipeline::prepare_direct_model(
            &options,
            &serde_json::to_vec(&source).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("{name}: {e}"))?;
        let glb_route = report.contract.output_glb.replace('\\', "/");
        let glb_parent = Path::new(&glb_route).parent().ok_or("model directory")?;
        let prefix = format!("{package}/models/{name}");
        let mut native_model = None;
        let mut textures = Vec::new();
        for (path, bytes) in converted {
            let relative = path
                .strip_prefix(glb_parent)
                .map_err(|_| format!("model dependency outside owning directory: {path:?}"))?;
            let destination = if path == Path::new(&glb_route) {
                format!("{prefix}/model.glb")
            } else {
                format!("{prefix}/{}", relative.to_string_lossy().replace('\\', "/"))
            };
            let artifact = reference(&destination, &bytes);
            if path == Path::new(&glb_route) {
                native_model = Some(artifact.clone());
            } else if path.extension().and_then(|s| s.to_str()) == Some("png")
                && !destination.contains(".mips/")
            {
                textures.push(artifact);
            }
            put(files, destination, bytes)?;
        }
        let model = native_model.ok_or("converted GLB missing")?;
        let identity_route = format!("characters/player/equipment/{category}/{name}/{name}.glb");
        let hash = blake3::hash(identity_route.as_bytes()).to_hex().to_string();
        let id = format!("player-item-{hash}");
        let set_id = format!("player-item-set-{hash}");
        let source_route = format!("{category}/{name}/{name}.glb");
        let item = json!({"schema":"ffone.player-item.v1","id":id,"trueName":name,"category":category,"sourceRoute":source_route,"resourceSet":set_id,"model":model});
        let item_route = format!("{prefix}/item.json");
        let item_bytes = p::encode(&item)?;
        let set = json!({"schema":"ffone.resource-set.v1","id":set_id,"name":name,"domain":"player_item","category":category,"prefix":"PLAYER","family":family,"textures":textures,"members":[{"id":id,"name":name,"definition":reference(&item_route,&item_bytes),"files":[model]}]});
        let set_route = format!("{package}/set.json");
        let set_bytes = p::encode(&set)?;
        if root.join(&set_route).exists() {
            return Err(format!("preserving existing package {package}"));
        }
        catalog["models"].as_array_mut().ok_or("catalog models")?.push(json!({"trueName":name,"category":category,"sourceRoute":source_route,"resourceSet":set_id,"model":model}));
        catalog["sets"].as_array_mut().ok_or("catalog sets")?.push(json!({"id":set_id,"name":name,"category":category,"prefix":"PLAYER","family":family,"definition":reference(&set_route,&set_bytes),"memberCount":1,"textureCount":textures.len()}));
        put(files, item_route, item_bytes)?;
        put(files, set_route, set_bytes)?;
        Ok(())
    }

    pub fn install_texture(
        &self,
        name: &str,
        route: &str,
        root: &Path,
        files: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<Value, String> {
        let key = *self.textures.get(name).ok_or("texture source")?;
        let texture = crate::logical_model_material::exact_texture(&self.env, key)?;
        let mut png = payload(&texture["payload"])?;
        let retained = root.join(route).exists();
        if retained {
            png = fs::read(root.join(route)).map_err(|e| e.to_string())?;
        }
        let image = image::load_from_memory(&png)
            .map_err(|e| e.to_string())?
            .to_rgba8();
        put(files, route.into(), png.clone())?;
        // Keep the complete decoded source chain alongside the editable base image.
        for level in texture["mipLevels"]
            .as_array()
            .into_iter()
            .flatten()
            .skip(1)
            .filter(|_| !retained)
        {
            let mip_route = format!(
                "{}.mips/mip-{}.png",
                route.trim_end_matches(".png"),
                level["level"]
            );
            put(files, mip_route, payload(&level["payload"])?)?;
        }
        let sampler = &texture["sampler"];
        let filter = sampler["filterMode"]["value"]
            .as_i64()
            .ok_or("filter mode")?;
        let wrap = sampler["wrapMode"]["value"].as_i64().ok_or("wrap mode")?;
        let count = texture["mipCount"].as_u64().ok_or("mip count")?;
        let min = match (filter, count > 1) {
            (0, false) => "nearest",
            (0, true) => "nearestMipmapNearest",
            (1, false) | (2, false) => "linear",
            (1, true) => "linearMipmapNearest",
            (2, true) => "linearMipmapLinear",
            _ => return Err("unsupported texture filter".into()),
        };
        let wrap = match wrap {
            0 => "repeat",
            1 => "clampToEdge",
            2 => "mirroredRepeat",
            _ => return Err("unsupported texture wrap".into()),
        };
        let native_source = if retained {
            json!({"asset":route,"containerRoute":route,"pathId":0,"width":image.width(),"height":image.height(),"textureFormat":4,"textureFormatName":"RGBA32","completeImageSize":image.as_raw().len(),"sourceChainSha256":p::digest(image.as_raw()),"mipMap":false,"sourceMipCount":1,"imageCount":1,"textureDimension":2})
        } else {
            json!({"asset":route,"containerRoute":route,"pathId":0,"width":texture["width"],"height":texture["height"],"textureFormat":texture["textureFormat"],"textureFormatName":texture["textureFormatName"],"completeImageSize":texture["sourcePayload"]["byteLength"],"sourceChainSha256":texture["sourcePayload"]["sha256"],"mipMap":count>1,"sourceMipCount":count,"imageCount":1,"textureDimension":2})
        };
        Ok(json!({
            "trueName":name,"nativeAsset":reference(route,&png),"nativePngSha256":p::digest(&png),
            "source":native_source,
            "usageColorSpace":"srgb","usageColorSpaceSource":"ActorSkinCombiner _MainTex native equipment slot",
            "sampler":{"name":name,"magFilter":if filter==0 {"nearest"} else {"linear"},"minFilter":min,"wrapS":wrap,"wrapT":wrap,"legacyFilterMode":filter,"legacyWrapMode":sampler["wrapMode"]["value"],"anisotropyLevel":sampler["aniso"]["value"],"mipMapBias":sampler["mipBias"]["value"]},"publishedMipPolicy":"baseLevelOnly"
        }))
    }
}

fn payload(value: &Value) -> Result<Vec<u8>, String> {
    STANDARD
        .decode(
            value["dataUrl"]
                .as_str()
                .ok_or("PNG payload")?
                .strip_prefix("data:image/png;base64,")
                .ok_or("PNG data URL")?,
        )
        .map_err(|e| e.to_string())
}
