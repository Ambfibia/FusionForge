use super::*;

#[derive(Debug)]
pub enum AvatarCatalogError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid(String),
}

impl fmt::Display for AvatarCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::Json { path, source } => {
                write!(formatter, "invalid JSON {}: {source}", path.display())
            }
            Self::Invalid(detail) => formatter.write_str(detail),
        }
    }
}

impl Error for AvatarCatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

pub(super) fn require_input_schema(input: &PlanInput, expected: &str) -> Result<(), AvatarCatalogError> {
    if input.schema == expected {
        Ok(())
    } else {
        Err(AvatarCatalogError::Invalid(format!(
            "{} evidence schema {:?} does not match {:?}",
            input.role, input.schema, expected
        )))
    }
}

pub(super) fn require_avatar_tables(root: &Value) -> Result<(), AvatarCatalogError> {
    for table in [
        "m_pBackItemTable",
        "m_pFaceItemTable",
        "m_pGlassItemTable",
        "m_pHatItemTable",
        "m_pHeadItemTable",
        "m_pPantsItemTable",
        "m_pShirtsItemTable",
        "m_pShoesItemTable",
        "m_pVehicleItemTable",
        "m_pWeaponItemTable",
    ] {
        if !root.get(table).is_some_and(Value::is_object) {
            return Err(AvatarCatalogError::Invalid(format!(
                "TableData is missing required avatar table {table}"
            )));
        }
    }
    Ok(())
}
