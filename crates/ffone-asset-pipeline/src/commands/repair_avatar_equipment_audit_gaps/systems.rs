use super::*;

pub(super) fn apply_white_slot_repair(
    avatar: &mut CharacterCreationAvatarItems,
    repair: WhiteSlotRepair,
    donor: &AvatarTextureReference,
    evidence: &mut Vec<Value>,
) -> Result<(), String> {
    require_verified_texture(donor, "white-slot donor")?;
    for gender in RepairGender::ALL {
        let target_name = item(avatar, repair.category, repair.item_number)?
            .name
            .clone();
        let target = visual_mut(avatar, repair.category, repair.item_number, gender)?;
        target.primary_texture = Some(donor.clone());
        evidence.push(json!({
            "kind": "implicit-white-slot-texture",
            "category": repair.category,
            "itemNumber": repair.item_number,
            "itemName": target_name,
            "gender": gender,
            "donorTexture": donor
        }));
    }
    Ok(())
}
