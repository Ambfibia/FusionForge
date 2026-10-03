use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct BufferViewInfo {
    pub(super) offset: usize,
    pub(super) length: usize,
    pub(super) stride: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AccessorInfo {
    pub(super) view: BufferViewInfo,
    pub(super) accessor_offset: usize,
    pub(super) component_type: u32,
    pub(super) component_count: usize,
    pub(super) count: usize,
    pub(super) normalized: bool,
}

impl AccessorInfo {
    pub(super) fn element_size(self) -> Option<usize> {
        component_size(self.component_type)?.checked_mul(self.component_count)
    }

    pub(super) fn stride(self) -> Option<usize> {
        Some(self.view.stride.unwrap_or(self.element_size()?))
    }

    pub(super) fn component_offset(self, element: usize, component: usize) -> Option<usize> {
        self.view
            .offset
            .checked_add(self.accessor_offset)?
            .checked_add(element.checked_mul(self.stride()?)?)?
            .checked_add(component.checked_mul(component_size(self.component_type)?)?)
    }
}
