use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum RawAlphaReference {
    Literal(f64),
    FloatProperty(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum RawAlphaTest {
    Disabled,
    Enabled {
        compare: MaterialCompareFunction,
        reference: RawAlphaReference,
    },
}

#[derive(Clone, Debug)]
pub(super) struct ExpectedProgram {
    pub(super) category: RawState,
    pub(super) passes: Vec<RawState>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Block {
    pub(super) open: usize,
    pub(super) close: usize,
}
