use std::{
    env,
    path::{Path, PathBuf},
};

const NEW_DATA_DIR_ENV: &str = "AXIOMNEXUS_DATA_DIR";
const LEGACY_DATA_DIR_ENV: &str = "AXIOMS_DATA_DIR";
const NEW_STORE_URL_ENV: &str = "AXIOMNEXUS_STORE_URL";
const LEGACY_STORE_URL_ENV: &str = "AXIOMS_STORE_URL";
const NEW_EXPORT_PATH_ENV: &str = "AXIOMNEXUS_EXPORT_PATH";
const LEGACY_EXPORT_PATH_ENV: &str = "AXIOMS_EXPORT_PATH";
const NEW_HTTP_ADDR_ENV: &str = "AXIOMNEXUS_HTTP_ADDR";
const LEGACY_HTTP_ADDR_ENV: &str = "AXIOMS_HTTP_ADDR";
const DEFAULT_DATA_DIR: &str = ".axiomnexus";
const LEGACY_DATA_DIR: &str = ".axioms";

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub store_url: String,
    pub export_path: PathBuf,
    pub http_bind_addr: String,
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = data_dir_from_env();
        let store_url = store_url_from_env(&data_dir);
        let export_path = export_path_from_env(&data_dir);

        Self {
            export_path,
            data_dir,
            store_url,
            http_bind_addr: http_bind_addr_from_env(),
        }
    }
}

fn data_dir_from_env() -> PathBuf {
    preferred_path_from_env(NEW_DATA_DIR_ENV, LEGACY_DATA_DIR_ENV).unwrap_or_else(|| {
        let preferred = PathBuf::from(DEFAULT_DATA_DIR);
        if preferred.exists() {
            return preferred;
        }

        let legacy = PathBuf::from(LEGACY_DATA_DIR);
        if legacy.exists() {
            return legacy;
        }

        preferred
    })
}

fn store_url_from_env(data_dir: &Path) -> String {
    preferred_string_from_env(NEW_STORE_URL_ENV, LEGACY_STORE_URL_ENV)
        .unwrap_or_else(|| default_surrealkv_store_url(data_dir))
}

fn default_surrealkv_store_url(data_dir: &Path) -> String {
    format!("surrealkv://{}", data_dir.join("state.db").display())
}

fn export_path_from_env(data_dir: &Path) -> PathBuf {
    preferred_path_from_env(NEW_EXPORT_PATH_ENV, LEGACY_EXPORT_PATH_ENV)
        .unwrap_or_else(|| data_dir.join("store_snapshot.json"))
}

fn http_bind_addr_from_env() -> String {
    preferred_string_from_env(NEW_HTTP_ADDR_ENV, LEGACY_HTTP_ADDR_ENV)
        .unwrap_or_else(|| "127.0.0.1:3000".to_owned())
}

fn preferred_path_from_env(primary: &str, legacy: &str) -> Option<PathBuf> {
    preferred_os_string_from_env(primary, legacy).map(PathBuf::from)
}

fn preferred_string_from_env(primary: &str, legacy: &str) -> Option<String> {
    env::var(primary)
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| env::var(legacy).ok().filter(|value| !value.is_empty()))
}

fn preferred_os_string_from_env(primary: &str, legacy: &str) -> Option<std::ffi::OsString> {
    env::var_os(primary)
        .filter(|value| !value.is_empty())
        .or_else(|| env::var_os(legacy).filter(|value| !value.is_empty()))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::default_surrealkv_store_url;

    #[test]
    fn store_url_reads_only_target_env() {
        let resolved = resolve_store_url(
            Some("surrealkv://.axiomnexus/state.db".to_owned()),
            Some("surrealkv://.axioms/state.db".to_owned()),
            PathBuf::from(".axiomnexus"),
        );

        assert_eq!(resolved, "surrealkv://.axiomnexus/state.db");
    }

    #[test]
    fn store_url_defaults_to_embedded_surreal_runtime() {
        let resolved = resolve_store_url(None, None, PathBuf::from(".axiomnexus"));

        assert_eq!(
            resolved,
            default_surrealkv_store_url(PathBuf::from(".axiomnexus").as_path())
        );
    }

    #[test]
    fn export_path_defaults_under_data_dir() {
        assert_eq!(
            resolve_export_path(None, None, PathBuf::from(".axiomnexus")),
            PathBuf::from(".axiomnexus/store_snapshot.json")
        );
    }

    #[test]
    fn export_path_prefers_explicit_env() {
        assert_eq!(
            resolve_export_path(
                Some(PathBuf::from("/tmp/export.json")),
                Some(PathBuf::from("/tmp/legacy-export.json")),
                PathBuf::from(".axiomnexus")
            ),
            PathBuf::from("/tmp/export.json")
        );
    }

    #[test]
    fn store_url_falls_back_to_legacy_env_when_new_env_is_missing() {
        let resolved = resolve_store_url(
            None,
            Some("surrealkv://.axioms/state.db".to_owned()),
            PathBuf::from(".axiomnexus"),
        );

        assert_eq!(resolved, "surrealkv://.axioms/state.db");
    }

    #[test]
    fn data_dir_falls_back_to_legacy_runtime_dir_when_new_dir_is_absent() {
        let temp = TestDir::new("legacy-data-dir");
        let legacy_dir = temp.path().join(".axioms");
        fs::create_dir_all(&legacy_dir).expect("legacy dir should exist");

        let resolved = resolve_data_dir(None, None, temp.path().join(".axiomnexus"), legacy_dir);

        assert_eq!(resolved, temp.path().join(".axioms"));
    }

    fn resolve_store_url(
        store_url: Option<String>,
        legacy_store_url: Option<String>,
        data_dir: PathBuf,
    ) -> String {
        store_url
            .or(legacy_store_url)
            .unwrap_or_else(|| default_surrealkv_store_url(data_dir.as_path()))
    }

    fn resolve_export_path(
        export_path: Option<PathBuf>,
        legacy_export_path: Option<PathBuf>,
        data_dir: PathBuf,
    ) -> PathBuf {
        export_path
            .or(legacy_export_path)
            .unwrap_or_else(|| data_dir.join("store_snapshot.json"))
    }

    fn resolve_data_dir(
        data_dir: Option<PathBuf>,
        legacy_data_dir: Option<PathBuf>,
        preferred: PathBuf,
        legacy: PathBuf,
    ) -> PathBuf {
        data_dir.or(legacy_data_dir).unwrap_or_else(|| {
            if preferred.exists() {
                preferred
            } else if legacy.exists() {
                legacy
            } else {
                preferred
            }
        })
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should advance")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "axiomnexus-config-{label}-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("temp dir should exist");
            Self { path }
        }

        fn path(&self) -> &PathBuf {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
