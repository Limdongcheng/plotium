//! Prepared native insertion. All identities are fixed before it reaches history.
use crate::state::*;

#[derive(Clone)]
pub enum NativePlotInsertion {
    NewCanvas {
        page: Box<CanvasDocument>,
        active_before: Option<CanvasId>,
    },
    ExistingCanvas {
        canvas: CanvasId,
        object: Box<CanvasObject>,
        panel: Box<Panel>,
        new_panel: bool,
        additional_panel: bool,
        next_label_slot: u64,
    },
}
