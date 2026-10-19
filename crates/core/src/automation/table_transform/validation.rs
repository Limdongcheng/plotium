use super::*;
use plotium_data::{Expression, LogicalType, Relation};

pub(super) fn validate_wire(value: &Value) -> Result<(), AutomationError> {
    if serde_json::to_vec(value)
        .map_err(|e| invalid(e.to_string()))?
        .len()
        > TABLE_TRANSFORM_PLAN_BYTES
    {
        return Err(invalid("Transform parameters exceed 64 KiB."));
    }
    let mut pending = vec![(value, 0)];
    while let Some((v, depth)) = pending.pop() {
        if depth > 48 {
            return Err(invalid("Transform JSON nesting exceeds 48."));
        }
        match v {
            Value::Object(o) => pending.extend(o.values().map(|v| (v, depth + 1))),
            Value::Array(a) => pending.extend(a.iter().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    if let Some(plan) = value.get("plan") {
        keys(plan, &["version", "operation_id", "root"])?;
        if let Some(root) = plan.get("root") {
            wire_relation(root)?;
        }
    }
    Ok(())
}
pub(super) fn validate_params(p: &TableTransformParams) -> Result<(), AutomationError> {
    if p.name.trim().is_empty() || p.name.len() > 256 {
        return Err(invalid("Name requires 1–256 UTF-8 bytes."));
    }
    if p.memory_limit_bytes == 0 || p.memory_limit_bytes > TABLE_TRANSFORM_MEMORY {
        return Err(invalid("memory_limit_bytes must be 1–536870912."));
    }
    validate_plan(&p.plan)
}
pub(super) fn validate_plan(plan: &RelPlanV1) -> Result<(), AutomationError> {
    plan.validate().map_err(|e| invalid(e.to_string()))?;
    relation(&plan.root, 1, &mut 0)
}
fn node(depth: usize, count: &mut usize) -> Result<(), AutomationError> {
    *count += 1;
    if depth > TABLE_TRANSFORM_DEPTH || *count > TABLE_TRANSFORM_NODES {
        return Err(invalid(
            "Transform allows at most 64 relation/expression nodes and depth 16.",
        ));
    }
    Ok(())
}
fn relation(r: &Relation, depth: usize, count: &mut usize) -> Result<(), AutomationError> {
    node(depth, count)?;
    match r {
        Relation::SnapshotRead(_) => Ok(()),
        Relation::Project { input, .. } => relation(input, depth + 1, count),
        Relation::StableSort { input, keys } => {
            if keys.is_empty() {
                return Err(invalid("Stable sort requires at least one key."));
            }
            relation(input, depth + 1, count)
        }
        Relation::ComputedColumn {
            input,
            expression,
            column,
        } => {
            if !matches!(
                column.logical_type,
                LogicalType::Boolean
                    | LogicalType::Int64
                    | LogicalType::Float64
                    | LogicalType::Utf8
            ) {
                return Err(invalid(
                    "Computed outputs support boolean, int64, float64 and utf8.",
                ));
            }
            relation(input, depth + 1, count)?;
            expr(expression, depth + 1, count)
        }
        Relation::Filter { input, predicate } => {
            relation(input, depth + 1, count)?;
            expr(predicate, depth + 1, count)
        }
        _ => Err(invalid(
            "Only snapshot_read, project, filter, stable_sort and computed_column are exposed in data-only mode.",
        )),
    }
}
fn expr(e: &Expression, depth: usize, count: &mut usize) -> Result<(), AutomationError> {
    node(depth, count)?;
    match e {
        Expression::Column { .. } | Expression::Literal { .. } => Ok(()),
        Expression::Call { function, args } => {
            if ![
                "is_null.v1",
                "is_finite.v1",
                "eq.v1",
                "not.v1",
                "and.v1",
                "or.v1",
                "add.v1",
                "subtract.v1",
                "multiply.v1",
                "divide.v1",
            ]
            .contains(&function.as_str())
            {
                return Err(invalid(format!("Unsupported function {function}.")));
            }
            for a in args {
                expr(a, depth + 1, count)?;
            }
            Ok(())
        }
        Expression::Cast { input, target, .. } => {
            if !matches!(
                target,
                LogicalType::Boolean
                    | LogicalType::Int64
                    | LogicalType::Float64
                    | LogicalType::Utf8
            ) {
                return Err(invalid("Cast targets are boolean, int64, float64 or utf8."));
            }
            expr(input, depth + 1, count)
        }
    }
}
pub(super) fn validate_input(
    plan: &RelPlanV1,
    input: &TypedTableState,
) -> Result<(), AutomationError> {
    let mut root = &plan.root;
    while let Relation::Project { input, .. }
    | Relation::Filter { input, .. }
    | Relation::StableSort { input, .. }
    | Relation::ComputedColumn { input, .. } = root
    {
        root = input;
    }
    let Relation::SnapshotRead(read) = root else {
        return Err(invalid("Expected one snapshot_read leaf."));
    };
    let actual = &input.envelope.revision;
    if read.table != actual.table_id {
        return Err(invalid(
            "Snapshot TableId does not belong to the selected DatasetId.",
        ));
    }
    if read.revision != actual.id {
        return Err(AutomationError::StaleTableRevision {
            expected: read.revision.to_string(),
            actual: actual.id.to_string(),
        });
    }
    if read.fingerprint != actual.snapshot.fingerprint {
        return Err(invalid(
            "Snapshot fingerprint differs from the selected revision.",
        ));
    }
    let catalog = std::collections::BTreeMap::from([(
        (actual.table_id, actual.id),
        actual.snapshot.schema.clone(),
    )]);
    plotium_data::typecheck_plan(plan, &catalog).map_err(|e| invalid(e.to_string()))?;
    Ok(())
}

// RelPlanV1 remains the native model. Its general-purpose deserializer tolerates
// extra keys, but this bounded write interface must not silently ignore a typo.
fn keys(v: &Value, allowed: &[&str]) -> Result<(), AutomationError> {
    if let Some(object) = v.as_object()
        && let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str()))
    {
        return Err(invalid(format!("Unknown plan field {key}.")));
    }
    Ok(())
}
fn wire_relation(r: &Value) -> Result<(), AutomationError> {
    let allowed: &[&str] = match r.get("op").and_then(Value::as_str) {
        Some("snapshot_read") => &["op", "table", "revision", "fingerprint"],
        Some("project") => &["op", "input", "columns"],
        Some("filter") => &["op", "input", "predicate"],
        Some("stable_sort") => &["op", "input", "keys"],
        Some("computed_column") => &["op", "input", "column", "expression"],
        _ => return Err(invalid("Unsupported relation node.")),
    };
    keys(r, allowed)?;
    if let Some(input) = r.get("input") {
        wire_relation(input)?;
    }
    for name in ["expression", "predicate"] {
        if let Some(e) = r.get(name) {
            wire_expr(e)?;
        }
    }
    if let Some(sort) = r.get("keys").and_then(Value::as_array) {
        for k in sort {
            keys(k, &["column", "direction", "nulls"])?;
        }
    }
    if let Some(c) = r.get("column") {
        keys(
            c,
            &[
                "id",
                "name",
                "logical_type",
                "nullable",
                "unit",
                "role",
                "constraints",
                "metadata",
            ],
        )?;
        if let Some(t) = c.get("logical_type") {
            keys(t, &["kind"])?;
        }
        if let Some(u) = c.get("unit") {
            keys(
                u,
                &[
                    "quantity",
                    "dimension",
                    "canonical_unit",
                    "display_unit",
                    "scale",
                    "offset",
                    "ucum",
                    "extension_id",
                ],
            )?;
        }
        if let Some(k) = c.get("constraints") {
            keys(k, &["unique"])?;
        }
    }
    Ok(())
}
fn wire_expr(e: &Value) -> Result<(), AutomationError> {
    let allowed: &[&str] = match e.get("expr").and_then(Value::as_str) {
        Some("column") => &["expr", "column"],
        Some("literal") => &["expr", "value"],
        Some("call") => &["expr", "function", "args"],
        Some("cast") => &["expr", "input", "target", "failure"],
        _ => return Err(invalid("Unsupported expression node.")),
    };
    keys(e, allowed)?;
    if let Some(input) = e.get("input") {
        wire_expr(input)?;
    }
    if let Some(args) = e.get("args").and_then(Value::as_array) {
        for a in args {
            wire_expr(a)?;
        }
    }
    if let Some(target) = e.get("target") {
        keys(target, &["kind"])?;
    }
    if e.get("expr").and_then(Value::as_str) == Some("literal")
        && let Some(v) = e.get("value")
    {
        keys(
            v,
            if v.get("type").and_then(Value::as_str) == Some("null") {
                &["type"]
            } else {
                &["type", "value"]
            },
        )?;
        if v.get("type").and_then(Value::as_str) == Some("float64")
            && let Some(f) = v.get("value")
        {
            keys(
                f,
                if f.get("kind").and_then(Value::as_str) == Some("finite") {
                    &["kind", "value"]
                } else {
                    &["kind"]
                },
            )?;
        }
    }
    Ok(())
}
