use super::*;
use crate::state::{
    DatasetLineage, DerivationKind, FloatSeries, Nmr2DDataset, NmrDataset,
    materialized_float_series_table,
};
use num_complex::Complex64;
use plotium_data::{
    TraceCollectionCatalog, TraceCollectionId, TraceItemDescriptor, TraceItemId,
    TraceItemParameter, TraceParameterValue,
};
use plotium_io::{AcquisitionIdentity, Dim, Domain, NmrData, NmrData2D, QuadMode};
use plotium_processing::Slice1D;

fn nmr(domain: Domain, subject: &str, acquisition: &str) -> Dataset {
    let mut data = crate::nmr_test_support::load_1d(NmrData {
        points: vec![Complex64::new(1.0, 0.0); 8],
        domain,
        spectral_width_hz: 4_000.0,
        observe_freq_mhz: 400.0,
        carrier_ppm: 4.7,
        nucleus: "1H".to_owned(),
        source: "fid".to_owned(),
        group_delay: 0.0,
    })
    .unwrap();
    data.acquisition_identity = AcquisitionIdentity {
        subject: Some(subject.to_owned()),
        acquisition: Some(acquisition.to_owned()),
        source_label: "fid".to_owned(),
    };
    Dataset::Nmr(Box::new(data))
}

fn nmr_2d(direct: &str, indirect: &str) -> Nmr2DDataset {
    let dimension = |nucleus: &str| Dim {
        spectral_width_hz: 4_000.0,
        observe_freq_mhz: 400.0,
        carrier_ppm: 0.0,
        nucleus: nucleus.to_owned(),
        group_delay: 0.0,
    };
    crate::nmr_test_support::load_2d(NmrData2D {
        data: vec![Complex64::new(1.0, 0.0); 4],
        rows: 2,
        cols: 2,
        domain: Domain::Frequency,
        direct: dimension(direct),
        indirect: dimension(indirect),
        quad: QuadMode::Complex,
        indirect_conjugate: false,
        experiment: None,
        pseudo_axis: None,
        diffusion: None,
        nus: None,
        source: "2d".to_owned(),
    })
    .unwrap()
}

#[test]
fn two_dimensional_nmr_observation_uses_literature_nucleus_notation() {
    assert_eq!(nmr_2d_observation(&nmr_2d("1H", "13C")).text, "¹H–¹³C");
    assert_eq!(nmr_2d_observation(&nmr_2d("1H", "1H")).text, "¹H–¹H");
}

#[test]
fn derived_slice_inherits_the_source_subject() {
    let source = nmr(Domain::Frequency, "Specimen A", "HSQC");
    let source_id = source.resource_id();
    let mut derived = Dataset::Nmr(Box::new(
        NmrDataset::from_slice(
            Slice1D {
                coordinates: vec![2.0, 1.0],
                domain: Domain::Frequency,
                values: vec![Complex64::new(1.0, 0.0); 2],
                nucleus: "1H".to_owned(),
                observe_freq_mhz: Some(400.0),
                reference_freq_mhz: Some(400.0),
                unit: nmr::axis::AxisUnit::Ppm,
                position: Some(3.0),
                position_domain: Domain::Frequency,
            },
            "F2 slice at 3 ppm".to_owned(),
        )
        .unwrap(),
    ));
    derived.set_lineage(Some(DatasetLineage::new(
        DerivationKind::Slice,
        [source_id],
    )));
    let mut app = PlotiumApp::default();
    app.doc
        .canvases
        .push(crate::workflow::build_default_canvas_for_dataset(
            &derived,
            1,
            "Slice".to_owned(),
            crate::state::DEFAULT_CANVAS_SIZE_MM,
        ));
    app.doc.datasets.extend([source, derived]);

    assert_eq!(
        app.canvas_scientific_summary(0).formatted_lines(),
        vec!["Specimen A · 1H"]
    );
}

#[test]
fn overlay_keeps_distinct_nonempty_contexts() {
    let summaries = vec![
        ScientificSummary {
            subject: SummaryPart::new("subject:a", "A"),
            observation: SummaryPart::new("nmr:1h", "1H"),
            context: Some(SummaryPart::new("experiment:cosy", "COSY")),
        },
        ScientificSummary {
            subject: SummaryPart::new("subject:a", "A"),
            observation: SummaryPart::new("nmr:1h", "1H"),
            context: Some(SummaryPart::new("experiment:tocsy", "TOCSY")),
        },
    ];
    assert_eq!(
        combine_summaries(summaries).unwrap().format(),
        "A · 1H · COSY + TOCSY"
    );
}

