# Plotium

[![CI](https://github.com/nmrtist/plotium/actions/workflows/ci.yml/badge.svg)](https://github.com/nmrtist/plotium/actions/workflows/ci.yml)

Plotium is a native desktop application for scientific data analysis and figure
preparation.

[User manual](https://plotium.nmrtist.space/) · [Releases](https://github.com/nmrtist/plotium/releases) · [Contributing](CONTRIBUTING.md)

## Highlights

- **Bring scientific data together.** Import NMR, mass spectrometry,
  electrophysiology, and XRD data, plus spectra and tables in common formats.
- **Process and analyze interactively.** Build ordered processing pipelines,
  then pick peaks, integrate regions, and fit data. NMR workflows also include
  DOSY and relaxation analysis, plus sweep statistics and IV analysis for
  electrophysiology recordings.
- **Prepare figures for submission.** Compose spectra, tables, and annotations
  on a multipage board; use journal-size and resolution presets, check font
  sizes and line widths before export, then write vector or raster files. On
  Windows, figures also copy to the clipboard as both bitmap and vector data.
- **Make work reproducible.** Save complete `.plotium` sessions and reuse
  `.plotiumproc` processing recipes across datasets.

The [user manual](https://plotium.nmrtist.space/) has the supported-format
matrix, the processing-step reference, and workflow guides.

## Install

Prebuilt packages are published on the
[Releases](https://github.com/nmrtist/plotium/releases) page; the
[installation guide](https://plotium.nmrtist.space/getting-started/installation/)
covers setup and updates. If there is no package for your platform yet, build
from source below.

## Build from source

Building requires the current stable Rust toolchain.

```sh
git clone https://github.com/nmrtist/plotium.git
cd plotium
cargo run --release -p plotium
```

The development workflow, optional backend builds, and pre-submission checks are in
[CONTRIBUTING.md](CONTRIBUTING.md).

## Project status

Plotium is pre-1.0 and under active development.

## License

Plotium is licensed under the [GNU GPL v3.0 or later](LICENSE).

See [COPYRIGHT.md](COPYRIGHT.md) for asset licensing and use of the Plotium
name and logos.
