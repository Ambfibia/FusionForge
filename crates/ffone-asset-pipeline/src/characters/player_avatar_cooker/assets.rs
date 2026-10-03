use super::*;

pub(super) const ACTOR_ROUTE: &str = "actor/m.kfm";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteOwnership {
    pub exact_route: String,
    pub asset_bundle_asset_name: String,
    pub asset_bundle_path_id: i64,
    pub target_path_id: i64,
    pub target_object_type: String,
    pub true_name: String,
    pub preload_index: usize,
    pub preload_size: usize,
}

pub(super) struct DumpCatalog {
    pub(super) objects: Vec<DumpObject>,
    pub(super) by_path_id: HashMap<i64, usize>,
}

impl DumpCatalog {
    pub(super) fn read(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|error| io_at(path, error))?;
        let objects: Vec<DumpObject> =
            serde_json::from_reader(BufReader::new(file)).map_err(|source| {
                PipelineError::Json {
                    path: path.display().to_string(),
                    source,
                }
            })?;
        let mut by_path_id = HashMap::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            if by_path_id.insert(object.path_id, index).is_some() {
                return player_error(format!(
                    "object dump contains duplicate PathID {}",
                    object.path_id
                ));
            }
        }
        Ok(Self {
            objects,
            by_path_id,
        })
    }

    pub(super) fn get(&self, path_id: i64) -> Result<&DumpObject> {
        self.by_path_id
            .get(&path_id)
            .and_then(|index| self.objects.get(*index))
            .ok_or_else(|| {
                PipelineError::PlayerAvatarCook(format!(
                    "object dump has no object PathID {path_id}"
                ))
            })
    }

    pub(super) fn value(&self, path_id: i64) -> Result<Value> {
        let object = self.get(path_id)?;
        serde_json::from_str(object.value.get()).map_err(|source| PipelineError::Json {
            path: format!("object-dump#{}", object.path_id),
            source,
        })
    }

    pub(super) fn route(&self, exact_route: &str) -> Result<RouteOwnership> {
        let bundles = self
            .objects
            .iter()
            .filter(|object| object.object_type == "AssetBundle")
            .collect::<Vec<_>>();
        let [bundle] = bundles.as_slice() else {
            return player_error(format!(
                "expected exactly one AssetBundle object, found {}",
                bundles.len()
            ));
        };
        let value: Value =
            serde_json::from_str(bundle.value.get()).map_err(|source| PipelineError::Json {
                path: format!("AssetBundle#{}", bundle.path_id),
                source,
            })?;
        let entries = array_field(&value, "m_Container")?;
        let mut matched = Vec::new();
        for entry in entries {
            let Some(pair) = entry.as_array() else {
                continue;
            };
            if pair.len() != 2 || pair[0].as_str() != Some(exact_route) {
                continue;
            }
            let metadata = &pair[1];
            let target_path_id = pointer_path_id(
                metadata
                    .get("asset")
                    .ok_or_else(|| player_message("container entry has no asset pointer"))?,
            )?;
            let target = self.get(target_path_id)?;
            matched.push(RouteOwnership {
                exact_route: exact_route.to_owned(),
                asset_bundle_asset_name: bundle.asset.clone(),
                asset_bundle_path_id: bundle.path_id,
                target_path_id,
                target_object_type: target.object_type.clone(),
                true_name: target.name.clone(),
                preload_index: usize_field(metadata, "preloadIndex")?,
                preload_size: usize_field(metadata, "preloadSize")?,
            });
        }
        let [ownership] = matched.as_slice() else {
            return player_error(format!(
                "route {exact_route:?} resolved {} times; exact ownership is required",
                matched.len()
            ));
        };
        if ownership.target_object_type != "GameObject" {
            return player_error(format!(
                "route {exact_route:?} targets {}, not GameObject",
                ownership.target_object_type
            ));
        }
        validate_true_name("player route target", &ownership.true_name)?;
        Ok(ownership.clone())
    }
}

pub(super) fn component_path_ids(value: &Value) -> Result<Vec<i64>> {
    array_field(value, "m_Component")?
        .iter()
        .map(|entry| {
            let pair = entry
                .as_array()
                .ok_or_else(|| player_message("GameObject component is not a legacy pair"))?;
            if pair.len() != 2 {
                return player_error("GameObject component pair does not have two values");
            }
            pointer_path_id(&pair[1])
        })
        .collect()
}

pub(super) fn pointer_path_id(value: &Value) -> Result<i64> {
    value
        .get("pathId")
        .and_then(Value::as_i64)
        .ok_or_else(|| player_message("value is not a Unity pointer"))
}
