# Contributing to Plotium

## Start developing

Use the current stable Rust toolchain with `rustfmt` and `clippy`. Run these
commands from the repository root:

```sh
rustup component add rustfmt clippy
cargo install --locked cargo-deny
cargo run -p plotium
```

For the edit/check/test loop, target the frontend or crate you changed:

| Purpose | Command |
| --- | --- |
| Check the desktop app | `cargo check -p plotium` |
| Run the CLI | `cargo run -p plotium-cli -- --help` |
| Test a changed crate (example) | `cargo test -p plotium-core` |
| Build the shipping app / assess performance | `cargo release-build` |

Development builds optimize numerical processing crates, but leave the UI and
renderer unoptimized. Use a release build to assess rendering performance.

Default builds use the reference table executor. Avoid `--workspace` during
routine development: it also selects the optional DataFusion and Substrait
crates. For explicit backend maintenance, use `cargo datafusion-build`;
`cargo pr-check` and CI do not validate those crates.

## Find the code

| Area | Location |
| --- | --- |
| Desktop UI | [`crates/app/`](crates/app/) — read its [UI rules](crates/app/AGENTS.md) before editing |
| CLI | [`crates/cli/`](crates/cli/) |
| Application state, actions, and workflows | [`crates/core/`](crates/core/) |
| Data model and table semantics | [`crates/data/`](crates/data/) |
| File parsing and import fixtures | [`crates/io/`](crates/io/) |
| Scientific algorithms and spectral transforms | [`crates/analysis/`](crates/analysis/), [`crates/processing/`](crates/processing/) |
| Figure models and rendering/export | [`crates/figure/`](crates/figure/), [`crates/render/`](crates/render/) |

[AGENTS.md](AGENTS.md) records repository-wide invariants, including typed IDs,
crate boundaries, and persisted-format rules.

For UI screenshots, set `PLOTIUM_SHOT` to an output directory and run the app.
The harness runs a synthetic session, captures both themes, and exits; see
[`shot.rs`](crates/app/src/shot.rs) for scene and theme controls.

## Edit the manual

The Astro/Starlight manual lives in `docs/`. With Node.js LTS installed:

```sh
cd docs
npm ci
npm run dev
```

English pages in `docs/src/content/docs/` are canonical. Keep matching pages
under `zh-cn/` aligned when changing translated content. Run `npm run build`
from `docs/` before submitting. See [docs/README.md](docs/README.md) for the site
layout and deployment details.

## Before a pull request

- Add or update tests for changed behavior, especially numerical results and
  import edge cases. For UI changes, also check the affected interaction in the app.
- Run `cargo pr-check` for Rust changes. It checks formatting, the 800-line Rust
  source limit, dependencies, both frontends, Clippy, and reference-backend tests.
  To rerun a failed stage, use `cargo pr-check quick`, `cargo pr-check lint`, or
  `cargo pr-check test`.
- Update the manual for user-visible changes and build it as described above.
- Explain the problem, the resulting behavior, and how you validated the change
  in the pull request. Include screenshots for visual changes.
- Keep generated output (`target/`, `dist/`, `docs/dist/`, `docs/node_modules/`)
  out of commits. Use minimal anonymized samples; never include confidential,
  personal, or proprietary data in code, issues, or pull requests.

## Publish a release

The [Release workflow](.github/workflows/release.yml) packages the desktop app
and CLI for Windows x86-64, Linux x86-64, and macOS Intel / Apple Silicon.
It does not rerun CI or require extra secrets.

1. Set `[workspace.package].version` in `Cargo.toml`, refresh `Cargo.lock`, and
   merge the change. Wait for CI to pass on the release commit.
2. Tag that commit with the Cargo version prefixed by `v`, then push the tag:

   ```sh
   git tag -a v0.1.0 -m "Plotium 0.1.0"
   git push origin v0.1.0
   ```

3. Download the packages from the generated Release draft, check the desktop
   app, CLI, and license notices on each platform, then edit the notes and publish.

Use `v0.1.0` for a regular pre-1.0 release. Only `-alpha.N` and `-beta.N`
suffixes are supported for prereleases; they also select the updater channel.

The draft includes unsigned portable ZIPs with license notices, matching source,
and `SHA256SUMS`. Platform requirements are in the
[installation guide](https://plotium.nmrtist.space/getting-started/installation/).

For a rehearsal, use **Actions → Release → Run workflow** on the chosen ref;
this uploads Actions artifacts without creating a release. Failed runs can be
retried to finish a draft; published releases require a new version.

## Contribution licensing

Contributions use the project's `GPL-3.0-or-later` license; contributors retain
their copyright. Make sure you have the right to contribute, including any
employer rights. Document the source and license of third-party material and
data files. Check new dependency licenses yourself: CI does not check license
compatibility. See [COPYRIGHT.md](COPYRIGHT.md) for the licensing notice.
