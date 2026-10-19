//! Exact deletion planner over native curve editing and structural deletion snapshots.
use super::*;
use crate::actions::{DeletionChange, deletion::remove_members};
use crate::automation::{
    ComponentRef, DocumentRevision, TargetRef, canvas_object_ref, panel_resource,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

impl FigureEdit {
    pub(crate) fn deletion_target(&self) -> Option<&TargetRef> {
        match self {
            Self::ContentDelete { target }
            | Self::PanelDelete { target }
            | Self::SeriesDelete { target } => Some(target),
            _ => None,
        }
    }
}
impl PlotiumApp {
    pub(super) fn plan_figure_deletion(
        &self,
        revision: DocumentRevision,
        params: &FigureEditParams,
        original: &CanvasDocument,
    ) -> Result<FigureEditPlan, String> {
        let mut seen = Vec::new();
        let mut contents = BTreeSet::new();
        let mut panels = BTreeSet::new();
        let mut series: BTreeMap<ContentId, Vec<SeriesId>> = BTreeMap::new();
        let mut entries = Vec::new();
        let mut retained_assets = BTreeSet::new();
        for edit in &params.edits {
            let target = edit
                .deletion_target()
                .ok_or("Do not mix deletion with non-deletion edits.")?;
            if seen.contains(target) {
                return Err("Duplicate deletion target; supply each exact identity once.".into());
            }
            seen.push(target.clone());
            match edit {
                FigureEdit::PanelDelete { .. } => {
                    let (index, panel) = original
                        .panels
                        .iter()
                        .enumerate()
                        .find(|(_, p)| *target == TargetRef::resource(panel_resource(original, p)))
                        .ok_or("Discover an exact Panel target on this canvas.")?;
                    if panel.locked {
                        return Err("Panel is locked; unlock it before deletion.".into());
                    }
                    if !panel.item_order.is_empty() {
                        return Err("Panel is not empty; delete its contents in a separate transaction first.".into());
                    }
                    panels.insert(panel.id);
                    entries
                        .push(json!({"target":target,"panel_index":index,"parent":params.canvas}));
                }
                FigureEdit::ContentDelete { .. } | FigureEdit::SeriesDelete { .. } => {
                    let (index, item) = original
                        .objects
                        .iter()
                        .enumerate()
                        .find(|(_, o)| {
                            target.resource == canvas_object_ref(original.resource_id, o.id)
                        })
                        .ok_or("Discover an exact content owner on this canvas.")?;
                    original.validate_content_edit(item.id, false)?;
                    let parent = original
                        .parent_panel(item.id)
                        .and_then(|id| original.panel(id))
                        .map(|p| TargetRef::resource(panel_resource(original, p)))
                        .unwrap_or_else(|| params.canvas.clone());
                    if matches!(edit, FigureEdit::ContentDelete { .. }) {
                        if target.component.is_some() {
                            return Err(
                                "content_delete requires a whole-content target, not a Series."
                                    .into(),
                            );
                        }
                        contents.insert(item.id);
                        if let ContentKind::RasterImage(image) = &item.kind {
                            retained_assets.insert(image.asset);
                        }
                        let owned = item
                            .plot()
                            .map(|p| {
                                p.binding
                                    .series
                                    .iter()
                                    .map(|s| TargetRef {
                                        resource: target.resource.clone(),
                                        component: Some(ComponentRef::Series(s.id)),
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        entries.push(json!({"target":target,"parent":parent,"object_index":index,"owned_series_removed":owned}));
                    } else {
                        let Some(ComponentRef::Series(id)) = target.component else {
                            return Err(
                                "series_delete requires the owning plot and an exact SeriesId."
                                    .into(),
                            );
                        };
                        let plot = item.plot().ok_or("Series owner is not a plot.")?;
                        let position = plot
                            .binding
                            .series
                            .iter()
                            .position(|s| s.id == id)
                            .ok_or("SeriesId does not exist in this plot.")?;
                        series.entry(item.id).or_default().push(id);
                        entries.push(json!({"target":target,"parent":TargetRef::resource(target.resource.clone()),"series_index":position}));
                    }
                }
                _ => return Err("Do not mix deletion with non-deletion edits.".into()),
            }
        }
        if contents.iter().any(|id| series.contains_key(id)) {
            return Err(
                "Overlapping deletion: choose either the entire plot or its Series, not both."
                    .into(),
            );
        }
        let mut page = original.clone();
        let mut modified = vec![params.canvas.clone()];
        let mut remaining = Vec::new();
        let mut bindings = Vec::new();
        for (object, ids) in series {
            let plot = original
                .object(object)
                .and_then(ContentItem::plot)
                .ok_or("Plot missing.")?;
            if plot.display_owner.is_some()
                || plot.binding.coordinates.is_none()
                || plot
                    .binding
                    .series
                    .iter()
                    .any(|s| s.source.columns().is_none())
            {
                return Err("Delete Series only from a stable explicit-column plot; table_default and dynamic displays are not materialized or rebound.".into());
            }
            let mut after = plot.binding.clone();
            after.series.retain(|s| !ids.contains(&s.id));
            if after.series.is_empty() {
                return Err(
                    "Cannot delete the last Series; explicitly delete the whole content instead."
                        .into(),
                );
            }
            bindings.push(after.clone());
            self.check_curve_row_budget(bindings.iter())?;
            let plan = self.plan_curve_series_order(revision, page.resource_id, object, after)?;
            plan.copy_candidates_to(&mut page)?;
            let target = TargetRef::resource(canvas_object_ref(page.resource_id, object));
            modified.push(target.clone());
            let remaining_plot = page
                .object(object)
                .and_then(ContentItem::plot)
                .ok_or("Plot missing.")?;
            let active_before = plot
                .stack
                .active
                .and_then(|i| plot.binding.series.get(i))
                .map(|s| s.id);
            let active_after = remaining_plot
                .stack
                .active
                .and_then(|i| remaining_plot.binding.series.get(i))
                .map(|s| s.id);
            remaining.push(json!({"plot":target,"series_ids":remaining_plot.binding.series.iter().map(|s|s.id).collect::<Vec<_>>(),"active_series":{"before":active_before,"after":active_after}}));
        }
        remove_members(&mut page, &contents, &panels);
        page.validate_structure()?;
        let parent_changes = original
            .panels
            .iter()
            .filter_map(|before| {
                page.panel(before.id)
                    .filter(|after| after.item_order != before.item_order)
                    .map(|after| {
                        let target = TargetRef::resource(panel_resource(original, before));
                        modified.push(target.clone());
                        json!({"target":target,"before":before.item_order,"after":after.item_order})
                    })
            })
            .collect::<Vec<_>>();
        let groups = original.groups.iter().filter(|g|!page.groups.iter().any(|after|after == *g)).map(|before|json!({"id":before.id,"before":group_members(original, &before.members),"after":page.groups.iter().find(|g|g.id == before.id).map(|g|group_members(original, &g.members))})).collect::<Vec<_>>();
        let links = original.x_viewport_links.iter().filter(|g|!page.x_viewport_links.contains(g)).map(|before|json!({"id":before.id,"before":before.members,"after":page.x_viewport_links.iter().find(|g|g.id == before.id).map(|g|&g.members)})).collect::<Vec<_>>();
        let ci = self
            .doc
            .canvas_index(original.resource_id)
            .ok_or("Canvas missing.")?;
        let change = DeletionChange::new(self, ci, &page);
        let deleted = params
            .edits
            .iter()
            .filter_map(FigureEdit::deletion_target)
            .cloned()
            .collect::<Vec<_>>();
        let height_before = super::page_fit::auto_height_mm(original);
        let height_after = super::page_fit::auto_height_mm(&page);
        let result = json!({"canvas":params.canvas,"deleted_targets":deleted,"deletions":entries,"remaining_series":remaining,
            "parent_changes":parent_changes,"group_changes":groups,"viewport_link_changes":links,"selection_change":change.selection_value(),
            "retained_asset_ids":retained_assets,"auto_height":if original.auto_height {json!({"before_mm":height_before,"after_mm":height_after})} else {serde_json::Value::Null},
            "created":[],"modified":modified,"deletion_contract":"Only explicit targets; preserve empty parents, source datasets and assets. One atomic Undo/Redo. No sibling reflow."});
        Ok(FigureEditPlan {
            expected_revision: revision,
            canvas: params.canvas.clone(),
            modified,
            created: vec![],
            deleted,
            result,
            candidate: None,
            assets: vec![],
            deletion: Some(Box::new(change)),
        })
    }
}

fn group_members(page: &CanvasDocument, members: &[GroupMember]) -> Vec<serde_json::Value> {
    members.iter().map(|m| match m {
        GroupMember::Content(id) => json!({"kind":"content","target":TargetRef::resource(canvas_object_ref(page.resource_id,*id))}),
        GroupMember::Panel(id) => json!({"kind":"panel","target":page.panel(*id).map(|p|TargetRef::resource(panel_resource(page,p)))}),
    }).collect()
}
