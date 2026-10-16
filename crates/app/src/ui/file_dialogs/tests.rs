use super::*;
use std::path::PathBuf;

fn origin_9_initial_structure() -> Vec<u8> {
    const HEADER_LEN: usize = 115;
    let mut bytes = b"CPYA 4.3268 195 W64 #\n".to_vec();
    bytes.extend_from_slice(&(HEADER_LEN as u32).to_le_bytes());
    bytes.push(b'\n');
    let mut header = [0_u8; HEADER_LEN];
    header[0x1b..0x23].copy_from_slice(&9.510195_f64.to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.push(b'\n');
    bytes.extend_from_slice(&[0, 0, 0, 0, b'\n']);
    bytes
}

#[test]
fn table_import_preview_uses_unique_scroll_area_ids() {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    import_delimited_text(
        &mut app,
        "time,value\n0,1\n",
        DelimitedTableSource::Clipboard,
    );
    let ctx = crate::typography::test_context();

    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        table_import_preview_window(&mut app, ui.ctx());
    });
    let painted = format!("{:?}", output.shapes);

    assert!(!painted.contains("use of ScrollArea ID"), "{painted}");
}

#[test]
fn recent_path_classification_reads_enough_bytes_for_origin_9() {
    let path = std::env::temp_dir().join(format!(
        "plotium-origin-9-probe-{}.opj",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&path, origin_9_initial_structure()).unwrap();

    let classified = recent::classify_open_path(&path);

    std::fs::remove_file(path).unwrap();
    assert_eq!(classified.unwrap().kind(), RecentOpenKind::OriginProject);
}

#[test]
fn mixed_columns_import_as_typed_text_without_being_discarded() {
    let mut app = PlotiumApp::new();
    let input = "x,mixed,note\n0,1,ok\n1,oops,review\n";
    import_delimited_text(&mut app, input, DelimitedTableSource::Clipboard);
    assert!(app.doc.datasets.is_empty());
    assert!(commit_table_import_preview(&mut app));
    let table = app.doc.datasets[0].as_table().unwrap();
    assert!(table.series_bindings.is_empty());
    assert_eq!(table.import_sources[0].bytes(), input.as_bytes());
    let typed = &table.typed_state;
    assert_eq!(typed.envelope.revision.snapshot.schema.columns.len(), 3);
    assert_eq!(
        typed.envelope.revision.snapshot.schema.columns[1].logical_type,
        plotium_core::data::LogicalType::Utf8
    );
    let codecs = plotium_core::data::CodecRegistry::with_arrow_ipc();
    let batch = plotium_core::data::SnapshotReader::new(
        &typed.envelope.revision.snapshot,
        typed.store.as_ref(),
        &codecs,
    )
    .unwrap()
    .read_batch(0, &[])
    .unwrap();
    assert_eq!(
        batch.columns[1].1.value(1),
        Some(plotium_core::data::ScalarValue::Utf8("oops".into()))
    );

    let original_fingerprint = typed.envelope.revision.snapshot.fingerprint;
    let delta = {
        let table = app.doc.datasets[0].as_table().unwrap();
        let preview = table.typed_rows(1, &[]).unwrap();
        let row = preview.row_ids[0];
        let column = preview.columns[0].schema.id;
        let mut delta = plotium_core::state::TableEditDelta::new_dataset(table);
        delta.record_typed_value(
            row,
            column,
            plotium_core::data::LiteralValue::Int64(0),
            plotium_core::data::LiteralValue::Int64(5),
        );
        delta.finish_dataset(table);
        assert!(delta.typed_diagnostic.is_none());
        delta
    };
    app.execute_action(plotium_core::actions::Action::edit_table(
        app.doc.datasets[0].resource_id(),
        delta,
    ));
    let edited_fingerprint = app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .revision
        .snapshot
        .fingerprint;
    assert_ne!(edited_fingerprint, original_fingerprint);
    app.undo();
    assert_eq!(typed_fingerprint(&app), original_fingerprint);
    app.redo();
    assert_eq!(typed_fingerprint(&app), edited_fingerprint);

    let path = std::env::temp_dir().join(format!(
        "plotium-mixed-typed-import-{}.plotium",
        uuid::Uuid::new_v4()
    ));
    plotium_core::project::save_project(&app, &path, false).unwrap();
    let loaded = plotium_core::project::load_project(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    let loaded = loaded.doc.datasets[0].as_table().unwrap();
    assert_eq!(loaded.import_sources[0].bytes(), input.as_bytes());
    assert_eq!(
        loaded
            .typed_state
            .envelope
            .revision
            .snapshot
            .schema
            .columns
            .len(),
        3
    );
}

fn typed_fingerprint(app: &PlotiumApp) -> plotium_core::data::ContentHash {
    app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .revision
        .snapshot
        .fingerprint
}

#[test]
fn clipboard_schema_restores_typed_contract_and_is_retained() {
    let id = plotium_core::data::ColumnId::new();
    let mut value =
        plotium_core::data::ColumnSchema::new("count", plotium_core::data::LogicalType::Int64);
    value.id = id;
    let label =
        plotium_core::data::ColumnSchema::new("label", plotium_core::data::LogicalType::Utf8);
    let contract = plotium_core::xlsx::PlotiumDelimitedSchemaV1 {
        schema_version: 1,
        table_id: plotium_core::data::TableId::new(),
        schema: plotium_core::data::TableSchema::new(vec![value, label]).unwrap(),
        uncertainty: Vec::new(),
    };
    let schema_json = serde_json::to_string(&contract).unwrap();
    let mut app = PlotiumApp::new();
    import_delimited_text_with_schema(
        &mut app,
        "count,label\n9007199254740993,exact\n",
        DelimitedTableSource::Clipboard,
        Some(&schema_json),
    );
    assert!(app.doc.datasets.is_empty());
    assert!(commit_table_import_preview(&mut app));

    let table = app.doc.datasets[0].as_table().unwrap();
    let snapshot = &table.typed_state.envelope.revision.snapshot;
    assert_eq!(snapshot.schema.columns[0].id, id);
    assert_eq!(
        snapshot.schema.columns[0].logical_type,
        plotium_core::data::LogicalType::Int64
    );
    assert_eq!(table.import_sources.len(), 2);
    assert_eq!(table.import_sources[1].bytes(), schema_json.as_bytes());
}

/// Every recent entry must reopen through the loader that imported it; a
/// delimited table routed to the acquisition loader fails on every click.
#[test]
fn recent_entries_route_to_their_import_path() {
    let file = |name: &str| PathBuf::from(format!("C:/data/{name}"));
    let regular = |path: &std::path::Path| {
        recent::classify_open_path_with_header(path, recent::OpenPathEntryType::RegularFile, || {
            Ok(([0_u8; recent::OPEN_HEADER_BYTES], 0))
        })
        .unwrap()
    };
    assert_eq!(regular(&file("session.PLOTIUM")), RecentOpenKind::Project);
    assert_eq!(
        regular(&file("results.csv")),
        RecentOpenKind::DelimitedTable
    );
    assert_eq!(
        regular(&file("results.tsv")),
        RecentOpenKind::DelimitedTable
    );
    assert_eq!(
        regular(&file("results.txt")),
        RecentOpenKind::DelimitedTable
    );
    assert_eq!(regular(&file("results.XLSX")), RecentOpenKind::XlsxTable);
    assert_eq!(regular(&file("run.abf")), RecentOpenKind::DataFile);
    assert_eq!(regular(&file("fid")), RecentOpenKind::DataFile);
    assert_eq!(
        format!("{:?}", regular(&file("project.opj"))),
        "OriginProject"
    );
    assert_eq!(
        format!("{:?}", regular(&file("project.OPJU"))),
        "OriginProject"
    );
    assert_eq!(
        recent::classify_open_path_with_header(
            &std::env::temp_dir(),
            recent::OpenPathEntryType::Directory,
            || panic!("directories must not be opened for header reads"),
        )
        .unwrap(),
        RecentOpenKind::Folder
    );

    let root = std::env::temp_dir().join(format!(
        "plotium-recent-open-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    ));
    std::fs::create_dir(&root).expect("create recent-open test directory");
    let csv_directory = root.join("sample.csv");
    let plotium_directory = root.join("sample.plotium");
    std::fs::create_dir(&csv_directory).expect("create CSV-named directory");
    std::fs::create_dir(&plotium_directory).expect("create Plotium-named directory");
    let kinds = (
        recent::classify_open_path(&csv_directory).unwrap().kind(),
        recent::classify_open_path(&plotium_directory)
            .unwrap()
            .kind(),
    );
    std::fs::remove_dir_all(&root).expect("remove recent-open test directory");
    assert_eq!(kinds, (RecentOpenKind::Folder, RecentOpenKind::Folder));
}

#[test]
fn project_dispatch_preserves_dirty_document_confirmation() {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    app.mark_document_dirty();
    let path = PathBuf::from("pending-project.plotium");

    recent::dispatch_classified_path(&mut app, &path, recent::ClassifiedOpenPath::Project);

    let pending = app
        .session
        .ui
        .project_transition
        .expect("a dirty document must defer the project open");
    assert_eq!(
        pending.target,
        plotium_core::state::ProjectTransition::Open(path)
    );
    assert_eq!(
        pending.phase,
        plotium_core::state::ProjectTransitionPhase::NeedsConfirmation
    );
}
