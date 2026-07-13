//! On-disk cache of pkgutil receipt lookups, keyed by bundle path + mtime so a
//! reinstall (new mtime) invalidates its entry. Negative lookups are cached
//! too — "this bundle has no receipt" is exactly as expensive to recompute.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEntry {
    pub mtime_ms: u64,
    pub package_id: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Default)]
pub struct ReceiptCache {
    entries: HashMap<String, CacheEntry>,
}

impl ReceiptCache {
    /// Missing, unreadable, corrupt, or future-versioned files all load as an
    /// empty cache — worst case is one full pkgutil pass, never an error.
    pub fn load(path: &Path) -> Self {
        let entries = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<CacheFile>(&b).ok())
            .filter(|f| f.version == VERSION)
            .map(|f| f.entries)
            .unwrap_or_default();
        Self { entries }
    }

    /// Atomic write: temp file in the same directory, then rename.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
        std::fs::create_dir_all(parent)?;
        let file = CacheFile {
            version: VERSION,
            entries: self.entries.clone(),
        };
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&file)?)?;
        std::fs::rename(&tmp, path)
    }

    /// `Some(package_id)` on a fresh hit (the inner Option is the cached
    /// answer, which may be a cached "no receipt"); `None` means recompute.
    pub fn lookup(&self, id: &str, mtime_ms: u64) -> Option<Option<String>> {
        self.entries
            .get(id)
            .filter(|e| e.mtime_ms == mtime_ms)
            .map(|e| e.package_id.clone())
    }

    pub fn insert(&mut self, id: String, mtime_ms: u64, package_id: Option<String>) {
        self.entries.insert(
            id,
            CacheEntry {
                mtime_ms,
                package_id,
            },
        );
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Bundle root mtime in ms since epoch; `None` when the path can't be statted.
pub fn bundle_mtime_ms(path: &str) -> Option<u64> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    m.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_hits_only_on_matching_mtime() {
        let mut c = ReceiptCache::default();
        c.insert("/p/a.vst3".into(), 100, Some("com.a.pkg".into()));
        c.insert("/p/b.vst3".into(), 200, None); // negative result cached too
        assert_eq!(c.lookup("/p/a.vst3", 100), Some(Some("com.a.pkg".into())));
        assert_eq!(c.lookup("/p/a.vst3", 999), None); // mtime moved → miss
        assert_eq!(c.lookup("/p/b.vst3", 200), Some(None)); // cached negative
        assert_eq!(c.lookup("/p/unknown", 1), None);
    }

    #[test]
    fn roundtrips_through_disk_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("receipt-cache.json");
        let mut c = ReceiptCache::default();
        c.insert("/p/a.vst3".into(), 100, Some("com.a.pkg".into()));
        c.save(&path).unwrap(); // creates parent dirs
        let loaded = ReceiptCache::load(&path);
        assert_eq!(
            loaded.lookup("/p/a.vst3", 100),
            Some(Some("com.a.pkg".into()))
        );
        // no leftover temp files
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("receipt-cache.json")]);
    }

    #[test]
    fn corrupt_or_missing_or_wrong_version_loads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.json");
        assert_eq!(ReceiptCache::load(&p).len(), 0); // missing
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(ReceiptCache::load(&p).len(), 0); // corrupt
        std::fs::write(
            &p,
            r#"{"version": 99, "entries": {"/x": {"mtimeMs": 1, "packageId": null}}}"#,
        )
        .unwrap();
        assert_eq!(ReceiptCache::load(&p).len(), 0); // future version discarded
    }
}
