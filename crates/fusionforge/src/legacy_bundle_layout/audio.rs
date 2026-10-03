use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct AudioObjectIdentity {
    pub(super) name: String,
    pub(super) semantic_sha1: String,
    pub(super) payload_sha1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PatchedAudioIdentity {
    pub container: String,
    pub asset: String,
    pub path_id: i64,
    pub name: String,
    pub replacement_sha1: String,
}

pub(super) fn audio_object_identity(value: &UnityValue) -> Result<AudioObjectIdentity, String> {
    let payload = audio_payload(value)?;
    Ok(AudioObjectIdentity {
        name: object_name(value),
        semantic_sha1: semantic_value_sha1("AudioClip", value),
        payload_sha1: sha1_hex(payload),
    })
}

pub(super) const USE_TUTORIAL_AUDIO: u16 = 1 << 1;

pub(super) const USE_UI_AUDIO: u16 = 1 << 2;

pub(super) const USE_NPC_VOICE: u16 = 1 << 3;

pub(super) fn patched_audio_nodes(
    inventory: &Inventory,
    identities: &[PatchedAudioIdentity],
) -> Result<BTreeMap<NodeKey, String>, String> {
    let mut result = BTreeMap::<NodeKey, String>::new();
    for identity in identities {
        let mut matches = Vec::<NodeKey>::new();
        for (asset_index, asset) in inventory.assets.iter().enumerate() {
            let bundle = &inventory.bundles[asset.bundle_index].name;
            if !bundle.eq_ignore_ascii_case(&identity.container)
                || !asset.name.eq_ignore_ascii_case(&identity.asset)
            {
                continue;
            }
            if let Some(path_id) = asset_has_path(asset, identity.path_id) {
                matches.push((asset_index, path_id));
            }
        }
        if matches.is_empty() {
            // main/TableData/music are retained rather than repacked.
            if !identity
                .container
                .to_ascii_lowercase()
                .ends_with(".resourcefile")
                || matches!(
                    identity.container.to_ascii_lowercase().as_str(),
                    "tabledata.resourcefile"
                        | "futuremusic.resourcefile"
                        | "lobbymusic.resourcefile"
                        | "pastmusic.resourcefile"
                        | "retromusic.resourcefile"
                )
            {
                continue;
            }
            return Err(format!(
                "patched audio identity was not found in repack inventory: {} / {} #{}",
                identity.container, identity.asset, identity.path_id
            ));
        }
        if matches.len() != 1 {
            return Err(format!(
                "patched audio identity is ambiguous: {} / {} #{} -> {matches:?}",
                identity.container, identity.asset, identity.path_id
            ));
        }
        let key = matches[0];
        let node = inventory
            .nodes
            .get(&key)
            .ok_or_else(|| format!("patched AudioClip node {key:?} is absent"))?;
        if node.object_type != "AudioClip" {
            return Err(format!(
                "patched audio identity {key:?} is {}",
                node.object_type
            ));
        }
        if node.name != identity.name {
            return Err(format!(
                "patched audio identity {key:?} name mismatch: status='{}', object='{}'",
                identity.name, node.name
            ));
        }
        let replacement_sha1 = identity.replacement_sha1.trim().to_ascii_lowercase();
        if replacement_sha1.len() != 40
            || !replacement_sha1
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(format!(
                "patched audio identity {key:?} has invalid replacementSha1 '{}'",
                identity.replacement_sha1
            ));
        }
        let payload_sha1 = node.audio_payload_sha1.as_ref().ok_or_else(|| {
            format!("patched AudioClip node {key:?} has no verified embedded payload")
        })?;
        if payload_sha1 != &replacement_sha1 {
            return Err(format!(
                "patched AudioClip {key:?} '{}' payload mismatch: status={}, source={}",
                node.name, replacement_sha1, payload_sha1
            ));
        }
        if let Some(existing) = result.insert(key, replacement_sha1.clone()) {
            return Err(format!(
                "duplicate patched audio identity resolved to {key:?}: {} and {}",
                existing, replacement_sha1
            ));
        }
    }
    Ok(result)
}
