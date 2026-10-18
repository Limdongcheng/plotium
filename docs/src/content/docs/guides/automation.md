---
title: Automation
description: Apply one action to many datasets or figures, or run a saved workflow.
---

When the same operation has to happen to a whole series of experiments,
automation replaces the click-per-dataset routine with one reviewed batch.

Open **File → Automation…** (also on the command palette). The window has two
tabs:

## Current Project

Works on what is already open. Search for datasets or figures and check the
ones you want — or press **Current selection** to pull in your current
selection — then pick a tool and press **Preflight** to see which targets it
will affect and which will be skipped. **Confirm and execute** applies
it, and the whole batch collapses into a single **Undo automation** step.

### Settings

Three tools reach the settings the **Object inspector** and the processing
steps edit, so one level, colour, width, type size or window function can be
applied across a whole project as a single reviewed batch:

| Tool | What it does |
| --- | --- |
| **Inspect a property** | Reads the current value, the default, and the range the setting accepts |
| **Set a property** | Writes a value |
| **Reset a property** | Re-derives the value from the current data, as the panel's reset button does |

Name the setting in **Parameters (JSON)** by its id:
`{"key": "series.contour.count", "value": 12}` for **Set a property**, and
`{"key": "series.contour.count"}` for the other two.

The three tools reach every setting the panels edit: object and series styling,
processing-step parameters, document and canvas settings, and application
preferences. Which resource to name follows from the setting — a contour or
line setting lives on a plot object, an apodization setting on a dataset,
figure typography on the document (listed as **Plotium document**), a page size
on a canvas, and a preference on the application (listed as **Plotium
application**).

| Setting in the Object inspector | id | Accepts |
| --- | --- | --- |
| **Lowest level** | `series.contour.base.magnitude` | a number above 0; what it measures depends on the anchor |
| **Anchor** | `series.contour.base.policy` | `absolute`, `noise_floor`, `background_scale`, `fraction_of_range` |
| **Levels** | `series.contour.count` | 1 to 256 |
| **Level ratio** | `series.contour.ratio` | above 1, at most 10 |
| **Negative contours** | `series.contour.negative.enabled` | `true` or `false` |
| **Positive colour** | `series.contour.positive_color` | `"#rrggbb"` |
| **Negative colour** | `series.contour.negative_color` | `"#rrggbb"` |
| **Line width** | `series.contour.line_width` | 0.05 to 10 pt; default 0.5 |
| **Stroke width**, in the **Line** section | `series.line.stroke_width` | 0.05 to 10 pt; default 0.5 |
| **Tick-label size**, in the **Figure typography** section | `document.figure.typography.tick_pt` | 1 to 72 |

Application preferences take the same three tools. For
`settings.appearance.accent.color`, **Set a property** accepts `"#rrggbb"` and
pins the canvas accent to that colour; **Reset a property** clears it, and the
accent follows the theme again.

| Setting on an apodization step | id | Accepts |
| --- | --- | --- |
| **Window** | `dataset.processing.apodization.kind` | `none`, `cosine_bell`, `exponential`, `gaussian` |
| **LB** | `dataset.processing.apodization.lb_hz` | −10000 to 10000 |
| **GB** | `dataset.processing.apodization.gb_hz` | greater than 0, at most 10000 |

[Contour levels](/guides/contour-levels/) explains what each contour setting
does and which data each anchor needs; [Processing](/guides/processing/) does
the same for the apodization rows.

#### What a checked target expands to

The resource you check is rarely the thing that is written — the setting decides
what inside it is. A plot object expands to its series, a dataset to its
processing steps, and the document to itself. Preflight and the result list show
one row per component, so a plot holding three series produces three rows,
naming the object and the series inside it.

A component the setting does not reach is listed as **Skipped** with the reason,
and the rest still apply: a heatmap drawn under a contour, a zero-fill step when
the setting belongs to apodization, or a **GB** asked of a step whose window is
*Exponential*. A resource with nothing to address at all — a text box, a dataset
with no pipeline — is skipped the same way.

Result rows and the run record name the component beside the resource:

```json
{
  "resource": { "id": "…", "kind": "plotium.dataset" },
  "component": { "kind": "processing_step", "id": 3 }
}
```

