//! Model-neutral automation primitives shared by the desktop, CLI and future
//! agents. The public boundary is deliberately semantic: callers can inspect,
//! select and invoke registered tools, but cannot construct document actions.

mod document;
mod preview;
mod properties;
pub use preview::*;
mod registry;
mod resources;
mod tasks;
mod tool_executors;
mod tools;
mod types;
mod workflow;

pub use document::{
    DOCUMENT_INSPECT, DOCUMENT_PAGE_DEFAULT, DOCUMENT_PAGE_MAX, DOCUMENT_RESPONSE_BYTES,
    DocumentInspectParams, DocumentView, document_inspect_schema, document_root_target,
    panel_resource,
};
pub use properties::{TOOL_INSPECT, TOOL_RESET, TOOL_SET, property_definition_description};
pub use registry::ToolRegistry;
pub use resources::*;
pub use tasks::*;
pub use tools::*;
pub use types::*;
pub use workflow::*;

pub const WORKFLOW_SCHEMA: &str = "plotium.workflow.v1";
pub const RUN_MANIFEST_SCHEMA: &str = "plotium.run-manifest.v1";

#[cfg(test)]
mod tests;
