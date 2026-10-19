//! Shared reversible deletion state. Native callers choose their existing parent policy.
use super::{Action, PanelState};
use crate::state::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DeleteSelection {
    active: Option<CanvasId>,
    selected: Option<ObjectId>,
    objects: Selection,
    hierarchy: HierarchicalSelection,
    layer: Option<ObjectId>,
    lead: Option<ObjectId>,
    label: Option<(CanvasId, ObjectId)>,
}
impl DeleteSelection {
    fn capture(app: &PlotiumApp, page: &CanvasDocument) -> Self {
        let ui = &app.session.ui;
        Self {
            active: app
                .session
                .active_canvas
                .and_then(|i| app.doc.canvases.get(i))
                .map(|c| c.resource_id),
            selected: page.selected_object,
            objects: ui.selection.clone(),
            hierarchy: ui.hierarchical_selection.clone(),
            layer: ui.selection_anchors.layer,
            lead: ui.selection_anchors.layer_lead,
            label: ui
                .panel_label_selection
                .and_then(|(ci, id)| app.doc.canvases.get(ci).map(|c| (c.resource_id, id))),
        }
    }
    fn filtered(&self, page: &CanvasDocument) -> Self {
        self.filtered_members(page.resource_id, &page.objects, &page.panels)
    }
    fn filtered_members(
        &self,
        canvas: CanvasId,
        objects: &[ContentItem],
        panels: &[Panel],
    ) -> Self {
        let exists = |id| objects.iter().any(|o| o.id == id);
        let panel_exists = |id| panels.iter().any(|p| p.id == id);
        let mut after = self.clone();
        after.selected = self.selected.filter(|id| exists(*id));
        if self.active == Some(canvas) {
            let ids: Vec<_> = self
                .objects
                .objects()
                .iter()
                .copied()
                .filter(|id| exists(*id))
                .collect();
            after.objects = if ids.is_empty() {
                Selection::None
            } else {
                Selection::Objects(ids)
            };
            after.layer = self.layer.filter(|id| exists(*id));
            after.lead = self.lead.filter(|id| exists(*id));
        }
        after.hierarchy = HierarchicalSelection::default();
        for path in self.hierarchy.paths() {
            if path.canvas != canvas
                || (path.panel.is_none_or(panel_exists) && path.content.is_none_or(exists))
            {
                after
                    .hierarchy
                    .extend_sibling(*path)
                    .expect("filtering preserves sibling scope");
            }
        }
        after.label = self
            .label
            .filter(|(owner, id)| *owner != canvas || exists(*id));
        after
    }
    fn value(&self) -> Value {
        json!({"active_canvas":self.active,"canvas_selected_object":self.selected,"objects":self.objects.objects(),
            "hierarchy":self.hierarchy.paths().iter().map(|p|json!({"canvas":p.canvas,"panel":p.panel,"content":p.content})).collect::<Vec<_>>(),
            "layer_anchor":self.layer,"layer_lead":self.lead,"panel_label":self.label})
    }
    fn change_value(&self, other: &Self) -> Value {
        let before = self.value();
        let after = other.value();
        let mut left = serde_json::Map::new();
        let mut right = serde_json::Map::new();
        for (key, value) in before.as_object().expect("selection object") {
            if *value != after[key] {
                left.insert(key.clone(), value.clone());
                right.insert(key.clone(), after[key].clone());
            }
        }
        if self.objects != other.objects
            || self.hierarchy != other.hierarchy
            || self.layer != other.layer
            || self.lead != other.lead
        {
            left.insert("active_canvas".into(), json!(self.active));
            right.insert("active_canvas".into(), json!(other.active));
        }
        json!({"before":left,"after":right})
    }
    fn restore(&self, other: &Self, app: &mut PlotiumApp, ci: usize) {
        if self.selected != other.selected {
            app.doc.canvases[ci].selected_object = self.selected;
        }
        if self.objects != other.objects
            || self.hierarchy != other.hierarchy
            || self.layer != other.layer
            || self.lead != other.lead
        {
            app.session.active_canvas = self.active.and_then(|id| app.doc.canvas_index(id));
        }
        let label = self
            .label
            .and_then(|(canvas, id)| app.doc.canvas_index(canvas).map(|ci| (ci, id)));
        let ui = &mut app.session.ui;
        if self.objects != other.objects {
            ui.selection = self.objects.clone();
        }
        if self.hierarchy != other.hierarchy {
            ui.hierarchical_selection = self.hierarchy.clone();
        }
        if self.layer != other.layer {
            ui.selection_anchors.layer = self.layer;
        }
        if self.lead != other.lead {
            ui.selection_anchors.layer_lead = self.lead;
        }
        if self.label != other.label {
            ui.panel_label_selection = label;
        }
    }
}