Series components read `{"kind": "series", "id": 2}`. Both ids are local to the
resource they travel with, so a step id means nothing next to a different
dataset.

#### Why a target was skipped

Every skipped row carries a sentence written to be read. Rows the write itself
passed over also carry `skip_reason`, a stable token a workflow can branch on
without matching on wording:

| `skip_reason` | Means |
| --- | --- |
| `already_at_value` | The target already held that value, so nothing was written |
| `not_applicable` | The setting does not apply to this target |
| `target_missing` | The address no longer names anything in the document |

Rows already ruled out at preflight carry the sentence alone. They are never
same-value no-ops, so the absence of the token is itself the distinction.

A **Set a property** whose every target already holds the value writes nothing
at all: each target is reported as skipped, the document revision does not move,
and nothing is added to the undo stack.

A value outside the range is refused at **Preflight**, before you confirm, and
names both the value you gave and the limit that rejected it. Past that point
either every listed component takes the value or none does, and what is written
collapses into one **Undo automation** step.

**Inspect a property** returns its readings to whoever ran it: run it as a
workflow step and the values land in the run record, which is also what
[the command line](/reference/cli/) writes to its manifest file. In the window
the numbers appear under **Result value (JSON)**, below the per-component rows:
one reading per component, each with its current value, its default and the
range it accepts.

#### What a reading contains

Each reading names its `target` and carries the current `value`, a
`default_value` where the setting has one, `modified` — whether the value
differs from that default — an `availability`, and the `schema` that bounds it.

A setting the current state does not allow you to write is still read back:
`availability` is `"disabled"` and `disabled_reason` names what has to change
first, such as switching the phase mode to *Manual* before φ0 can be set.

Schemas are tagged by `type`: `bool`, `text`, `int`, `stepped_int`, `float`,
`enum`, or `color`.

- `int` and `stepped_int` carry `min` and `max`, plus a `unit` where the setting
  has one. `stepped_int` also carries the `step` its values must land on — a
  Savitzky-Golay window, for instance, runs from 3 to 201 in steps of 2.
- `float` carries `min`, `max` and `exclusive_min`, and where a setting refuses
  particular values, `excluded` (one value) or `excluded_magnitude` (everything
  at or below that magnitude). Its `display` — `linear`, `degrees` or `log10` —
  is how the panel shows the number; the `unit` and `log` fields beside it
  restate the same thing.
- `enum` lists its variants, each with a stable id and a label.

Bounds and values are always in the setting's own units, whatever `display`
says: a phase whose `display` is `degrees` is radians on the wire.

## External Inputs

Runs a saved workflow that starts from files on disk — for example: import
every experiment in a folder, apply a processing recipe, and export each
figure. Press **Open workflow…** to load it, **Validate** to check it, then
**Confirm and run workflow**. Progress is reported step by step and a long run
can be cancelled.

## What is recorded

A workflow run leaves a record of the workflow and its hash, the Plotium version,
and every step's targets, parameters and results. Run it here and that record is
kept in the project; run it from [the command line](/reference/cli/) and the
same record is written to a file. A batch from **Current Project** leaves no
such record — it is an ordinary edit, in the document and on the undo stack.

Anything that writes files outside the project — exporting figures, for
instance — is also listed under **Help → Operation and Diagnostic History**,
with each file's path, size and SHA-256 checksum. An export that fails partway
still names the files it had already written, so nothing reaches disk
unaccounted for.

Workflow files are plain JSON and can also be run without the desktop app —
see [the command line](/reference/cli/), which executes the same workflows
headlessly. [File formats](/reference/file-formats/) describes the workflow
and run-record files themselves.

## Local agent connections (MCP)

Open **Preferences → Agent connections**. **File → Agent connections…** and
**Agent connections** in the command palette take you to the same settings page.
Press **Start connection**.

### Connect Codex CLI

1. Install and sign in to Codex CLI (`--no-daemon` support is required; verified
   with version 0.163.0).
2. Press **Copy Codex launch command** in Plotium.
3. Paste the command into a terminal and run it as-is: use zsh or bash on macOS
   and Linux, or PowerShell on Windows. It starts a new Codex session with the
   current connection address and token. No manual substitutions or separate
   `export` / `codex mcp add` steps are needed.
