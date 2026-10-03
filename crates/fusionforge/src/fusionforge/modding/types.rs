
#[derive(Debug, Clone)]
pub struct ImportedMatrix4x4 {
    pub values: [f64; 16],
}

#[derive(Debug, Clone)]
pub struct ImportedVec3Curve {
    pub path: String,
    pub keys: Vec<ImportedVec3Key>,
}

#[derive(Debug, Clone)]
pub struct ImportedVec3Key {
    pub time: f64,
    pub value: (f64, f64, f64),
}

#[derive(Debug, Clone)]
pub struct ImportedQuatCurve {
    pub path: String,
    pub keys: Vec<ImportedQuatKey>,
}

#[derive(Debug, Clone)]
pub struct ImportedQuatKey {
    pub time: f64,
    pub value: (f64, f64, f64, f64),
}

#[derive(Debug, Clone)]
pub struct ImportedJoint {
    pub node_index: usize,
    pub path: String,
    pub parent_path: Option<String>,
    pub translation: (f64, f64, f64),
    pub rotation: (f64, f64, f64, f64),
    pub scale: (f64, f64, f64),
}
