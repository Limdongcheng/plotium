#[cfg(not(test))]
use directories::ProjectDirs;
use std::path::PathBuf;

pub fn config_dir() -> Option<PathBuf> {
    #[cfg(not(test))]
    {
        ProjectDirs::from("", "", "plotium").map(|dirs| dirs.config_dir().to_path_buf())
    }
    #[cfg(test)]
    Some(TEST_ROOT.with(|root| root.0.join("config")))
}

pub fn data_local_dir() -> Option<PathBuf> {
    #[cfg(not(test))]
    {
        ProjectDirs::from("", "", "plotium").map(|dirs| dirs.data_local_dir().to_path_buf())
    }
    #[cfg(test)]
    Some(TEST_ROOT.with(|root| root.0.join("data")))
}

pub fn settings_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("settings.json"))
}

// Native completion tests must really persist settings, but never into the
// developer's profile. Per-thread roots also prevent parallel fixtures racing.
// Downstream crate tests use the production library and isolate their process
// HOME explicitly instead; this seam does not change shipping path resolution.
#[cfg(test)]
thread_local! {
    static TEST_ROOT: TestRoot = TestRoot(std::env::temp_dir().join(format!(
        "plotium-core-test-{}", uuid::Uuid::new_v4()
    )));
}
#[cfg(test)]
struct TestRoot(PathBuf);
#[cfg(test)]
impl Drop for TestRoot {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "Could not remove test settings directory {:?}: {error}",
                self.0
            );
        }
    }
}
#[cfg(test)]
#[test]
fn native_settings_paths_are_isolated_and_persist_real_bytes() {
    let config = config_dir().unwrap();
    let data = data_local_dir().unwrap();
    assert!(config.starts_with(std::env::temp_dir()));
    assert_eq!(config.parent(), data.parent());
    let other = std::thread::spawn(|| config_dir().unwrap()).join().unwrap();
    assert_ne!(config, other);
    let mut settings = super::Settings::default();
    settings.export.include_view_snapshots = true;
    super::save(&settings).unwrap();
    assert!(settings_file().unwrap().is_file());
    assert!(super::load().export.include_view_snapshots);
}