4. In Codex, enter `/mcp` and check that `plotium` is connected with eleven
   tools. Try: “Use plotium to inspect the current Plotium project without
   changing it.”

The command supplies temporary startup configuration and a token environment
variable for that invocation. It uses an independent Codex process so an existing
background service cannot retain an old environment. It does not write Codex
configuration files. Run it from the folder you want Codex to work in. After
stopping and restarting the Plotium connection, copy a new command and start a
new Codex session.

### Other MCP clients

**Copy connection information** provides a generic JSON object with the actual
`url` and authentication `headers`. Configure a client that supports HTTP MCP and
custom authentication headers using those values. The shape is:

```json
{
  "url": "http://127.0.0.1:PORT/mcp",
  "headers": { "Authorization": "Bearer TOKEN" }
}
```

The copied object already contains the actual port and token; your client's
surrounding configuration may differ. Plotium does not edit other applications'
settings. Each start chooses a new local port and secret token. Connection
settings and permissions are never saved. Closing Preferences keeps the connection
running. Stop the connection to revoke the token; project transitions and quitting
also stop it. Only local clients can connect. Browser origins must match the
server's own origin; cross-origin access is unavailable.

### Permissions and supported operations

Connections start with **Read and modify** access, so agents can apply supported
edits immediately after connecting. Changes use Plotium's normal **Undo** and
**Redo**. Choose **Read only** under **Access** to prevent further edits, including
pending plans. Table queries and existing edit preflight remain available; import
and native curve binding/creation preflight require Read and modify. The selected access applies
to every client holding that connection token and lasts until the connection is
stopped. A new connection starts with **Read and modify** again.

Real Agent project reads and executions save run records and mark the project as
unsaved; handshakes, tool listing and the static property catalog do not.

The connection supports CSV/TSV data-only import, native table queries, canvas previews,
document structure, native single-plot creation, explicit numeric curve bindings and resource queries, renaming datasets,
canvases and canvas objects, and these display properties only:

| Property ID | Target |
| --- | --- |
| `series.line.stroke_width` | Plot object (its line series) |
| `series.contour.line_width` | Plot object (its contour series) |
| `series.contour.count` | Plot object (its contour series) |
| `document.figure.typography.tick_pt` | `document` |
| `document.figure.typography.label_pt` | `document` |

Processing parameters, preferences, other import formats, computation, exports, built-in chat
and stdio connections are outside this interface.

### Discover structure without relying on names

Start with `plotium_get_state {}`. Its `connection_capabilities` describes this
connection's eleven tools, input schemas, result contracts, supported operations,
property IDs, limits and current execution permission. Standard MCP `tools/list`
also lists their input schemas. Resource descriptors describe **core** data and
rendering capabilities; a capability such as `figure.export` or `data.transform`
does not make that operation callable through this connection. Only
`resource.rename`, `properties.set`, `properties.reset` and the restricted
`data.import` mode, `plot.create` and `plot.series.bind` / `plot.series.materialize_default` below can be planned here. Read-only access allows table reads
and existing property preflight, but refuses import/binding/creation preflight and all edit execution.

Use the returned revision to list the document's canvases:

```json
{
  "expected_revision": 7,
  "target": {"resource":{"id":"document","kind":"plotium.document"}},
  "view": "children",
  "offset": 0,
  "limit": 50
}
```

Call `plotium_document_inspect` with this request. Copy a complete
`value.items[].target` into the next request to descend from canvas to panel,
content, and Series. A canvas lists its panels followed by loose content;
parented content appears only under its panel. `view: "self"`, with pagination
arguments omitted, reads one target's details, `ancestors` and `child_count`.
The content kinds are plot, text, shape and raster image; image pixels and data
rows are not returned. Frames use points: a panel is page-relative, while content
inside it is panel-local. Canvas dimensions are in millimetres.

Series discovery uses the plot's current display binding, like property addressing.
Inactive channel or display-mode alternatives remain stored but are not listed or
addressable until reactivated. Authored hidden Series in the active binding are
still listed with `visible: false`. After switching channels or display modes,
fetch the current revision and discover the active targets again.

