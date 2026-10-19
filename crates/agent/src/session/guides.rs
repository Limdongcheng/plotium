//! New guide calls are strict and bounded without changing older property calls.
use super::*;
use plotium_core::properties::guide;

fn applies(parameters: &Value) -> bool {
    parameters
        .get("key")
        .and_then(Value::as_str)
        .and_then(properties::definition_by_key)
        .is_some_and(|d| guide::is_guide_property(d.id))
}

pub(super) fn validate_request(request: &mut ToolRequest) -> Result<(), Failure> {
    if !applies(&request.parameters) {
        return Ok(());
    }
    let TargetSelector::Explicit { ids } = &mut request.targets else {
        return Err(Failure::new(
            "invalid_parameters",
            "Guides require explicit plot object ids, not components",
        ));
    };
    if ids.is_empty() || ids.len() > 200 {
        return Err(Failure::new(
            "invalid_parameters",
            "Guide requests require 1..=200 plot object ids",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    if request.parameters.get("key").and_then(Value::as_str) == Some(guide::TITLE.as_str())
        && request
            .parameters
            .get("value")
            .and_then(Value::as_str)
            .is_some_and(|s| s.len() > 16384)
    {
        return Err(Failure::new(
            "invalid_parameters",
            "Guide title exceeds 16384 UTF-8 bytes",
        ));
    }
    Ok(())
}

pub(super) fn validate_plan(plan: &ToolPlan) -> Result<(), Failure> {
    if applies(&plan.request.parameters)
        && plan
            .targets
            .iter()
            .any(|t| t.status != TargetCompatibility::Compatible)
    {
        return Err(Failure::new(
            "invalid_parameters",
            "Every guide write target must be an applicable plot object; nothing was committed",
        ));
    }
    Ok(())
}

pub(super) fn check_response(value: &Value, parameters: &Value) -> Result<(), Failure> {
    if applies(parameters)
        && serde_json::to_vec(value)
            .map_err(|e| Failure::new("execution_failed", e.to_string()))?
            .len()
            > DOCUMENT_RESPONSE_BYTES
    {
        return Err(Failure::new(
            "response_too_large",
            "Guide response exceeds 256 KiB; use fewer objects or shorter titles",
        ));
    }
    Ok(())
}
