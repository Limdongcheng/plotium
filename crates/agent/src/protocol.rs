use crate::{Failure, PendingRequest, session::property_catalog};
use rmcp::{ErrorData, RoleServer, ServerHandler, model::*, service::RequestContext};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(crate) struct Handler {
    pub send: mpsc::Sender<PendingRequest>,
    pub cancel: CancellationToken,
    pub wake: Arc<dyn Fn() + Send + Sync>,
}
impl ServerHandler for Handler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("Plotium", env!("CARGO_PKG_VERSION")))
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: definitions(),
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !definitions().iter().any(|tool| tool.name == request.name) {
            return Err(ErrorData::invalid_params("Unknown tool", None));
        }
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        if request.name == "plotium_properties_list" {
            return Ok(if arguments.as_object().is_some_and(|a| a.is_empty()) {
                CallToolResult::structured(property_catalog())
            } else {
                CallToolResult::structured_error(
                    Failure::new("invalid_parameters", "Expected empty arguments").value(),
                )
            }
            .into());
        }
        let (response, mut receive) = oneshot::channel();
        let image_request = request.name == "plotium_render_preview";
        let pending = PendingRequest {
            name: request.name.into_owned(),
            arguments,
            response,
            cancellation: context.ct.clone(),
        };
        if self.send.try_send(pending).is_err() {
            return Ok(CallToolResult::structured_error(
                Failure::new("busy", "Desktop queue is full or stopped; retry").value(),
            )
            .into());
        }
        (self.wake)();
        let outcome = tokio::select! {
            biased;
            _ = context.ct.cancelled() => {
                receive.close();
                Err(Failure::new("request_cancelled", "Request cancelled before completion"))
            },
            _ = self.cancel.cancelled() => Err(Failure::new("connection_stopped", "Connection stopped")),
            result = &mut receive => result.unwrap_or_else(|_| Err(Failure::new("connection_stopped", "Desktop disconnected"))),
        };
        Ok(match outcome {
            Ok(value) if image_request => image_result(value, &context.id),
            Ok(value) => CallToolResult::structured(value),
            Err(error) => CallToolResult::structured_error(error.value()),
        }
        .into())
    }
}
pub(crate) fn definitions() -> Vec<Tool> {
    let revision = json!({"type": "integer", "minimum": 0});
    let ids = json!({"type": "array", "items": {"type": "string"}, "minItems": 1});
    let targets = json!({"type":"array", "minItems":1, "items": {
        "type":"object", "additionalProperties":false, "required":["resource","component"],
        "properties": {
            "resource": {"type":"object", "additionalProperties":false, "required":["id","kind"], "properties": {
                "id":{"type":"string"}, "kind":{"type":"string"},
                "parent_id":{"type":["string","null"]}, "local_id":{"type":["string","null"]}
            }},
            "component":{"type":"object", "additionalProperties":false, "required":["kind","id"], "properties": {
                "kind":{"const":"series"}, "id":{"type":"integer","minimum":0,"maximum":u64::MAX}
            }}
        }
    }});
    let key = json!({"type": "string", "enum": crate::session::PROPERTIES.iter().map(|id| id.as_str()).collect::<Vec<_>>()});
    let registry = plotium_core::automation::ToolRegistry::built_in();
    let mut query = registry
        .get("resources.search")
        .and_then(|d| d.parameter_schema.get("properties"))
        .and_then(|p| p.get("query"))
        .cloned()
        .unwrap_or(json!({"type": "object"}));
    query["description"] = json!(
        "ResourceQuery fields: kinds (resource kind IDs), capabilities (capability IDs), name_contains, units (strings), metadata (string map), lineage_source, offset, limit. Omitted filters match all resources. Plot object kind: plotium.canvas.object; document ID: document."
    );
    let mut document = plotium_core::automation::document_inspect_schema();
    document["properties"]["expected_revision"] = revision.clone();
    document["required"] = json!(["expected_revision", "target", "view"]);
    let mut tools: Vec<Tool> = [
        ("plotium_get_state", "Read project overview, revision, selection, supported connection capabilities and permissions. Agent reads create saved run records.", json!({}), json!([])),
        ("plotium_resources_search", "Search current project resources using ResourceQuery.", json!({"expected_revision": revision, "query": query}), json!(["expected_revision", "query"])),
        ("plotium_resources_inspect", "Read descriptors for explicit stable resource IDs.", json!({"expected_revision": revision, "ids": ids}), json!(["expected_revision", "ids"])),
        ("plotium_properties_list", "List supported display property definitions: schema, units, scope and static applicability. Does not create a run record.", json!({}), json!([])),
        ("plotium_properties_inspect", "Read property values, defaults, bounds and applicability.", json!({"expected_revision": revision, "ids": ids, "targets": targets, "key": key}), json!(["expected_revision", "key"])),
        ("plotium_plan", "Preflight a supported edit on resource IDs or exact series targets. Returns a one-use opaque plan_id; at most 32 pending plans.", json!({"expected_revision": revision, "ids": ids, "targets": targets, "tool_id": {"enum": crate::capabilities::EDIT_OPERATIONS}, "parameters": {"type": "object", "description": "Core tool arguments: rename {name}; properties.set {key,value}; properties.reset {key}."}}), json!(["expected_revision", "tool_id", "parameters"])),
        ("plotium_execute", "Submit a saved plan once. Requires current desktop write permission and unchanged revision. Undo is available in Plotium.", json!({"plan_id": {"type": "string"}}), json!(["plan_id"])),
    ].into_iter().map(|(name, description, properties, required)| {
        let mut schema = json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false});
        if matches!(name, "plotium_properties_inspect" | "plotium_plan") {
            schema["oneOf"] = json!([
                {"required":["ids"], "not":{"required":["targets"]}},
                {"required":["targets"], "not":{"required":["ids"]}}
            ]);
            if name == "plotium_plan" {
                schema["allOf"] = json!([{"if":{"properties":{"tool_id":{"const":"resource.rename"}}},"then":{"required":["ids"],"not":{"required":["targets"]}}}]);
            }
        }
        Tool::new(name, description, schema.as_object().expect("local schema is an object").clone())
    }).collect();
    tools.push(Tool::new("plotium_document_inspect", "Inspect bounded document structure and source bindings by full stable target. view self returns details; children pages direct children. Never uses selection.", document.as_object().expect("local schema is an object").clone()));
    let mut preview = plotium_core::automation::image_preview_schema();
    preview["properties"]["expected_revision"] = json!({"type":"integer","minimum":0});
    preview["required"] = json!(["expected_revision", "target"]);
    tools.push(Tool::new("plotium_render_preview", "Read a whole-canvas PNG image with full target identity, frozen render_revision and pixel dimensions. No file I/O. Rejects missing assets and budget overflow.", preview.as_object().expect("local schema is an object").clone()));
    tools
}