A Series detail includes its complete property-compatible target, encoding,
visibility, explicit label, and `source`: dataset and field resource references,
owner-local `field_id`, optional `item_id`, units and binding status. Use the
returned resource IDs with `plotium_resources_inspect` for field/column
descriptors. A missing field retains its `field_id` but has no resolvable field
reference. A source can report `resolved`, `missing_dataset`, `missing_field`,
`missing_item`, `incompatible_encoding`, `unbound` or `missing_column`.

For tables, `binding_mode: "table_default"` reports the shared `x_column` and
all `series_columns` Y/error bindings. One logical Series may expand to several
rendered table traces; those traces do not gain separate Series IDs here.
Explicit sources report `binding_mode: "table_explicit"`, TableId, current table revision,
`x_column`, `y_column` and optional `error_column`, without reporting defaults as their source.
`coordinate_units` reports the plot coordinate contract. Names, including identical curve names, are display
text, never identity. Panels use `CANVAS_UUID/panels/PANEL_ID`; content and Series
retain the existing addresses shown below.

Children queries default to 50 entries, accept 1–200, and return `total`,
`next_offset` and `truncated`. Follow `next_offset` with the **same revision and
target**, including when the 256 KiB structured-response budget shortens a page.
A detail or single entry that exceeds that budget returns `response_too_large`.
There is no recursive whole-project dump. Revision changes reject continuation;
read state and restart the listing. Missing targets and forged identities return
`invalid_parameters`, without using the current selection.

`plotium_properties_list {}` returns exactly the five supported definitions:
`schema`, `unit` (empty for dimensionless values), `scope`, `access` and static
`applicability`. It contains no current target values. Use
`plotium_properties_inspect` to resolve values, defaults, bounds and applicability
for explicit targets. Property applicability and connection write permission
are separate facts.

### Import CSV/TSV data without creating a plot

Ask the agent to import an explicitly named UTF-8 CSV or TSV, inspect its columns,
and read the whole table in pages. Start with `plotium_get_state {}`, then call
`plotium_plan`:

```json
{
  "expected_revision": 7,
  "tool_id": "data.import",
  "parameters": {
    "mode": "table_data_only",
    "paths": ["/absolute/path/measurement.csv"],
    "header": "present"
  }
}
```

Do not supply `ids` or `targets` for import. `header` accepts `auto` (default),
`present` or `absent`; ambiguous auto detection fails. The `.csv` or `.tsv`
extension selects comma or tab; `format: "csv" | "tsv"` overrides it explicitly.
Only named regular files are read. There is no directory discovery, adjacent
sidecar lookup, source writeback, implicit plotting or X/Y binding.

Preflight runs in the background and returns only when preparation finishes.
`import.items` reports each file as `prepared` or `failed`, with proposed native
IDs, metadata and diagnostics. No dataset exists yet. After reviewing the result,
call `plotium_execute {"plan_id":"RETURNED_PLAN_ID"}`. Execution uses those frozen
bytes and typed snapshots, even if a source file changed after preflight. It
rechecks revision and permission before one atomic commit of all prepared items.
`value.status` is `succeeded`, `partial` or `failed`; inspect every item and its
created IDs. Failed files leave no partial dataset. One Undo removes the batch's
successful items; Redo restores the same DatasetId, FieldId, ColumnId and RowId.
Existing GUI/CLI import entry points retain their original behavior.

Metadata includes DatasetId, TableId, table RevisionId, field identities, stable
ColumnIds, logical types, nullable flags, units, total rows and diagnostics.
Fields and columns are distinct native identities. Missing cells remain null;
mixed numeric/text columns preserve text. Integer-looking tokens outside signed
64-bit range remain text rather than being rounded. Units are never inferred
from numbers or header spelling. Column `type_source` is `file`, `user`,
`inferred` or `unknown`; `unit_source` is `file`, `user` or `unknown`.

