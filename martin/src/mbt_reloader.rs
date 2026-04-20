use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use martin_core::tiles::mbtiles::MbtSource;
use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::MartinResult;
use crate::config::file::mbtiles::MbtConfig;
use crate::config::file::{
    CachePolicy, ConfigFileError, FileConfig, FileConfigEnum, FileConfigSrc, TileSourceWarning,
    collect_files_with_extension, is_sqlite_memory_uri,
};
use crate::config::primitives::IdResolver;
use crate::reload::{DeletedSource, NewSource, ReloadAdvisory};
use crate::tile_source_manager::TileSourceManager;

/// Per-source state tracked across reloads.
struct TrackedSource {
    /// Canonical (absolute) path for filesystem operations and dedup.
    canonical: PathBuf,
    /// Original path as discovered/configured, for `--save-config` serialization.
    original: PathBuf,
    cache: CachePolicy,
    mtime: u64,
}

/// Reloader actor that owns the `MBTiles` lifecycle: initialization, file watching, and reload.
pub struct MBTilesReloader {
    manager: TileSourceManager,
    id_resolver: IdResolver,
    default_cache: CachePolicy,
    directories: Vec<PathBuf>,
    /// Explicit source configs from the config file (immutable after construction).
    /// `resolved_id` -> `(canonical_path, original_path, cache_policy)`
    explicit_sources: BTreeMap<String, (PathBuf, PathBuf, CachePolicy)>,
    /// All currently tracked sources: `source_id` -> tracked state.
    sources: BTreeMap<String, TrackedSource>,
    event_rx: flume::Receiver<notify::Result<notify::Event>>,
    _watcher: RecommendedWatcher,
    cancel: CancellationToken,
}

impl MBTilesReloader {
    /// Creates a new [`MBTilesReloader`] from the `MBTiles` section of the config.
    pub fn new(
        manager: TileSourceManager,
        config: FileConfig<MbtConfig>,
        id_resolver: IdResolver,
        default_cache: CachePolicy,
        cancel: CancellationToken,
    ) -> MartinResult<Self> {
        let mut explicit_sources = BTreeMap::new();
        let mut directories = Vec::new();

        if let Some(sources) = config.sources {
            for (id, source) in sources {
                let cache = source.cache_zoom().or(default_cache);
                let orig_path = source.get_path().clone();
                match source.abs_path() {
                    Ok(can) => {
                        let id = id_resolver.resolve(&id, can.to_string_lossy().to_string());
                        explicit_sources.insert(id, (can, orig_path, cache));
                    }
                    Err(err) => {
                        warn!("Skipping MBTiles source {id}: {err}");
                    }
                }
            }
        }

        for path in config.paths {
            if path.is_dir() {
                directories.push(path);
            } else if path.is_file() {
                let can = path
                    .canonicalize()
                    .map_err(|e| ConfigFileError::IoError(e, path.clone()))?;
                let stem = path
                    .file_stem()
                    .map_or("_unknown".to_string(), |s| s.to_string_lossy().to_string());
                let id = id_resolver.resolve(&stem, can.to_string_lossy().to_string());
                explicit_sources.insert(id, (can, path, default_cache));
            } else {
                return Err(
                    ConfigFileError::InvalidFilePath(path.canonicalize().unwrap_or(path)).into(),
                );
            }
        }

        let (tx, event_rx) = flume::bounded(256);
        let mut watcher = RecommendedWatcher::new(
            move |event| {
                let _ = tx.send(event);
            },
            notify::Config::default(),
        )?;

        for dir in &directories {
            if let Err(err) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                warn!("Failed to watch directory {}: {err}", dir.display());
            }
        }

        for (path, _orig, _cache) in explicit_sources.values() {
            if !is_sqlite_memory_uri(path)
                && let Some(parent) = path.parent()
                && let Err(err) = watcher.watch(parent, RecursiveMode::NonRecursive)
            {
                warn!("Failed to watch path {}: {err}", parent.display());
            }
        }