#[derive(Clone)]
pub struct DeletionChange {
    pub(crate) canvas: CanvasId,
    pub(crate) before: PanelState,
    pub(crate) after: PanelState,
    before_links: Vec<XViewportLinkGroup>,
    after_links: Vec<XViewportLinkGroup>,
    selection: Option<(DeleteSelection, DeleteSelection)>,
}
impl DeletionChange {
    pub(crate) fn new(app: &PlotiumApp, ci: usize, page: &CanvasDocument) -> Self {
        let before = &app.doc.canvases[ci];
        let selection = DeleteSelection::capture(app, before);
        let after_selection = selection.filtered(page);
        Self {
            canvas: before.resource_id,
            before: PanelState::of(before),
            after: PanelState::of(page),
            before_links: before.x_viewport_links.clone(),
            after_links: page.x_viewport_links.clone(),
            selection: (selection != after_selection).then_some((selection, after_selection)),
        }
    }
    pub(crate) fn selection_value(&self) -> Value {
        self.selection
            .as_ref()
            .map_or(Value::Null, |(before, after)| before.change_value(after))
    }
    /// Selection does not advance the document revision. Recheck only a deletion's
    /// actual selection effect; unrelated live selection must not be overwritten.
    fn current_selection(
        &self,
        app: &PlotiumApp,
    ) -> Result<Option<(DeleteSelection, DeleteSelection)>, String> {
        let ci = app
            .doc
            .canvas_index(self.canvas)
            .ok_or("Deletion canvas disappeared.")?;
        let before = DeleteSelection::capture(app, &app.doc.canvases[ci]);
        let after = before.filtered_members(self.canvas, &self.after.objects, &self.after.panels);
        let value = if before == after {
            Value::Null
        } else {
            before.change_value(&after)
        };
        if self.selection_value() != value {
            return Err("Deletion selection effect changed; preflight again.".into());
        }
        Ok((before != after).then_some((before, after)))
    }
    pub(crate) fn check_selection(&self, app: &PlotiumApp) -> Result<(), String> {
        self.current_selection(app).map(|_| ())
    }
    pub(crate) fn recheck_selection(&mut self, app: &PlotiumApp) -> Result<(), String> {
        self.selection = self.current_selection(app)?;
        Ok(())
    }
    pub(crate) fn retain_allocators(&mut self, page: &CanvasDocument) {
        for state in [&mut self.before, &mut self.after] {
            state.next_object_id = state.next_object_id.max(page.next_object_id);
            state.next_group_id = state.next_group_id.max(page.next_group_id);
            state.next_panel_label_slot =
                state.next_panel_label_slot.max(page.next_panel_label_slot);
            for item in &mut state.objects {
                let id = item.id;
                if let (Some(saved), Some(live)) =
                    (item.plot_mut(), page.object(id).and_then(ContentItem::plot))
                {
                    saved.next_series_id = saved.next_series_id.max(live.next_series_id);
                }
            }
        }
    }
    pub(crate) fn validate(&self, app: &PlotiumApp) -> Result<(), String> {
        let ci = app
            .doc
            .canvas_index(self.canvas)
            .ok_or("Deletion canvas disappeared.")?;
        for (state, links) in [
            (&self.before, &self.before_links),
            (&self.after, &self.after_links),
        ] {
            let mut page = app.doc.canvases[ci].clone();
            page.objects.clone_from(&state.objects);
            page.panels.clone_from(&state.panels);
            page.groups.clone_from(&state.groups);
            page.x_viewport_links.clone_from(links);
            page.validate_structure()?;
        }
        Ok(())
    }
    pub(crate) fn restore_selection(&self, app: &mut PlotiumApp, ci: usize, undo: bool) {
        if let Some((before, after)) = &self.selection {
            if undo {
                before.restore(after, app, ci);
            } else {
                after.restore(before, app, ci);
            }
        }
        let ui = &mut app.session.ui;
        let exists = |id| app.doc.canvases[ci].object(id).is_some();
        if ui
            .text_edit
            .as_ref()
            .is_some_and(|e| e.canvas == ci && !exists(e.object))
        {
            ui.text_edit = None;
        }
        if ui
            .panel_note_edit
            .as_ref()
            .is_some_and(|e| e.canvas == ci && !exists(e.object))
        {
            ui.panel_note_edit = None;
        }
        if ui
            .panel_note_inline_edit
            .as_ref()
            .is_some_and(|e| e.canvas == ci && !exists(e.object))
        {
            ui.panel_note_inline_edit = None;
        }
        if ui
            .axis_overrides_before
            .as_ref()
            .is_some_and(|(c, id, _)| *c == ci && !exists(*id))
        {
            ui.axis_overrides_before = None;
        }
        if ui.layers_drag_content.is_some_and(|id| !exists(id))
            && app.session.active_canvas == Some(ci)
        {
            ui.layers_drag_content = None;
        }
        if ui.panel_drop_target.is_some_and(|id| {
            self.before.panels.iter().any(|p| p.id == id)
                && app.doc.canvases[ci].panel(id).is_none()
        }) {
            ui.panel_drop_target = None;
        }
    }
    pub(crate) fn links(&self, undo: bool) -> &[XViewportLinkGroup] {
        if undo {
            &self.before_links
        } else {
            &self.after_links
        }
    }
}

