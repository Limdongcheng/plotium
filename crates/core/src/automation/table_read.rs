//! Revision-bound native table metadata and range reads, never selection-based.
use super::*;
use crate::state::{Dataset, DatasetId, PlotiumApp, TableDataset};
use plotium_data::{ColumnId, RevisionId, ScalarValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const TABLE_COLUMNS: &str = "table.columns";
pub const TABLE_READ_ROWS: &str = "table.read_rows";
pub const TABLE_PAGE_DEFAULT: usize = 200;
pub const TABLE_PAGE_MAX: usize = 1000;
pub const TABLE_PAGE_CELLS: usize = 20_000;
pub const TABLE_RESPONSE_BYTES: usize = 256 * 1024;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableColumnsParams {
    pub dataset_id: DatasetId,
    pub table_revision: RevisionId,
    #[serde(default)]
    pub include_provenance: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableReadRowsParams {
    pub dataset_id: DatasetId,
    pub table_revision: RevisionId,
    #[serde(default)]
    pub column_ids: Vec<ColumnId>,
    #[serde(default)]
    pub offset: u64,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub include_provenance: bool,
}
fn default_limit() -> usize {
    TABLE_PAGE_DEFAULT
}
pub fn table_read_limits() -> Value {
    json!({"default_rows":TABLE_PAGE_DEFAULT,"max_rows":TABLE_PAGE_MAX,"max_cells":TABLE_PAGE_CELLS,"response_bytes":TABLE_RESPONSE_BYTES,"order":"native_snapshot","int64_encoding":"decimal_string"})
}
pub fn table_read_schema(rows: bool) -> Value {
    let mut schema = json!({"type":"object","additionalProperties":false,"required":["dataset_id","table_revision"],"properties":{
        "dataset_id":{"type":"string","format":"uuid"},"table_revision":{"type":"string","format":"uuid"}}});
    schema["properties"]["include_provenance"] = json!({"type":"boolean","default":false,"description":"Include the persisted recipe in metadata, or native sources for each returned row."});
    if rows {
        schema["properties"]["column_ids"] = json!({"type":"array","items":{"type":"string","format":"uuid"},"uniqueItems":true,"description":"Omitted or empty selects all columns in schema order; otherwise preserves projection order."});
        schema["properties"]["include_provenance"] = json!({"type":"boolean","default":false,"description":"Include pinned native row sources for this page. Imported rows have no derived sources. Unsupported lineage shapes fail explicitly."});
        schema["properties"]["offset"] = json!({"type":"integer","minimum":0,"default":0});
        schema["properties"]["limit"] = json!({"type":"integer","minimum":1,"maximum":TABLE_PAGE_MAX,"default":TABLE_PAGE_DEFAULT});
    }
    schema
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: TABLE_READ_ROWS.into(),
        message: message.into(),
    }
}
fn table(
    app: &PlotiumApp,
    id: DatasetId,
    revision: RevisionId,
) -> Result<&TableDataset, AutomationError> {
    let dataset = app
        .doc
        .dataset_index(id)
        .and_then(|i| app.doc.datasets.get(i))
        .ok_or_else(|| invalid("DatasetId does not exist."))?;
    let Dataset::Table(table) = dataset else {
        return Err(invalid("Dataset is not a native typed table."));
    };
    let actual = table.typed_state.envelope.revision.id;
    if revision != actual {
        return Err(AutomationError::StaleTableRevision {
            expected: revision.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(table)
}
pub(super) fn freeze_table(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<FrozenTargetSet, AutomationError> {
    let (id, revision) = if request.tool_id == TABLE_COLUMNS {
        let p: TableColumnsParams =
            super::registry::parse(TABLE_COLUMNS, request.parameters.clone())?;
        (p.dataset_id, p.table_revision)
    } else {
        let p: TableReadRowsParams =
            super::registry::parse(TABLE_READ_ROWS, request.parameters.clone())?;
        let t = table(app, p.dataset_id, p.table_revision)?;
        validate_page(t, &p)?;
        (p.dataset_id, p.table_revision)
    };
    table(app, id, revision)?;
    if request.targets
        != (TargetSelector::Explicit {
            ids: vec![id.to_string()],
        })
    {
        return Err(invalid(
            "Table requests require exactly their explicit DatasetId target.",
        ));
    }
    Ok(FrozenTargetSet {
        revision: request.expected_revision,
        targets: vec![id.into()],
        component_targets: vec![],
        reasons: vec![],
        total_matches: 1,
        truncated: false,
    })
}
fn validate_page(table: &TableDataset, p: &TableReadRowsParams) -> Result<(), AutomationError> {
    let snapshot = &table.typed_state.envelope.revision.snapshot;
    let width = if p.column_ids.is_empty() {
        snapshot.schema.columns.len()
    } else {
        p.column_ids.len()
    };
    if p.limit == 0
        || p.limit > TABLE_PAGE_MAX
        || width
            .checked_mul(p.limit)
            .is_none_or(|cells| cells > TABLE_PAGE_CELLS)
    {
        return Err(invalid(
            "Page requires 1–1000 rows and at most 20000 projected cells.",
        ));
    }
    if p.offset > snapshot.row_count {
        return Err(invalid("Offset exceeds table length."));
    }
    let mut selected = BTreeSet::new();
    for id in &p.column_ids {
        if !selected.insert(id) || snapshot.schema.column(*id).is_none() {
            return Err(invalid(
                "Projection contains a duplicate or absent ColumnId.",
            ));
        }
    }
    Ok(())
}
pub(super) fn table_metadata(dataset: &Dataset) -> Result<Value, AutomationError> {
    let Dataset::Table(table) = dataset else {
        return Err(invalid("Not a typed table."));
    };
    let revision = &table.typed_state.envelope.revision;
    let fields=dataset.field_descriptors().iter().map(|field|json!({"field_id":field.id,"resource":{"id":format!("{}/{}",dataset.resource_id(),field.local_id),"kind":KIND_FIELD,"parent_id":dataset.resource_id(),"local_id":field.local_id}})).collect::<Vec<_>>();
    let columns=revision.snapshot.schema.columns.iter().map(|c|json!({"column_id":c.id,"source_column_id":c.metadata.get("import.source_column_id"),"name":c.name,"logical_type":c.logical_type,"nullable":c.nullable,"unit":c.unit,
        "type_source":c.metadata.get("import.type_source").cloned().unwrap_or(json!("unknown")),"unit_source":c.metadata.get("import.unit_source").cloned().unwrap_or(json!("unknown"))})).collect::<Vec<_>>();
    Ok(
        json!({"dataset_id":dataset.resource_id(),"table_id":revision.table_id,"table_revision":revision.id,"snapshot_fingerprint":revision.snapshot.fingerprint,"fields":fields,"columns":columns,"total_rows":revision.snapshot.row_count,
        "analyses":{"curve_fit_count":table.curve_fit_analyses.len(),"curve_fit_bindings":table.series_bindings.iter().filter(|b|b.fit.is_some()).count()},"diagnostics":revision.snapshot.metadata.get("space.nmrtist.plotium.import.diagnostics"),"inference":revision.snapshot.metadata.get("space.nmrtist.plotium.import.inference")}),
    )
}
pub(super) fn execute_table_read(
    app: &PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    let before = DocumentRevision(app.doc.automation_revision);
    let value = if plan.request.tool_id == TABLE_COLUMNS {
        let p: TableColumnsParams =
            super::registry::parse(TABLE_COLUMNS, plan.request.parameters.clone())?;
        table(app, p.dataset_id, p.table_revision)?;
        let index = app
            .doc
            .dataset_index(p.dataset_id)
            .ok_or_else(|| invalid("Dataset disappeared."))?;
        let mut metadata = table_metadata(&app.doc.datasets[index])?;
        if p.include_provenance {
            metadata["provenance"] =
                super::table_provenance::dataset_metadata(&app.doc.datasets[index]);
        }
        metadata["freshness"] = json!(super::table_update::connection_table_freshness(
            app,
            p.dataset_id
        ));
        metadata
    } else {
        let p: TableReadRowsParams =
            super::registry::parse(TABLE_READ_ROWS, plan.request.parameters.clone())?;
        let t = table(app, p.dataset_id, p.table_revision)?;
        validate_page(t, &p)?;
        let page = t
            .typed_rows_page(p.offset, p.limit, &p.column_ids)
            .map_err(AutomationError::Execution)?;
        let column_ids = page.columns.iter().map(|c| c.schema.id).collect::<Vec<_>>();
        let sources = if p.include_provenance {
            Some(super::table_provenance::row_sources(
                t,
                p.offset,
                &page.row_ids,
            )?)
        } else {
            None
        };
        let mut rows = Vec::new();
        // Account encoded rows incrementally, then check the complete ToolResult.
        let mut bytes = 1024 + column_ids.len() * 40;
        for (index, id) in page.row_ids.iter().enumerate() {
            let values = page
                .columns
                .iter()
                .map(|column| scalar_json(&column.values[index]))
                .collect::<Vec<_>>();
            let mut row = json!({"row_id":id,"values":values});
            if let Some(sources) = &sources {
                row["sources"] = sources[index].clone();
            }
            let encoded = serde_json::to_vec(&row)
                .map_err(|e| AutomationError::Execution(e.to_string()))?
                .len()
                + 1;
            if bytes + encoded > TABLE_RESPONSE_BYTES {
                if rows.is_empty() {
                    return Err(AutomationError::ResponseTooLarge);
                }
                break;
            }
            bytes += encoded;
            rows.push(row);
        }
        let next = p.offset + rows.len() as u64;
        json!({"dataset_id":p.dataset_id,"table_id":t.typed_state.envelope.revision.table_id,"table_revision":p.table_revision,"revision":before,
            "freshness":super::table_update::connection_table_freshness(app, p.dataset_id),"column_ids":column_ids,"offset":p.offset,"requested_rows":p.limit,"returned_rows":rows.len(),"total_rows":page.total_rows,"next_offset":if next<page.total_rows{Some(next)}else{None},"end":next==page.total_rows,"rows":rows})
    };
    let result = super::tool_executors::readonly_value(&plan.request.tool_id, before, value);
    if serde_json::to_vec(&result)
        .map_err(|e| AutomationError::Execution(e.to_string()))?
        .len()
        > TABLE_RESPONSE_BYTES
    {
        return Err(AutomationError::ResponseTooLarge);
    }
    Ok(result)
}
fn scalar_json(v: &ScalarValue) -> Value {
    match v {
        ScalarValue::Null => json!({"kind":"null"}),
        ScalarValue::Boolean(x) => json!({"kind":"boolean","value":x}),
        ScalarValue::Int64(x) => json!({"kind":"int64","value":x.to_string()}),
        ScalarValue::Utf8(x) => json!({"kind":"utf8","value":x}),
        ScalarValue::Float64(x) => {
            json!({"kind":"float64","value":if x.is_finite(){json!(x)}else{json!(if x.is_nan(){"NaN"}else if *x>0.0{"+Inf"}else{"-Inf"})}})
        }
        ScalarValue::Categorical(x) => json!({"kind":"categorical","value":x}),
        ScalarValue::Date(x) => json!({"kind":"date","value":x}),
        ScalarValue::Time(x) => json!({"kind":"time","value":x.to_string()}),
        ScalarValue::Timestamp(x) => json!({"kind":"timestamp","value":x.to_string()}),
        ScalarValue::Duration(x) => json!({"kind":"duration","value":x.to_string()}),
        ScalarValue::Extension { type_id, storage } => {
            json!({"kind":"extension","type_id":type_id,"storage":scalar_json(storage)})
        }
    }
}
