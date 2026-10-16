//! DataFusion adapter kept outside Plotium's lightweight data-model crate.
//!
//! This crate owns engine-specific lowering and streaming storage output. Its
//! public surface consists only of the backend adapter; Plotium IR and storage
//! types remain owned by `plotium-data`.

// Internal convenience only: the adapter reaches Plotium IR and storage helpers
// through `crate::`. Consumers depend on plotium-data directly rather than having
// its whole API re-exported from here.
pub(crate) use plotium_data::*;

pub mod backend;

pub use backend::*;
