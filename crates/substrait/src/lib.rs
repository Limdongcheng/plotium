//! Opt-in Substrait exchange boundary. It is never linked by default.

pub(crate) use plotium_data::*;
pub(crate) use plotium_datafusion::compile_for_interop;

mod adapter;

pub use adapter::*;