Optional `schema` is an ordered native TableSchema with typed ColumnSchema IDs,
logical types, nullable flags and optional UnitSpec declarations. Alternatively,
`schema_path` names a v1 PlotiumDelimitedSchema JSON file explicitly. Its header
and order must match exactly; declarations are strict, with no inference fallback
for invalid values. Supported declared types are null, boolean, int64, float64,
utf8, date and timestamp. Uniqueness/business-key constraints and uncertainty
relations are not accepted by this import. Plain `_sigma` column names do not
assign scientific uncertainty meanings. New native IDs are allocated per import;
file/user schema IDs describe the source, not an existing project target.

Limits are 16 files, 16 MiB per file, 64 MiB selected input bytes per batch,
256 columns and 1,000,000 rows per file. Explicit metadata files are limited to
256 KiB. The prepared item summary is limited to 128 KiB and the complete
preflight response to 256 KiB; reduce the batch/schema if either is exceeded.
At most four prepared imports and 32 total plans are retained per
connection; one import preflight runs at a time. Limits bound inputs, not total
process memory. Cancellation, a project transition, permission revocation or an
intervening edit prevents a pending preflight from becoming an executable plan.
Queued or running work is not completion. Failed/stale submissions consume their
plan; fetch state and preflight again instead of replaying a create request.

### Read a complete table in bounded pages

Copy DatasetId and table RevisionId from import metadata or the dataset resource
descriptor. Call `plotium_table_columns` with those IDs and `expected_revision` to
inspect metadata, then `plotium_table_read_rows`:

```json
{
  "expected_revision": 8,
  "dataset_id": "DATASET_UUID",
  "table_revision": "TABLE_REVISION_UUID",
  "offset": 0,
  "limit": 200
}
```

Replace the placeholders with returned IDs. Optional `column_ids` projects
columns by stable ColumnId, in the supplied order; omitted or empty selects all
columns in schema order. Rows follow immutable native snapshot order. The result
includes `column_ids`, `rows: [{row_id, values}]`, `total_rows`, `requested_rows`,
`returned_rows`, `next_offset` and `end`. Values are tagged by `kind`; null has no
value. Int64, time, timestamp and duration values use decimal strings to avoid
JSON number rounding. Date values are days since the Unix epoch; time, timestamp
and duration values use nanoseconds. Non-finite floats are explicit `NaN`,
`+Inf` or `-Inf` strings, not null.

Follow `next_offset` with the same dataset, document revision, table revision
and projection until `end: true` and `next_offset: null`. The default page is
200 rows, maximum 1000 rows and 20000 projected cells; the complete structured
ToolResult is limited to 256 KiB. The byte limit can shorten a page, never a cell;
use the actual returned count and continuation. A single oversized row returns
`response_too_large`. An offset equal to total_rows returns an empty end page;
an offset beyond it fails. Invalid limits, duplicate/absent columns and oversized
requests fail rather than being silently clamped.

Both revisions must still match on every read. Human edits, including Undo/Redo,
invalidate old document revisions: read state and restart rather than mixing
pages. Invalid IDs never use the current selection. Read-only connections can
query existing tables but cannot import them. Limits and the import permission
condition appear in `connection_capabilities`.

### Rebind an existing curve without changing another plot

Two plots can use the same native table with different numeric X, Y and optional
symmetric Y-error columns. In the GUI, select a plot and expand **Data → Curve
columns** for its Series. Choose the columns and press **Apply curve binding**.
The table's shared default bindings are unchanged.

For MCP, discover the exact Series target with `plotium_document_inspect` and
copy the DatasetId, TableId, FieldId, table revision and ColumnIds from
`plotium_table_columns`. Use project ColumnIds, not `source_column_id`, names,
trace positions or a guessed field resource address. Call `plotium_plan`:

```json
{
  "expected_revision": 8,
  "tool_id": "plot.series.bind",
  "parameters": {
    "edits": [{
      "target": {
        "resource": {"id":"CANVAS_UUID/OBJECT_ID","kind":"plotium.canvas.object",
          "parent_id":"CANVAS_UUID","local_id":"OBJECT_ID"},
        "component": {"kind":"series","id":0}
      },
      "source": {
        "dataset_id":"DATASET_UUID", "field_id":0,
        "table_id":"TABLE_UUID", "table_revision":"TABLE_REVISION_UUID",
        "x_column_id":"X_COLUMN_UUID", "y_column_id":"Y_COLUMN_UUID",
        "y_error_column_id":null
      }
    }]
  }
}
```

