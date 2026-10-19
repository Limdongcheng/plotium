//! This connection's boundary, not the core registry's available algorithms.
use plotium_core::automation::{
    DOCUMENT_PAGE_DEFAULT, DOCUMENT_PAGE_MAX, DOCUMENT_RESPONSE_BYTES, document_root_target,
};
use serde_json::{Value, json};

pub(crate) const EDIT_OPERATIONS: [&str; 12] = [
    "resource.rename",
    "properties.set",
    "properties.reset",
    "data.import",
    "data.transform",
    "table.patch",
    "table.refresh",
    "plot.series.bind",
    "plot.create",
    "plot.series.append",
    "figure.edit",
    "plot.series.materialize_default",
];

pub(crate) fn manifest(allow_modify: bool) -> Value {
    let tools=crate::protocol::definitions().iter().map(|tool|{
        let (operations,result,targets)=match tool.name.as_ref(){
            "plotium_get_state"=>(vec!["project.get_blueprint"],"{revision,project,selection,permissions,connection_capabilities}","project"),
            "plotium_resources_search"=>(vec!["resources.search"],"ToolResult.value: FrozenTargetSet","ResourceQuery"),
            "plotium_resources_inspect"=>(vec!["resources.inspect"],"ToolResult.value: ResourceDescriptor[]","explicit resource IDs"),
            "plotium_properties_list"=>(vec!["properties.list"],"{properties:[{id,label,scope,schema,access,applicability}]}","static; no target"),
            "plotium_properties_inspect"=>(vec!["properties.inspect"],"ToolResult.value: {property,canonical_label,tier,aggregate,readings}; ToolResult.targets reports skips","ids expand by property, or exact Series TargetRef[]"),
            "plotium_plan"=>(EDIT_OPERATIONS.to_vec(),"{plan_id,preflight:ToolPlan,import?:{status,items},transform?:{status,output},update?:{status,output,before_table_revision,changed}}; import/transform only mode table_data_only; import omits ids/targets, transform requires one DatasetId in ids; prepared is not committed","rename: dataset/canvas/content IDs; properties: ids or exact Series targets; transform: one DatasetId; creation: explicit parent inside parameters, no top-level selector"),
            "plotium_execute"=>(EDIT_OPERATIONS.to_vec(),"ToolResult; submitted plan is consumed even on failure","one-use plan_id"),
            "plotium_document_inspect"=>(vec!["document.inspect"],"ToolResult.value: self {revision,node:{target,kind,name,visible,ancestors,child_count,...}}; children {revision,target,items,total,next_offset,truncated}","document, canvas, panel, content or exact Series TargetRef"),
            "plotium_table_columns"=>(vec!["table.columns"],"ToolResult.value: {dataset_id,table_id,table_revision,snapshot_fingerprint,fields,columns,total_rows,diagnostics,inference,freshness,analyses,provenance?}","explicit DatasetId and table RevisionId"),
            "plotium_table_read_rows"=>(vec!["table.read_rows"],"ToolResult.value: {dataset_id,table_id,table_revision,revision,column_ids,offset,requested_rows,returned_rows,total_rows,next_offset,end,rows:[{row_id,values,sources?}]} ; typed 64-bit integer values are decimal strings","explicit DatasetId, table RevisionId and ColumnId projection"),
            "plotium_render_preview"=>(vec!["render.preview_image"],"MCP PNG image content plus ToolResult.value: {target,render_revision,width_px,height_px,mime_type}","one exact whole-canvas TargetRef"),
            _=>unreachable!("all connection tools have discovery contracts"),
        };
        json!({"name":tool.name,"operations":operations,"input_schema":tool.input_schema,
            "result_contract":result,"targets":targets,
            "required_permission":if tool.name=="plotium_execute"{"reversible_modify"}else{"read"},
            "currently_permitted":tool.name!="plotium_execute" || allow_modify})
    }).collect::<Vec<_>>();
    json!({"tools":tools,"editable_operations":EDIT_OPERATIONS,
        "operation_constraints":{"plot.series.append":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::curve_append_schema(),"result_contract":"binding_plan.appended contains frozen new Series targets; existing bindings are retained; 200 total curves and shared binding row/plan/response budgets apply."},"table.patch":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::table_patch_schema()},"table.refresh":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::table_refresh_schema()},"figure.edit":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::figure_edit_schema(),"result_contract":"ToolPlan.figure_plan/result and ToolResult.value contain proposed created/modified exact targets, size_mm, margins/gutter and panel_frames. Frozen native assets commit once atomically; one Undo/Redo restores content and IDs. No automatic reflow."},"plot.create":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::plot_create_schema(),"result_contract":"ToolPlan.binding_plan and ToolResult.value contain creation {parent,canvas,panel,plot,created,size_mm,frame_pt,panel_frame_pt,coordinate_units,series_defaults}, and bindings [{target,source,diagnostics}]. Preflight IDs are proposed, not committed. One Undo removes only created content; Redo restores the same IDs. After response loss query proposed IDs/current revision before creating again."},"data.transform":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::table_transform_schema()},"plot.series.bind":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::curve_binding_schema(false)},"plot.series.materialize_default":{"preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify,"schema":plotium_core::automation::curve_binding_schema(true)},"data.import":{"mode":"table_data_only","preflight_permission":"reversible_modify","execute_permission":"reversible_modify","currently_permitted":allow_modify}},
        "supported_properties":crate::session::PROPERTIES.iter().map(|id|id.as_str()).collect::<Vec<_>>(),
        "resource_capabilities":"Core resource capabilities describe data/rendering semantics, not operations exposed by this connection. Only the operations above can be called here.",
        "document_root":document_root_target(),
        "limits":{"table_update":plotium_core::automation::table_update_limits(),"figure_edit":{"max_edits":200,"max_panels":200,"grid_rows_cols_max":16,"max_images":8,"image_file_bytes":1048576,"image_batch_bytes":4194304,"image_decoded_bytes":4194304,"text_utf8_bytes":16384},"plot_creation":{"max_plots":1,"max_series":200,"max_curve_rows":2000000,"pending_curve_rows_shared_with_bindings":2000000,"response_bytes":256*1024},"table_transform":plotium_core::automation::table_transform_limits(),"curve_binding":{"max_edits":200,"max_curve_rows":2000000,"pending_curve_rows":2000000,"response_bytes":256*1024,"diagnostic_rows":20},"document_page_default":DOCUMENT_PAGE_DEFAULT,"document_page_max":DOCUMENT_PAGE_MAX,"document_response_bytes":DOCUMENT_RESPONSE_BYTES,"pending_plans":32,"image_preview":plotium_core::automation::image_preview_limits(),"table_read":plotium_core::automation::table_read_limits(),"table_import":plotium_core::automation::table_import_limits()},
        "revision_contract":"Fetch get_state after edits or Undo. Revision-bound reads and plans reject stale revisions; execute rechecks permission and revision. Read run records mark dirty without advancing revision."})
}
