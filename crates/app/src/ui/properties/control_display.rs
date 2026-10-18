//! Controls for atomic range intent; no direct axis state writes.
use super::*;
use plotium_core::properties::AxisRangeValue;

pub(super) fn range(
    row: &Row,
    current: &AxisRangeValue,
    pending: &mut Option<Pending>,
    gesture: &mut Option<(PropertyId, GestureEdge)>,
    ui: &mut Ui,
) {
    let ResolvedSchema::AxisRange {
        unit,
        view,
        manual_allowed,
    } = &row.representative.schema
    else {
        unreachable!("range control requires range schema")
    };
    let manual_allowed = *manual_allowed;
    if row.mixed() {
        for (name, value) in [
            ("Auto", AxisRangeValue::Auto),
            (
                "Manual",
                AxisRangeValue::Manual {
                    min: view[0],
                    max: view[1],
                    unit: unit.clone(),
                },
            ),
        ] {
            if ui
                .add_enabled(
                    manual_allowed || name == "Auto",
                    egui::Button::selectable(false, name),
                )
                .clicked()
            {
                *pending = Some(Pending::Write(
                    row.presentation.id,
                    PropertyValue::AxisRange(value),
                ));
            }
        }
        ui.weak(NO_SINGLE_VALUE);
        return;
    }
    let mut auto = matches!(current, AxisRangeValue::Auto);
    if ui
        .add_enabled(
            manual_allowed || !auto,
            egui::Checkbox::new(&mut auto, "Auto"),
        )
        .changed()
    {
        let value = if auto {
            AxisRangeValue::Auto
        } else {
            AxisRangeValue::Manual {
                min: view[0],
                max: view[1],
                unit: unit.clone(),
            }
        };
        *pending = Some(Pending::Write(
            row.presentation.id,
            PropertyValue::AxisRange(value),
        ));
    }
    {
        let (mut min, mut max) = match current {
            AxisRangeValue::Manual { min, max, .. } => (*min, *max),
            AxisRangeValue::Auto => (view[0], view[1]),
        };
        let span = (max - min).abs();
        let a = ui
            .add_enabled(
                manual_allowed && !auto,
                DragValue::new(&mut min)
                    .speed((span / 200.).max(1e-9))
                    .max_decimals(8)
                    .range(f64::MIN..=max.next_down()),
            )
            .on_hover_text("Minimum");
        let b = ui
            .add_enabled(
                manual_allowed && !auto,
                DragValue::new(&mut max)
                    .speed((span / 200.).max(1e-9))
                    .max_decimals(8)
                    .range(min.next_up()..=f64::MAX),
            )
            .on_hover_text("Maximum");
        note_gesture(row, &a, gesture);
        note_gesture(row, &b, gesture);
        if !auto && (a.changed() || b.changed()) {
            *pending = Some(Pending::Write(
                row.presentation.id,
                PropertyValue::AxisRange(AxisRangeValue::Manual {
                    min,
                    max,
                    unit: unit.clone(),
                }),
            ));
        }
    }
    ui.weak(unit.as_deref().unwrap_or("data units"));
}