Replace every placeholder, including the numeric IDs, with discovered values.
Do not add top-level `ids` or `targets`: each edit carries its complete target.
`y_error_column_id` is a ColumnId or explicit null to remove the error binding.
Review `preflight.binding_plan.bindings`, including source mappings and row
diagnostics, then submit its one-use `plan_id` with `plotium_execute`.

A default table Series can represent several displayed curves. First use
`plot.series.materialize_default` with
`parameters: {"edits":[{"target":EXACT_SERIES_TARGET,
"retain_y_column_id":"EXISTING_Y_COLUMN_UUID",
"table_revision":"TABLE_REVISION_UUID"}]}`. In the GUI, use **Expand default
curves** and choose the Y column that keeps the original SeriesId. The other
existing curves receive new owner-local SeriesIds, returned in the plan and
execution result. This expands existing content; it does not create a plot.
Fit overlays and normalized/offset multi-curve defaults cannot be expanded
losslessly and are rejected. Default drawing remains available without expansion.

Only Int64 and Float64 columns are accepted. Integers that cannot be represented
exactly in f64 curve coordinates are rejected. X/Y nulls and non-finite values
create gaps, never zero values or connecting lines across a gap. A null error
keeps the valid point without its error bar; zero error is valid. Negative or
non-finite errors reject the entire batch, including on rows with missing X/Y.
Errors are non-negative absolute amplitudes; their names do not assign standard
deviation, standard error or confidence-interval meaning. Diagnostics count all
rows and include up to 20 affected RowIds per curve. No row sampling is used for
binding validation or native curve extraction.

Compatible units are converted into the plot's persisted coordinate units.
Error amplitudes use scale only, not affine offsets. Known and unknown units
cannot be combined. Manual ranges and shifts retain their coordinate meaning
through rebinding, deletion of the first curve and reordering. An incompatible
binding is rejected rather than reinterpreting those numbers. Authored labels,
colors, line/point encoding, visibility, panel placement and axis settings stay
with their existing targets. Explicit plots retain line/point encodings rather
than switching to unrelated table chart types.

A batch accepts 1–200 edits and at most 2,000,000 curve-row visits across the
affected plots. Pending MCP curve plans share that same row budget; complete
responses are limited to 256 KiB. All targets, columns, units, rows and revisions
are checked before one atomic commit and one Undo. There is no partial success
or skipped invalid target. Read-only connections reject binding preflight and
execution. Failed or stale execution consumes the plan; discover current state
and plan again. GUI save/reopen preserves the native bindings and units; MCP
save and export are not exposed; use `plot.create` below to create a new plot.

### Create a native single plot from an imported table

Ask the agent: “Import this CSV, use time in s and current/error in mA, and create
one editable line plot with symmetric Y error bars. Preview it, then change only
that curve's line width to 1.5 pt and preview again.” Supply an absolute input path
and explicit units. The agent can discover the complete import and creation
schemas through `plotium_get_state` and the `plotium_plan` tool schema.

After data-only import, read the actual DatasetId, FieldId, TableId, table revision
and ColumnIds from import results / table metadata. Plan `plot.create` without
top-level `ids` or `targets`:

```json
{
  "expected_revision": 1,
  "tool_id": "plot.create",
  "parameters": {
    "parent": {"resource": {"id": "document", "kind": "plotium.document"}},
    "name": "Current",
    "series": [{
      "kind": "line",
      "source": {
        "dataset_id": "RETURNED_DATASET_UUID",
        "field_id": 0,
        "table_id": "RETURNED_TABLE_UUID",
        "table_revision": "RETURNED_TABLE_REVISION_UUID",
        "x_column_id": "RETURNED_TIME_COLUMN_UUID",
        "y_column_id": "RETURNED_CURRENT_COLUMN_UUID",
        "y_error_column_id": "RETURNED_ERROR_COLUMN_UUID"
      }
    }]
  }
}
```

Replace every identity, including the example FieldId, with the actual returned
identity. `kind` is `line` or `scatter`; explicitly set `y_error_column_id` to a
ColumnId or `null`. Multiple entries create independent native Series in this
one plot, not separate plots. Source extraction and all numeric, missing-value,
error and unit rules are the same as explicit curve binding above. Table-wide
X/Y defaults and other plots are untouched; no source table is copied.

