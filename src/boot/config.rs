use std::{
    env,
    path::{Path, PathBuf},
};

const NEW_DATA_DIR_ENV: &str = "AXIOMNEXUS_DATA_DIR";
const NEW_STORE_URL_ENV: &str = "AXIOMNEXUS_STORE_URL";
const NEW_EXPORT_PATH_ENV: &str = "AXIOMNEXUS_EXPORT_PATH";
const NEW_HTTP_ADDR_ENV: &str = "AXIOMNEXUS_HTTP_ADDR";
const DEFAULT_DATA_DIR: &str = ".axiomnexus";

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
    preferred_path_from_env(NEW_DATA_DIR_ENV).unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR))
}

fn store_url_from_env(data_dir: &Path) -> String {
    preferred_string_from_env(NEW_STORE_URL_ENV)
        .unwrap_or_else(|| default_surrealkv_store_url(data_dir))
}

fn default_surrealkv_store_url(data_dir: &Path) -> String {
    format!("surrealkv://{}", data_dir.join("state.db").display())
}

fn export_path_from_env(data_dir: &Path) -> PathBuf {
    preferred_path_from_env(NEW_EXPORT_PATH_ENV)
        .unwrap_or_else(|| data_dir.join("store_snapshot.json"))
}

fn http_bind_addr_from_env() -> String {
    preferred_string_from_env(NEW_HTTP_ADDR_ENV).unwrap_or_else(|| "127.0.0.1:3000".to_owned())
}

fn preferred_path_from_env(primary: &str) -> Option<PathBuf> {
    preferred_os_string_from_env(primary).map(PathBuf::from)
}

fn preferred_string_from_env(primary: &str) -> Option<String> {
    env::var(primary).ok().filter(|value| !value.is_empty())
}

fn preferred_os_string_from_env(primary: &str) -> Option<std::ffi::OsString> {
    env::var_os(primary).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::default_surrealkv_store_url;

    #[test]
    fn store_url_reads_only_target_env() {
        let resolved = resolve_store_url(
            Some("surrealkv://.axiomnexus/state.db".to_owned()),
            PathBuf::from(".axiomnexus"),
        );

        assert_eq!(resolved, "surrealkv://.axiomnexus/state.db");
    }

    #[test]
    fn store_url_defaults_to_embedded_surreal_runtime() {
        let resolved = resolve_store_url(None, PathBuf::from(".axiomnexus"));

        assert_eq!(
            resolved,
            default_surrealkv_store_url(PathBuf::from(".axiomnexus").as_path())
        );
    }

    #[test]
    fn export_path_defaults_under_data_dir() {
        assert_eq!(
            resolve_export_path(None, PathBuf::from(".axiomnexus")),
            PathBuf::from(".axiomnexus/store_snapshot.json")
        );
    }

    #[test]
    fn export_path_prefers_explicit_env() {
        assert_eq!(
            resolve_export_path(
                Some(PathBuf::from("/tmp/export.json")),
                PathBuf::from(".axiomnexus")
            ),
            PathBuf::from("/tmp/export.json")
        );
    }

    #[test]
    fn data_dir_defaults_to_current_runtime_dir() {
        assert_eq!(resolve_data_dir(None), PathBuf::from(".axiomnexus"));
    }

    fn resolve_store_url(store_url: Option<String>, data_dir: PathBuf) -> String {
        store_url.unwrap_or_else(|| default_surrealkv_store_url(data_dir.as_path()))
    }

    fn resolve_export_path(export_path: Option<PathBuf>, data_dir: PathBuf) -> PathBuf {
        export_path.unwrap_or_else(|| data_dir.join("store_snapshot.json"))
    }

    fn resolve_data_dir(data_dir: Option<PathBuf>) -> PathBuf {
        data_dir.unwrap_or_else(|| PathBuf::from(".axiomnexus"))
    }
}
