use super::*;

pub(super) fn collect_table_references(
    table_set: &TableSetDocument,
) -> Result<BTreeMap<(u8, u32), IconReferenceGroup>> {
    let mut groups = BTreeMap::<(u8, u32), IconReferenceGroup>::new();
    for document in &table_set.tables {
        let object = document.value.as_object().ok_or_else(|| {
            invalid_error(format!(
                "TableData document {:?} is not an object",
                document.name
            ))
        })?;
        for (table_name, table_value) in object {
            let Some(table_object) = table_value.as_object() else {
                continue;
            };
            for (array_name, array_value) in table_object {
                if !array_name.to_ascii_lowercase().contains("icon") {
                    continue;
                }
                let Some(rows) = array_value.as_array() else {
                    continue;
                };
                for (row_index, row) in rows.iter().enumerate() {
                    let Some(row) = row.as_object() else {
                        continue;
                    };
                    let icon_type_value = row.get("m_iIconType");
                    let icon_number_value = row.get("m_iIconNumber");
                    if icon_type_value.is_none() && icon_number_value.is_none() {
                        continue;
                    }
                    let icon_type = icon_type_value.and_then(Value::as_u64).ok_or_else(|| {
                        invalid_error(format!(
                            "{}/{array_name}[{row_index}] has an incomplete or non-u64 m_iIconType",
                            table_name
                        ))
                    })?;
                    let icon_number = icon_number_value
                        .and_then(Value::as_u64)
                        .ok_or_else(|| {
                            invalid_error(format!(
                                "{}/{array_name}[{row_index}] has an incomplete or non-u64 m_iIconNumber",
                                table_name
                            ))
                        })?;
                    let icon_type = u8::try_from(icon_type)
                        .map_err(|_| invalid_error("legacy m_iIconType exceeds u8"))?;
                    let icon_number = u32::try_from(icon_number)
                        .map_err(|_| invalid_error("legacy m_iIconNumber exceeds u32"))?;
                    if icon_type == 0 && icon_number == 0 {
                        continue;
                    }
                    let reference = SemanticIconTableReference {
                        table_document: document.name.clone(),
                        table: table_name.clone(),
                        array: array_name.clone(),
                        row_index: row_index as u64,
                    };
                    groups
                        .entry((icon_type, icon_number))
                        .or_insert_with(|| IconReferenceGroup {
                            icon_type,
                            icon_number,
                            references: Vec::new(),
                        })
                        .references
                        .push(reference);
                }
            }
        }
    }
    for group in groups.values_mut() {
        group.references.sort_by(|left, right| {
            left.table_document
                .cmp(&right.table_document)
                .then_with(|| left.table.cmp(&right.table))
                .then_with(|| left.array.cmp(&right.array))
                .then_with(|| left.row_index.cmp(&right.row_index))
        });
        group.references.dedup();
    }
    Ok(groups)
}