The exact `parent` determines which objects are new:

- Document root: create canvas, single panel and plot. Optional
  `canvas: {name, size_mm: [width, height]}` sets the new canvas (10–1000 mm per
  dimension); omission freezes the native default size and plot name.
- Existing canvas without a panel or plot: create panel and plot using native
  GUI empty-page placement and the canvas's existing layout settings/margins.
  Do not supply `canvas` options.
- Its sole empty, visible, unlocked free-layout panel: create only the plot,
  preserving the panel's page position and size. Omit `canvas` and `panel_name`.

`panel_name` optionally names a newly created panel; otherwise it uses the plot
name. Names must be nonblank and at most 1024 UTF-8 bytes. This operation cannot
create a second panel/plot on an existing plotted canvas or rebuild existing
content. Placement is a native service decision, not an agent-supplied grid,
whitespace or collision algorithm.

`preflight.binding_plan.creation` returns proposed complete targets in
`canvas`, `panel`, `plot` and `created`, plus `size_mm`, `panel_frame_pt`
(page coordinates), `frame_pt` (panel-local content coordinates),
`coordinate_units` and `series_defaults` (kind and line width in pt).
`binding_plan.bindings` returns each exact owner+SeriesId target, actual source
mapping/revision and full-row diagnostics. Preflight creates nothing. Execute
the one-use plan; `ToolResult.value` returns the same identities and native
binding summary, with committed `after_revision` on the result.

The creation is atomic: a bad source, incompatible units, invalid error or
changed parent/revision leaves no new canvas, panel or partial plot. Read-only
connections reject creation preflight and execution. There are 1–200 curves per
plot, at most 2,000,000 curve-row visits, a shared 2,000,000-row pending-plan
budget with binding edits, 32 pending plans and a 256 KiB structured-response
limit. Failed submissions consume their plan too. After a lost execute response,
query current state and the proposed targets before deciding what to do; do not
blindly make a new creation request.

Use returned targets with document inspection and properties; preview the
returned whole-canvas target at the current revision. The result is selectable
and editable in the GUI. One Undo removes the created subtree but keeps existing
parents and imported data; Redo restores the same IDs. Existing native resize
and layout controls retain plot/Series identities and source bindings. Fetch a
fresh revision after any edit or Undo/Redo.

### Read the actual canvas image

Ask the agent: “Preview this canvas and describe its text, curves and local
images.” Use `plotium_document_inspect` to obtain a complete canvas target, then
call `plotium_render_preview` with that target and the current revision:

```json
{
  "expected_revision": 7,
  "target": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "max_edge": 1440
}
```

Replace `CANVAS_UUID` with the discovered canvas ID; do not pass a panel,
content item, selection or Series. The response contains a standard MCP PNG
**image content block**, plus `ToolResult.value` metadata: `target`,
`render_revision`, `width_px`, `height_px` and `mime_type`. It does not require
opening a file or rendering SVG text. The MCP client must deliver image content
to its agent; a text-only client cannot consume this preview.

The image covers the whole page with its native layout, background, panel clips,
text, curves and embedded local raster assets. Rendering runs off the desktop
thread from an immutable snapshot. `render_revision` identifies that snapshot;
human edits after capture do not change it. Read state again before editing.
Read-only connections permit previews. Completed reads save a metadata-only run
record and mark the project unsaved without advancing its revision.

`max_edge` defaults to 1440 px and accepts 1–2048; aspect ratio is preserved
with pixel rounding and a minimum of one pixel per axis. Limits are 4,194,304
output pixels, 16 MiB of referenced encoded assets, 64 MiB of header-probed
RGBA8 asset pages, 8 MiB of intermediate SVG, 4 MiB of PNG, and 6 MiB of JSON-RPC
preview response (including the request ID). Shared asset bytes count once;
decoded pages count once per asset/page. These are payload and image-buffer limits, not a total process
memory limit. All referenced assets are validated, including hidden items.
Limits are also available in `connection_capabilities.limits.image_preview`.