#[test]
fn fid_and_spectrum_have_distinct_semantic_observations() {
    let time = NmrDataset::from_slice(
        Slice1D {
            coordinates: vec![0.0, 0.001],
            domain: Domain::Time,
            values: vec![Complex64::new(1.0, 0.0); 2],
            nucleus: "1H".to_owned(),
            observe_freq_mhz: Some(400.0),
            reference_freq_mhz: Some(400.0),
            unit: nmr::axis::AxisUnit::Ppm,
            position: None,
            position_domain: Domain::Time,
        },
        "FID".to_owned(),
    )
    .unwrap();
    let frequency = nmr(Domain::Frequency, "A", "zg30");
    let time = nmr_1d_observation(&time);
    let frequency = nmr_1d_observation(frequency.as_nmr().unwrap());
    assert_ne!(time.semantic_key, frequency.semantic_key);
    assert_eq!(time.text, "1H FID");
    assert_eq!(frequency.text, "1H");
}

#[test]
fn active_trace_item_is_included_in_context() {
    let mut dataset = nmr(Domain::Frequency, "A", "zg30");
    let nmr = dataset.as_nmr_mut().unwrap();
    let field = nmr.field_catalog.id_for_key("nmr.real").unwrap();
    let collection = TraceCollectionId::new();
    let item = TraceItemId::derived(collection, b"10 ms");
    nmr.field_catalog.set_trace_collection(
        field,
        TraceCollectionCatalog {
            id: collection,
            axis_quantity: "Delay".to_owned(),
            axis_unit: "ms".to_owned(),
            items: vec![TraceItemDescriptor {
                id: item,
                parameters: vec![TraceItemParameter {
                    key: "delay".to_owned(),
                    name: "Delay".to_owned(),
                    value: TraceParameterValue::Number {
                        value: 10.0,
                        unit: "ms".to_owned(),
                    },
                }],
                primary_label_parameter: "delay".to_owned(),
                label_override: None,
            }],
        },
    );
    let mut app = PlotiumApp::default();
    let mut canvas = crate::workflow::build_default_canvas(&dataset, "fid");
    canvas.objects[0].plot_mut().unwrap().binding.series[0]
        .source
        .selection = crate::state::SeriesSelection::TraceItem(item);
    app.doc.canvases.push(canvas);
    app.doc.datasets.push(dataset);

    assert_eq!(
        app.canvas_scientific_summary(0).formatted_lines(),
        vec!["A · 1H · zg30 + 10 ms"]
    );
}

#[test]
fn table_summary_tracks_all_or_selected_plotted_columns() {
    let mut table = materialized_float_series_table(
        (
            "Time".to_owned(),
            "s".to_owned(),
            vec![Some(0.0), Some(1.0)],
        ),
        vec![
            FloatSeries {
                name: "Signal".to_owned(),
                unit: String::new(),
                values: vec![Some(1.0), Some(2.0)],
                uncertainty: None,
                fit: None,
            },
            FloatSeries {
                name: "Baseline".to_owned(),
                unit: String::new(),
                values: vec![Some(0.5), Some(0.7)],
                uncertainty: None,
                fit: None,
            },
        ],
        "summary-test",
    )
    .unwrap();
    table.acquisition_identity.subject = Some("Sample".to_owned());
    let baseline = table.series_bindings[1].value_column;
    let dataset = Dataset::Table(Box::new(table));
    let mut app = PlotiumApp::default();
    let mut canvas = crate::workflow::build_default_canvas(&dataset, "table");
    assert_eq!(
        table_observation(
            dataset.as_table().unwrap(),
            canvas.objects[0].plot().unwrap()
        )
        .text,
        "Signal + Baseline vs Time"
    );
    let plot = canvas.objects[0].plot_mut().unwrap();
    plot.chart.type_id = "table_histogram".to_owned();
    plot.chart.column = Some(baseline);
    app.doc.canvases.push(canvas);
    app.doc.datasets.push(dataset);
    assert_eq!(
        app.canvas_scientific_summary(0).formatted_lines(),
        vec!["Sample · Baseline · Histogram"]
    );
}

#[test]
fn xps_local_keys_are_scoped_to_the_dataset() {
    assert_ne!(
        xps_local_key(DatasetId::new(), "region", 1),
        xps_local_key(DatasetId::new(), "region", 1)
    );
}