/// Remove only membership made invalid by the explicit removals. Never reflow siblings.
pub(crate) fn remove_members(
    page: &mut CanvasDocument,
    contents: &BTreeSet<ContentId>,
    panels: &BTreeSet<PanelId>,
) {
    page.objects.retain(|o| !contents.contains(&o.id));
    page.panels.retain(|p| !panels.contains(&p.id));
    for panel in &mut page.panels {
        panel.item_order.retain(|id| !contents.contains(id));
    }
    page.groups.retain_mut(|g| {
        g.members.retain(|m| match m {
            GroupMember::Content(id) => !contents.contains(id),
            GroupMember::Panel(id) => !panels.contains(id),
        });
        g.members.len() >= 2
    });
    page.x_viewport_links.retain_mut(|g| {
        g.members.retain(|id| !contents.contains(id));
        g.members.len() >= 2
    });
}

impl Action {
    pub(super) fn capture_object_deletion(
        app: &PlotiumApp,
        canvas: usize,
        id: ObjectId,
    ) -> Option<Self> {
        let c = app.doc.canvases.get(canvas)?;
        c.object(id)?;
        let mut page = c.clone();
        let panels = c
            .parent_panel(id)
            .filter(|p| c.panel(*p).is_some_and(|p| p.item_order.len() == 1))
            .into_iter()
            .collect();
        super::deletion::remove_members(
            &mut page,
            &std::collections::BTreeSet::from([id]),
            &panels,
        );
        Some(Self::delete_candidate(app, canvas, &page))
    }

    pub(crate) fn delete_candidate(app: &PlotiumApp, ci: usize, page: &CanvasDocument) -> Self {
        Self::DeleteFigureTargets {
            change: Box::new(DeletionChange::new(app, ci, page)),
        }
    }
}