        Ok(Self {
            manager,
            id_resolver,
            default_cache,
            directories,
            explicit_sources,
            sources: BTreeMap::new(),
            event_rx,
            _watcher: watcher,
            cancel,
        })
    }

    /// Scans for changes, diffs against current state, and applies to the manager.
    ///
    /// Used both for initial load (from empty state) and runtime reloads.
    pub async fn reload_once(&mut self) -> MartinResult<Vec<TileSourceWarning>> {
        let next = self.scan();
        let mut warnings = Vec::new();
        let mut advisory = ReloadAdvisory::default();

        // Removals: sources we had but are no longer present
        for id in self.sources.keys() {
            if !next.contains_key(id) {
                advisory.removals.insert(DeletedSource { id: id.clone() });
            }
        }

        // Additions and updates
        for (id, tracked) in &next {
            let is_addition = !self.sources.contains_key(id);
            let is_update = self
                .sources
                .get(id)
                .is_some_and(|old| old.mtime != tracked.mtime);

            if !is_addition && !is_update {
                continue;
            }

            match MbtSource::new(id.clone(), tracked.canonical.clone(), tracked.cache.zoom()).await
            {
                Ok(src) => {
                    if is_addition {
                        info!(
                            "Configured MBTiles source {id} from {}",
                            tracked.original.display()
                        );
                    }
                    let target = if is_update {
                        &mut advisory.updates
                    } else {
                        &mut advisory.additions
                    };
                    target.push(NewSource {
                        id: id.clone(),
                        source: Box::new(src),
                    });
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    self.manager.handle_source_error(id, err.into())?;
                    warnings.push(TileSourceWarning::SourceError {
                        source_id: id.clone(),
                        error: err_msg,
                    });
                }
            }
        }

        self.manager.apply_changes(advisory).await;
        self.sources = next;

        Ok(warnings)
    }

    /// Returns true if there are filesystem sources to watch.
    /// In-memory `SQLite` URIs are excluded since they have no filesystem path.
    #[must_use]
    pub fn has_watchable_sources(&self) -> bool {
        !self.directories.is_empty()
            || self
                .explicit_sources
                .values()
                .any(|(path, _, _)| !is_sqlite_memory_uri(path))
    }

    /// Returns the resolved config for `--save-config` serialization.
    #[must_use]
    pub fn resolved_config(&self) -> FileConfigEnum<MbtConfig> {
        let configs = self
            .sources
            .iter()
            .map(|(id, s)| (id.clone(), FileConfigSrc::Path(s.original.clone())))
            .collect();
        FileConfigEnum::new_extended(self.directories.clone(), configs, MbtConfig::default())
    }

    /// Event loop: waits for file watcher events or cancellation, then reloads.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                () = self.cancel.cancelled() => {
                    info!("MBTiles reloader shutting down");
                    break;
                }
                event = self.event_rx.recv_async() => {
                    let Ok(event) = event else { break };
                    if !is_mbtiles_event(&event) {
                        continue;
                    }
                    while self.event_rx.try_recv().is_ok() {}
                    match self.reload_once().await {
                        Ok(warnings) => {
                            for w in &warnings {
                                warn!("{w}");
                            }
                        }
                        Err(e) => {
                            warn!("MBTiles reload aborted: {e}");
                            self.cancel.cancel();
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Scans explicit sources and directories, returning the complete next state.
    fn scan(&self) -> BTreeMap<String, TrackedSource> {
        let mut result = BTreeMap::new();
        let mut seen_canonical = HashSet::new();

        // 1. Explicit sources (fixed IDs from config)
        for (id, (canonical, original, cache)) in &self.explicit_sources {
            if is_sqlite_memory_uri(canonical) {
                result.insert(
                    id.clone(),
                    TrackedSource {
                        canonical: canonical.clone(),
                        original: original.clone(),
                        cache: *cache,
                        mtime: 0,
                    },
                );
                seen_canonical.insert(canonical.clone());
            } else if let Ok(mtime) = file_mtime(canonical) {
                result.insert(
                    id.clone(),
                    TrackedSource {
                        canonical: canonical.clone(),
                        original: original.clone(),
                        cache: *cache,
                        mtime,
                    },
                );
                seen_canonical.insert(canonical.clone());
            }
        }

        // 2. Directory-discovered sources
        for dir in &self.directories {
            let files = match collect_files_with_extension(dir, &["mbtiles"]) {
                Ok(f) => f,
                Err(err) => {
                    warn!("Failed to scan directory {}: {err}", dir.display());
                    continue;
                }
            };

            for path in files {
                let Ok(can) = path.canonicalize() else {
                    continue;
                };
                if !seen_canonical.insert(can.clone()) {
                    continue;
                }

                let id = self
                    .id_for_canonical(&can)
                    .unwrap_or_else(|| {
                        let stem = path.file_stem().map_or("_unknown".to_string(), |s| {
                            s.to_string_lossy().to_string()
                        });
                        self.id_resolver
                            .resolve(&stem, can.to_string_lossy().to_string())
                    });

                let mtime = file_mtime(&can).unwrap_or(0);
                result.insert(
                    id,
                    TrackedSource {
                        canonical: can,
                        original: path,
                        cache: self.default_cache,
                        mtime,
                    },
                );
            }
        }

        result
    }

    /// Finds the source ID for a canonical path, if previously tracked.
    fn id_for_canonical(&self, canonical: &Path) -> Option<String> {
        self.sources
            .iter()
            .find(|(_, s)| s.canonical == canonical)
            .map(|(id, _)| id.clone())
    }
}

fn file_mtime(path: &Path) -> Result<u64, std::io::Error> {
    Ok(std::fs::metadata(path)?
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs())
}

fn is_mbtiles_event(event: &notify::Result<notify::Event>) -> bool {
    match event {
        Ok(event) => {
            event.paths.is_empty()
                || event
                    .paths
                    .iter()
                    .any(|p| p.extension().is_some_and(|ext| ext == "mbtiles"))
        }
        Err(_) => true,
    }
}
