use super::*;
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}
fn reference(name: &str) -> Value {
    json!({"$ref":format!("#/$defs/transform_{name}")})
}
pub fn table_transform_limits() -> Value {
    json!({"memory_default_bytes":TABLE_TRANSFORM_MEMORY,"memory_max_bytes":TABLE_TRANSFORM_MEMORY,"memory_semantics":"reference materialized-result estimate, not peak process memory","prepared_plans":TABLE_TRANSFORM_PREPARED,"pending_reserved_bytes":TABLE_TRANSFORM_MEMORY,"in_flight":1,"parameter_bytes":TABLE_TRANSFORM_PLAN_BYTES,"json_depth":48,"prepared_summary_bytes":TABLE_RESPONSE_BYTES/2,"nodes":TABLE_TRANSFORM_NODES,"depth":TABLE_TRANSFORM_DEPTH,"response_bytes":TABLE_RESPONSE_BYTES})
}
pub fn table_transform_schema() -> Value {
    let uuid = json!({"type":"string","format":"uuid"});
    let hash = json!({"type":"string","pattern":"^[0-9a-f]{64}$"});
    let types = [
        "null",
        "boolean",
        "int64",
        "float64",
        "utf8",
        "categorical",
        "date",
        "time",
        "timestamp",
        "duration",
    ];
    let mut literals = vec![object(json!({"type":{"const":"null"}}), &["type"])];
    for kind in &types[1..] {
        let value = match *kind {
            "boolean" => json!({"type":"boolean"}),
            "utf8" => json!({"type":"string"}),
            "float64" => {
                json!({"oneOf":[object(json!({"kind":{"const":"finite"},"value":{"type":"number"}}), &["kind","value"]),object(json!({"kind":{"enum":["nan","positive_infinity","negative_infinity"]}}), &["kind"])]})
            }
            "categorical" => json!({"type":"integer","minimum":0,"maximum":u32::MAX}),
            "date" => json!({"type":"integer","minimum":i32::MIN,"maximum":i32::MAX}),
            _ => {
                json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX,"description":"Exact native signed 64-bit integer; do not round in a JavaScript client."})
            }
        };
        literals.push(object(
            json!({"type":{"const":kind},"value":value}),
            &["type", "value"],
        ));
    }
    let cast_type = object(
        json!({"kind":{"enum":["boolean","int64","float64","utf8"]}}),
        &["kind"],
    );
    let declaration = table_declaration_schema();
    let mut column = declaration["properties"]["columns"]["items"].clone();
    column["properties"]["constraints"]["properties"]["unique"] =
        json!({"type":"boolean","default":false});
    // The output type is explicitly declared; projection and sorting preserve native types.
    column["properties"]["logical_type"] = cast_type.clone();
    column["properties"]["unit"]["description"] = json!(
        "Explicit output UnitSpec or null. Arithmetic does not infer units; copy the source unit for dimensionless scaling."
    );
    let expression = json!({"oneOf":[
        object(json!({"expr":{"const":"column"},"column":uuid}), &["expr","column"]),
        object(json!({"expr":{"const":"literal"},"value":{"oneOf":literals}}), &["expr","value"]),
        object(json!({"expr":{"const":"call"},"function":{"enum":["is_null.v1","is_finite.v1","eq.v1","not.v1","and.v1","or.v1","add.v1","subtract.v1","multiply.v1","divide.v1"],"description":"is_null(any)->Boolean; is_finite(Float64)->Boolean; eq(same type,same type)->Boolean; not(Boolean), and/or(Boolean,Boolean); arithmetic(Float64,Float64)->Float64. Null propagates except SQL Boolean logic and is_null. No implicit casts or unit conversion."},"args":{"type":"array","minItems":1,"maxItems":2,"items":reference("expression")}}), &["expr","function","args"]),
        object(json!({"expr":{"const":"cast"},"input":reference("expression"),"target":cast_type,"failure":{"enum":["error","null"],"description":"error rejects the entire transform; null replaces a failed conversion. Native casts support Int64->Float64, integral finite Float64->Int64, Utf8->Float64 and scalar->Utf8; Int64->Float64 can lose precision."}}), &["expr","input","target","failure"])
    ]});
    let relation = json!({"oneOf":[
        object(json!({"op":{"const":"snapshot_read"},"table":uuid,"revision":uuid,"fingerprint":hash}), &["op","table","revision","fingerprint"]),
        object(json!({"op":{"const":"project"},"input":reference("relation"),"columns":{"type":"array","minItems":1,"uniqueItems":true,"items":uuid}}), &["op","input","columns"]),
        object(json!({"op":{"const":"filter"},"input":reference("relation"),"predicate":reference("expression")}), &["op","input","predicate"]),
        object(json!({"op":{"const":"stable_sort"},"input":reference("relation"),"keys":{"type":"array","minItems":1,"items":object(json!({"column":uuid,"direction":{"enum":["ascending","descending"]},"nulls":{"enum":["first","last"]}}), &["column","direction","nulls"]),"description":"Keys in priority order, ties resolved by RowId; null placement is independent of direction. Float order: -Inf < finite < +Inf < NaN."}}), &["op","input","keys"]),
        object(json!({"op":{"const":"computed_column"},"input":reference("relation"),"column":column,"expression":reference("expression")}), &["op","input","column","expression"])
    ]});
    let mut schema = object(
        json!({"mode":{"const":"table_data_only"},"name":{"type":"string","minLength":1,"maxLength":256,"description":"1–256 UTF-8 bytes, not blank."},"plan":object(json!({"version":{"const":1},"operation_id":uuid,"root":reference("relation")}), &["version","operation_id","root"]),"memory_limit_bytes":{"type":"integer","minimum":1,"maximum":TABLE_TRANSFORM_MEMORY,"default":TABLE_TRANSFORM_MEMORY}}),
        &["mode", "name", "plan"],
    );
    schema["$defs"] = json!({"transform_relation":relation,"transform_expression":expression});
    schema["description"] = json!(
        "One explicit DatasetId in top-level ids. Native RelPlanV1: supply fresh UUIDs for operation_id and each computed column.id; reuse existing ColumnIds and the exact snapshot identity. Filter keeps true only; false/null are excluded. Prepared is not committed; execute the returned plan_id once."
    );
    schema
}
