// HOME is process-global: never mutate it in the parallel test harness. The
// native completion path persists settings, so run these Unix filesystem tests
// in an independently launched harness with its own verified platform paths.
#[cfg(unix)]
pub(crate) fn isolated(name: &str, run: impl FnOnce()) {
    const KEY: &str = "PLOTIUM_APP_SETTINGS_TEST_HOME";
    if let Some(home) = std::env::var_os(KEY) {
        let home = std::path::PathBuf::from(home);
        let config = plotium_core::settings::config_dir().unwrap();
        let data = plotium_core::settings::data_local_dir().unwrap();
        assert!(
            config.starts_with(&home),
            "Unexpected settings path: {config:?}"
        );
        assert!(data.starts_with(&home), "Unexpected data path: {data:?}");
        eprintln!("Isolated native save test: config={config:?}; data={data:?}");
        run();
    } else {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", home.path().join(".config"))
            .env("XDG_DATA_HOME", home.path().join(".local/share"))
            .env(KEY, home.path())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprint!("{stderr}");
        assert!(
            stderr.contains("Isolated native save test:"),
            "Child test body was not entered: {name}"
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