Missing or damaged assets and unavailable image pages return
`preview_asset_unavailable`; no placeholder or incomplete success image is
returned. Budget overflow returns `preview_budget_exceeded`, without silent
quality reduction; lower `max_edge` for output-size overflow, or reduce source
assets for asset-budget overflow. Invalid full identities return
`invalid_parameters`, and outdated revisions return `stale_revision`.
Only one preview renders per connection; another returns `busy`. Other existing
tools remain available while rendering. Cancellation or stopping the connection
prevents image delivery; stopping or switching projects discards the old job.

### Inspect and edit an exact curve

An example agent task is: “Find my line plot, inspect its stroke width, preflight
changing it to 1.5 pt, then apply the saved plan.” The tool sequence is:

1. `plotium_get_state` with `{}` returns `revision`, project overview, selection
   and connection permissions.
2. `plotium_resources_search` with `expected_revision` and a `query` such as
   `{"kinds":["plotium.canvas.object"],"name_contains":"Plot"}` locates resources.
   Resolve results to explicit stable IDs; `plotium_resources_inspect` reads
   descriptors using `ids` and `expected_revision`.
3. `plotium_properties_list` with `{}` lists supported IDs and scopes. Discover
   curve addresses with `plotium_properties_inspect`:

   ```json
   {"ids":["CANVAS_UUID/OBJECT_ID"],"key":"series.line.stroke_width","expected_revision":7}
   ```

   The response's `value.readings` contains one row per applicable curve. Copy
   the complete `value.readings[0].target` for the curve you want, for example:

   ```json
   {
     "resource": {
       "id":"CANVAS_UUID/OBJECT_ID",
       "kind":"plotium.canvas.object",
       "parent_id":"CANVAS_UUID",
       "local_id":"OBJECT_ID"
     },
     "component":{"kind":"series","id":0}
   }
   ```

4. Pass that target unchanged to `plotium_plan` to edit exactly that curve:

   ```json
   {
     "tool_id":"properties.set",
     "parameters":{"key":"series.line.stroke_width","value":1.5},
     "targets":[{
       "resource":{
         "id":"CANVAS_UUID/OBJECT_ID",
         "kind":"plotium.canvas.object",
         "parent_id":"CANVAS_UUID",
         "local_id":"OBJECT_ID"
       },
       "component":{"kind":"series","id":0}
     }],
     "expected_revision":7
   }
   ```

5. Review `preflight`, then call `plotium_execute` with only
   `{"plan_id":"RETURNED_PLAN_ID"}`. Plotium immediately displays the change;
   **Undo** restores it and **Redo** reapplies it. Repeating the current value is
   reported as skipped. A batch applies as one undo step.

Property inspection and property-edit planning require exactly one nonempty `ids` or
`targets` array. `ids` expands each resource to all components addressed by the
property; `targets` addresses only the supplied Series components. Series IDs
are local to their plot, so always copy the full target, including its resource.
Duplicate targets are processed once. A missing component or forged resource
identity rejects the entire request; an existing curve to which the property
does not apply is skipped. Inspection returns values, defaults, bounds and
applicability. To inspect just one curve, use `targets` in the inspection request.
To reset it, plan `properties.reset` with `{"key":"series.line.stroke_width"}`
and the same target. Component addressing supports only the five listed display
properties; processing parameters are unavailable. `resource.rename` requires
`ids`.

Tool responses include structured content and its JSON text representation.
Project reads and preflight require the current `expected_revision`; fetch the
state again after edits or Undo. Execution rechecks the saved revision and current
permission. Plans are single-use, including failed submissions, and at most 32
can be pending. Restarting the connection clears them. Requests cancelled before
desktop execution are skipped without changing the project. Cancellation cannot
roll back an edit that has already started its atomic commit; use **Undo**.

Errors include stable codes: `permission_denied`, `expired_plan`,
`stale_revision`, `busy`, `invalid_parameters`, `unsupported_operation`,
`plan_limit`, `request_cancelled`, `connection_stopped`, `response_too_large` and `execution_failed`.
A `busy` response is retryable after the active edit, recovery confirmation or background project
operation finishes. For stale or expired plans, read state and preflight again.
The Agent connections settings page shows recent operations and errors without
opening itself in response to background activity.
