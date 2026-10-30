use std::path::{Path, PathBuf};

/// The app's writable folders. Config is roaming; caches and imported fonts are local.
#[derive(Clone, Debug)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub fonts: PathBuf,
    pub presets: PathBuf,
    pub log: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let roaming = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("IcebergRoaming"));
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("IcebergLocal"));
        Self::new(roaming.join("Iceberg"), local.join("Iceberg"))
    }

    pub fn new(roaming: PathBuf, local: PathBuf) -> Self {
        Self {
            config: roaming.join("config.json"),
            data: local.clone(),
            cache: local.join("cache"),
            fonts: local.join("fonts"),
            presets: local.join("presets"),
            log: local.join("widget.log"),
        }
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        if let Some(parent) = self.config.parent() {
            std::fs::create_dir_all(parent)?;
        }
        for directory in [&self.data, &self.cache, &self.fonts, &self.presets] {
            std::fs::create_dir_all(directory)?;
        }
        Ok(())
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_under_the_given_roots() {
        let paths = Paths::new(PathBuf::from(r"C:\Roam"), PathBuf::from(r"C:\Local"));
        assert_eq!(paths.config, PathBuf::from(r"C:\Roam\config.json"));
        assert_eq!(paths.cache, PathBuf::from(r"C:\Local\cache"));
        assert_eq!(paths.fonts, PathBuf::from(r"C:\Local\fonts"));
    }
}
