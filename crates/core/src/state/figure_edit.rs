//! Native page authoring transactions, independent of desktop selection.
use super::*;
use crate::actions::{Action, PageSizeState, PanelState};
use crate::automation::{DocumentRevision, TargetRef, canvas_object_ref, panel_resource};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FigureEditParams {
    pub canvas: TargetRef,
    pub edits: Vec<FigureEdit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum FigureEdit {
    Page {
        size_mm: Option<[f32; 2]>,
        margin_mm: Option<[f32; 4]>,
        gutter_mm: Option<f32>,
    },
    PanelFrame {
        target: TargetRef,
        frame_pt: [f32; 4],
    },
    ContentFrame {
        target: TargetRef,
        frame_pt: [f32; 4],
    },
    ImageReplace {
        target: TargetRef,
        path: std::path::PathBuf,
    },
    PanelLabel {
        target: TargetRef,
        text: String,
        visible: bool,
    },
    Align {
        panels: Vec<TargetRef>,
        edge: FigureAlign,
    },
    Distribute {
        panels: Vec<TargetRef>,
        axis: FigureAxis,
    },
    Grid {
        panels: Vec<TargetRef>,
        rows: u32,
        cols: u32,
    },
    TextCreate {
        parent: TargetRef,
        name: String,
        frame_pt: [f32; 4],
        text: String,
        font_pt: f32,
    },
    TextSet {
        target: TargetRef,
        text: String,
        font_pt: Option<f32>,
    },
    ImageCreate {
        parent: TargetRef,
        name: String,
        frame_pt: [f32; 4],
        path: std::path::PathBuf,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FigureAlign {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FigureAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FigureEditPlan {
    pub expected_revision: DocumentRevision,
    pub canvas: TargetRef,
    pub modified: Vec<TargetRef>,
    pub created: Vec<TargetRef>,
    pub result: Value,
    #[serde(skip)]
    candidate: Option<Box<CanvasDocument>>,
    #[serde(skip)]
    assets: Vec<AssetRecord>,
}
impl std::fmt::Debug for FigureEditPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FigureEditPlan").field(&self.result).finish()
    }
}
impl PartialEq for FigureEditPlan {
    fn eq(&self, b: &Self) -> bool {
        self.expected_revision == b.expected_revision
            && self.canvas == b.canvas
            && self.modified == b.modified
            && self.created == b.created
            && self.result == b.result
    }
}
fn text(value: &str, max: usize, nonblank: bool) -> Result<(), String> {
    if value.len() > max || (nonblank && value.trim().is_empty()) {
        return Err(format!(
            "Text must be at most {max} UTF-8 bytes{}.",
            if nonblank { " and nonblank" } else { "" }
        ));
    }
    Ok(())
}
fn font(pt: f32) -> Result<(), String> {
    if !pt.is_finite() || !(1.0..=72.0).contains(&pt) {
        return Err("Font size must be 1–72 pt.".into());
    }
    Ok(())
}
fn frame(pt: [f32; 4]) -> Result<ObjectFrame, String> {
    let f = ObjectFrame {
        x: pt[0],
        y: pt[1],
        width: pt[2],
        height: pt[3],
    };
    validate_frame(f, "authoring")?;
    Ok(f)
}
fn panel_id(page: &CanvasDocument, target: &TargetRef) -> Result<PanelId, String> {
    let p = page
        .panels
        .iter()
        .find(|p| *target == TargetRef::resource(panel_resource(page, p)))
        .ok_or("Discover an exact Panel target on this canvas.")?;
    if p.locked
        || p.layout != PanelLayout::Free
        || p.item_order
            .iter()
            .any(|id| page.object(*id).is_some_and(|o| o.locked))
    {
        return Err("Panel and its contents must be unlocked, with free layout.".into());
    }
    Ok(p.id)
}
fn author_frame(page: &mut CanvasDocument, id: PanelId, f: ObjectFrame) -> Result<(), String> {
    let child = page
        .panel(id)
        .ok_or("Panel missing.")?
        .item_order
        .first()
        .copied();
    if let Some(child) = child {
        if !page.set_layout_frame(child, f) {
            return Err("Authoring frame target disappeared.".into());
        }
    } else {
        page.panel_mut(id).ok_or("Panel missing.")?.frame = f;
    }
    Ok(())
}
fn panel_frames(
    page: &CanvasDocument,
    targets: &[TargetRef],
    min: usize,
) -> Result<Vec<(ObjectId, ObjectFrame)>, String> {
    if targets.len() < min || targets.len() > 200 {
        return Err(format!("Supply {min}–200 distinct Panel targets."));
    }
    let mut seen = BTreeSet::new();
    targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let id = panel_id(page, t)?;
            if !seen.insert(id) {
                return Err("Duplicate Panel target.".into());
            }
            Ok((
                ObjectId::new(i as u64),
                page.panel(id).expect("resolved panel").frame,
            ))
        })
        .collect()
}
fn parent_panel(page: &CanvasDocument, target: &TargetRef) -> Result<Option<PanelId>, String> {
    if *target == TargetRef::resource(page.resource_id.into()) {
        Ok(None)
    } else {
        panel_id(page, target).map(Some)
    }
}
fn insert(
    page: &mut CanvasDocument,
    parent: &TargetRef,
    name: &str,
    f: [f32; 4],
    kind: ContentKind,
) -> Result<TargetRef, String> {
    text(name, 1024, true)?;
    let parent = parent_panel(page, parent)?;
    let frame = frame(f)?;
    let id = page.next_object_id;
    page.next_object_id = id
        .try_advance(1)
        .ok_or("Content identity allocator exhausted.")?;
    page.objects.push(CanvasObject {
        id,
        name: name.into(),
        frame,
        locked: false,
        visible: true,
        kind,
    });
    if let Some(parent) = parent {
        page.panel_mut(parent)
            .expect("resolved parent")
            .item_order
            .push(id);
    }
    Ok(TargetRef::resource(canvas_object_ref(page.resource_id, id)))
}
pub(crate) fn validate_page_geometry(page: &CanvasDocument) -> Result<(), String> {
    if !page
        .size_mm
        .into_iter()
        .all(|v| v.is_finite() && (10.0..=1000.0).contains(&v))
    {
        return Err("Canvas dimensions must be 10–1000 mm.".into());
    }
    let m = page.layout.margin_mm;
    if !m.into_iter().all(|v| v.is_finite() && v >= 0.)
        || m[1] + m[3] >= page.size_mm[0]
        || m[0] + m[2] >= page.size_mm[1]
        || !page.layout.gutter_mm.is_finite()
        || page.layout.gutter_mm < 0.
    {
        return Err("Margins/gutter must be finite, nonnegative, and margins must leave positive page area.".into());
    }
    Ok(())
}
impl PlotiumApp {
    pub fn plan_figure_edit(
        &self,
        revision: DocumentRevision,
        params: &FigureEditParams,
    ) -> Result<FigureEditPlan, String> {
        if revision.0 != self.doc.automation_revision {
            return Err("Document revision is stale.".into());
        }
        if params.edits.is_empty() || params.edits.len() > 200 {
            return Err("Supply 1–200 figure edits.".into());
        }
        let original = self
            .doc
            .canvases
            .iter()
            .find(|c| params.canvas == TargetRef::resource(c.resource_id.into()))
            .ok_or("Discover an exact whole-canvas target.")?;
        if original.auto_height
            && params.edits.iter().any(|e| {
                !matches!(
                    e,
                    FigureEdit::PanelLabel { .. }
                        | FigureEdit::TextSet { .. }
                        | FigureEdit::ImageReplace { .. }
                )
            })
        {
            return Err("Turn off Auto height in the native page controls before exact page/layout authoring.".into());
        }
        let mut page = original.clone();
        let mut assets = Vec::new();
        let mut modified = Vec::new();
        let mut created = Vec::new();
        let mut input_bytes = 0usize;
        let mut image_count = 0;
        let mut content_edits = Vec::new();
        for (edit_index, edit) in params.edits.iter().enumerate() {
            match edit {
                FigureEdit::Page {
                    size_mm,
                    margin_mm,
                    gutter_mm,
                } => {
                    if size_mm.is_none() && margin_mm.is_none() && gutter_mm.is_none() {
                        return Err("Page edit requires a size, margins or gutter.".into());
                    }
                    if let Some(size) = size_mm {
                        let s = PageSizeState::of(&page).after_manual_resize(*size);
                        page.size_mm = s.size_mm;
                        page.size_preset_id = s.preset_id;
                    }
                    if let Some(m) = margin_mm {
                        page.layout.margin_mm = *m;
                    }
                    if let Some(g) = gutter_mm {
                        page.layout.gutter_mm = *g;
                    }
                    modified.push(params.canvas.clone());
                }
                FigureEdit::PanelFrame { target, frame_pt } => {
                    let id = panel_id(&page, target)?;
                    author_frame(&mut page, id, frame(*frame_pt)?)?;
                    modified.push(target.clone());
                }
                FigureEdit::ContentFrame { target, frame_pt } => {
                    content_edits.push(super::figure_edit_content::set_frame(
                        &mut page, target, *frame_pt, edit_index,
                    )?);
                    modified.push(target.clone());
                }
                FigureEdit::ImageReplace { target, path } => {
                    let id = super::figure_edit_content::image_target(&page, target)?;
                    let asset = super::figure_edit_image::freeze_image(
                        &self.doc.assets,
                        &mut assets,
                        path,
                        &mut input_bytes,
                        &mut image_count,
                    )?;
                    content_edits.push(super::figure_edit_content::replace_image(
                        &mut page, id, target, &asset, edit_index,
                    )?);
                    modified.push(target.clone());
                }
                FigureEdit::PanelLabel {
                    target,
                    text: value,
                    visible,
                } => {
                    text(value, 1024, true)?;
                    let id = panel_id(&page, target)?;
                    let p = page.panel_mut(id).expect("resolved panel");
                    p.label.mode = PanelLabelMode::Manual {
                        value: value.clone(),
                    };
                    p.label.visible = *visible;
                    modified.push(target.clone());
                }
                FigureEdit::Align { panels, edge } => {
                    let frames = panel_frames(&page, panels, 2)?;
                    let mode = match edge {
                        FigureAlign::Left => crate::layout::Align::Left,
                        FigureAlign::Center => crate::layout::Align::HCenter,
                        FigureAlign::Right => crate::layout::Align::Right,
                        FigureAlign::Top => crate::layout::Align::Top,
                        FigureAlign::Middle => crate::layout::Align::VCenter,
                        FigureAlign::Bottom => crate::layout::Align::Bottom,
                    };
                    apply_frames(&mut page, panels, crate::layout::align(&frames, mode))?;
                    modified.extend(panels.clone());
                }
                FigureEdit::Distribute { panels, axis } => {
                    let frames = panel_frames(&page, panels, 3)?;
                    let mode = match axis {
                        FigureAxis::Horizontal => crate::layout::Distribute::Horizontal,
                        FigureAxis::Vertical => crate::layout::Distribute::Vertical,
                    };
                    apply_frames(&mut page, panels, crate::layout::distribute(&frames, mode))?;
                    modified.extend(panels.clone());
                }
                FigureEdit::Grid { panels, rows, cols } => {
                    let frames = panel_frames(&page, panels, 1)?;
                    if !(1..=16).contains(rows)
                        || !(1..=16).contains(cols)
                        || (*rows as usize) * (*cols as usize) < panels.len()
                    {
                        return Err(
                            "Grid requires 1–16 rows/columns and enough cells for all targets."
                                .into(),
                        );
                    }
                    validate_page_geometry(&page)?;
                    let mut layout = page.layout;
                    layout.rows = *rows;
                    layout.cols = *cols;
                    let [top, right, bottom, left] = layout.margin_mm;
                    if page.size_mm[0] - left - right - layout.gutter_mm * (*cols - 1) as f32 <= 0.
                        || page.size_mm[1] - top - bottom - layout.gutter_mm * (*rows - 1) as f32
                            <= 0.
                    {
                        return Err("Grid margins/gutters leave no positive cell area.".into());
                    }
                    let cell_width =
                        (page.size_mm[0] - left - right - layout.gutter_mm * (*cols - 1) as f32)
                            * MM_TO_PT
                            / *cols as f32;
                    let cell_height =
                        (page.size_mm[1] - top - bottom - layout.gutter_mm * (*rows - 1) as f32)
                            * MM_TO_PT
                            / *rows as f32;
                    if cell_width < 1.0 || cell_height < 1.0 {
                        return Err("Grid cells must be at least 1 pt, the native minimum.".into());
                    }
                    let ids = frames.iter().map(|f| f.0).collect::<Vec<_>>();
                    let after = crate::layout::assign_grid(page.size_pt(), &layout, &ids);
                    apply_frames(&mut page, panels, after)?;
                    modified.extend(panels.clone());
                }
                FigureEdit::TextCreate {
                    parent,
                    name,
                    frame_pt,
                    text: value,
                    font_pt,
                } => {
                    text(value, 16 * 1024, false)?;
                    font(*font_pt)?;
                    let mut t = TextBox::label(value.clone());
                    t.font_size = *font_pt;
                    created.push(insert(
                        &mut page,
                        parent,
                        name,
                        *frame_pt,
                        ContentKind::Text(t),
                    )?);
                }
                FigureEdit::TextSet {
                    target,
                    text: value,
                    font_pt,
                } => {
                    text(value, 16 * 1024, false)?;
                    let id = page
                        .objects
                        .iter()
                        .find(|o| {
                            *target
                                == TargetRef::resource(canvas_object_ref(page.resource_id, o.id))
                        })
                        .map(|o| o.id)
                        .ok_or("Discover an exact text target on this canvas.")?;
                    if let Some(p) = page.parent_panel(id) {
                        panel_id(
                            &page,
                            &TargetRef::resource(panel_resource(
                                &page,
                                page.panel(p).expect("resolved parent"),
                            )),
                        )?;
                    }
                    let o = page.object_mut(id).expect("resolved text");
                    if o.locked {
                        return Err("Text is locked.".into());
                    }
                    let ContentKind::Text(t) = &mut o.kind else {
                        return Err("Target is not native text.".into());
                    };
                    t.text = value.clone();
                    if let Some(pt) = font_pt {
                        font(*pt)?;
                        t.font_size = *pt;
                    }
                    modified.push(target.clone());
                }
                FigureEdit::ImageCreate {
                    parent,
                    name,
                    frame_pt,
                    path,
                } => {
                    let asset = super::figure_edit_image::freeze_image(
                        &self.doc.assets,
                        &mut assets,
                        path,
                        &mut input_bytes,
                        &mut image_count,
                    )?;
                    let image = RasterImageContent::new(asset.id);
                    created.push(insert(
                        &mut page,
                        parent,
                        name,
                        *frame_pt,
                        ContentKind::RasterImage(image),
                    )?);
                }
            }
        }
        validate_page_geometry(&page)?;
        page.validate_structure()?;
        for o in &page.objects {
            validate_frame(o.frame, "content")?;
        }
        let panel_frames=page.panels.iter().map(|p|json!({"target":TargetRef::resource(panel_resource(&page,p)),"frame_pt":[p.frame.x,p.frame.y,p.frame.width,p.frame.height]})).collect::<Vec<_>>();
        let content_frames = super::figure_edit_content::final_frames(&page, &params.edits)?;
        let result = json!({"canvas":params.canvas,"size_mm":page.size_mm,"margin_mm":page.layout.margin_mm,"gutter_mm":page.layout.gutter_mm,"panel_frames":panel_frames,"content_edits":content_edits,"content_frames":content_frames,"created":created,"modified":modified,"frame_contract":"Panel frames are page-relative pt; resizing proportionally scales child-local frames. Content frames are parent-local pt (page pt for canvas parent); content_frame changes only the specified item. No automatic reflow."});
        Ok(FigureEditPlan {
            expected_revision: revision,
            canvas: params.canvas.clone(),
            modified,
            created,
            result,
            candidate: Some(Box::new(page)),
            assets,
        })
    }
    pub fn commit_figure_edit(&mut self, plan: FigureEditPlan) -> Result<(), String> {
        if plan.expected_revision.0 != self.doc.automation_revision {
            return Err("Document revision is stale.".into());
        }
        let page = plan
            .candidate
            .ok_or("Prepare a new figure edit plan before execution.")?;
        let ci = self
            .doc
            .canvas_index(page.resource_id)
            .ok_or("Canvas missing.")?;
        let before = &self.doc.canvases[ci];
        let mut actions = plan
            .assets
            .into_iter()
            .map(|a| Action::SetAsset {
                id: a.id,
                before: None,
                after: Some(a),
            })
            .collect::<Vec<_>>();
        actions.push(Action::ReplacePanelState {
            canvas: ci,
            before: PanelState::of(before),
            after: PanelState::of(&page),
        });
        actions.push(Action::SetCanvasSize {
            canvas: ci,
            before: PageSizeState::of(before),
            after: PageSizeState::of(&page),
        });
        // Content-only batches do not capture the live grid overlay as a layout change.
        let mut after_layout = page.layout;
        after_layout.show_grid = before.layout.show_grid;
        if after_layout != before.layout {
            actions.push(Action::SetPageLayout {
                canvas: ci,
                before: before.layout,
                after: after_layout,
            });
        }
        self.try_execute_action(Action::Composite(actions))
            .map_err(|e| e.to_string())
    }
}
fn apply_frames(
    page: &mut CanvasDocument,
    targets: &[TargetRef],
    frames: Vec<(ObjectId, ObjectFrame)>,
) -> Result<(), String> {
    for (ordinal, f) in frames {
        let target = &targets[ordinal.get() as usize];
        let id = panel_id(page, target)?;
        validate_frame(f, "panel")?;
        author_frame(page, id, f)?;
    }
    Ok(())
}