pub(crate) fn image_result(mut value: Value, request_id: &RequestId) -> CallToolResult {
    let data = value
        .get_mut("value")
        .and_then(Value::as_object_mut)
        .and_then(|v| v.remove("png_base64"));
    let Some(Value::String(data)) = data else {
        return CallToolResult::structured_error(
            Failure::new("execution_failed", "Preview produced no PNG image").value(),
        );
    };
    let mut result = CallToolResult::structured(value);
    result.content.push(ContentBlock::image(data, "image/png"));
    let envelope = JsonRpcResponse {
        jsonrpc: JsonRpcVersion2_0,
        id: request_id.clone(),
        result: &result,
    };
    match serde_json::to_vec(&envelope) {
        Ok(bytes) if bytes.len() <= plotium_core::automation::PREVIEW_RESPONSE_BYTES => result,
        Ok(_) => CallToolResult::structured_error(
            Failure::new(
                "preview_budget_exceeded",
                "MCP preview response exceeds 6 MiB; request a smaller max_edge",
            )
            .value(),
        ),
        Err(error) => CallToolResult::structured_error(
            Failure::new(
                "execution_failed",
                format!("Could not encode preview response: {error}"),
            )
            .value(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_address_schemas_require_one_addressing_mode_and_series_identity() {
        for tool in definitions().into_iter().filter(|tool| {
            matches!(
                tool.name.as_ref(),
                "plotium_plan" | "plotium_properties_inspect"
            )
        }) {
            let schema = Value::Object((*tool.input_schema).clone());
            assert_eq!(
                schema["oneOf"],
                json!([
                    {"required":["ids"],"not":{"required":["targets"]}},
                    {"required":["targets"],"not":{"required":["ids"]}}
                ])
            );
            assert_eq!(schema["properties"]["ids"]["minItems"], 1);
            let targets = &schema["properties"]["targets"];
            assert_eq!(targets["minItems"], 1);
            assert_eq!(
                targets["items"]["required"],
                json!(["resource", "component"])
            );
            let component = &targets["items"]["properties"]["component"];
            assert_eq!(component["required"], json!(["kind", "id"]));
            assert_eq!(component["properties"]["kind"]["const"], "series");
            assert_eq!(component["properties"]["id"]["maximum"], json!(u64::MAX));
            assert_eq!(
                targets["items"]["properties"]["resource"]["additionalProperties"],
                false
            );
            if tool.name == "plotium_plan" {
                assert_eq!(schema["allOf"][0]["then"]["required"], json!(["ids"]));
            }
        }
    }
}
