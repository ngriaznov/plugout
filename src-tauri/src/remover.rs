use crate::model::{PluginBundle, RemovalResult, RemovalStatus};
use std::process::Command;

pub trait Trasher {
    fn trash(&self, paths: &[String]) -> Result<(), String>;
    fn trash_privileged(&self, paths: &[String], forget_pkg: Option<&str>) -> Result<(), String>;
}

pub fn is_system_path(path: &str, home: &str) -> bool {
    // Boundary-aware: "/Users/tim" must not match "/Users/timothy/…".
    !path.starts_with(&format!("{home}/"))
}

/// Trash the selected plugin bundles. Batched by scope: all user-scope bundles go
/// in one Trash call, and all system-scope bundles go in a single privileged call
/// (so the user sees at most one admin prompt per removal, not one per plugin).
pub fn remove_bundles(
    bundles: &[PluginBundle],
    home: &str,
    trasher: &dyn Trasher,
) -> Vec<RemovalResult> {
    let user_paths: Vec<String> = bundles
        .iter()
        .filter(|b| !is_system_path(&b.path, home))
        .map(|b| b.path.clone())
        .collect();
    let sys_paths: Vec<String> = bundles
        .iter()
        .filter(|b| is_system_path(&b.path, home))
        .map(|b| b.path.clone())
        .collect();

    let user_result = if user_paths.is_empty() {
        Ok(())
    } else {
        trasher.trash(&user_paths)
    };
    let sys_result = if sys_paths.is_empty() {
        Ok(())
    } else {
        trasher.trash_privileged(&sys_paths, None)
    };

    bundles
        .iter()
        .map(|b| {
            let outcome = if is_system_path(&b.path, home) {
                &sys_result
            } else {
                &user_result
            };
            match outcome {
                Ok(()) => RemovalResult {
                    id: b.id.clone(),
                    path: b.path.clone(),
                    status: RemovalStatus::Trashed,
                    message: None,
                },
                Err(e) => RemovalResult {
                    id: b.id.clone(),
                    path: b.path.clone(),
                    status: RemovalStatus::Failed,
                    message: Some(e.clone()),
                },
            }
        })
        .collect()
}

pub struct RealTrasher;

impl Trasher for RealTrasher {
    fn trash(&self, paths: &[String]) -> Result<(), String> {
        trash::delete_all(paths).map_err(|e| e.to_string())
    }

    fn trash_privileged(&self, paths: &[String], forget_pkg: Option<&str>) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_default();
        let trash = format!("{home}/.Trash");
        // `set -e` makes any failed move abort with a non-zero exit, so a move that
        // did not happen is surfaced as an error instead of a false success (stderr is
        // captured, not discarded). A unique per-operation subfolder avoids clobbering
        // same-named items already in the Trash.
        let mut sh = format!(
            "set -e; mkdir -p {t}; d={t}/plugout-$(date +%s)-$$; mkdir -p \"$d\"; ",
            t = sh_quote(&trash)
        );
        for p in paths {
            sh.push_str(&format!("mv {} \"$d\"/; ", sh_quote(p)));
        }
        if let Some(pkg) = forget_pkg {
            // Forgetting the receipt is best-effort cleanup; it must not fail the removal.
            sh.push_str(&format!(
                "pkgutil --forget {} >/dev/null 2>&1 || true; ",
                sh_quote(pkg)
            ));
        }

        let apple = format!(
            "do shell script \"{}\" with administrator privileges",
            as_escape(&sh)
        );
        let out = Command::new("osascript")
            .arg("-e")
            .arg(apple)
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
}

/// Single-quote a string for POSIX sh.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Escape for embedding inside an AppleScript double-quoted string.
fn as_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Format, Scope};
    use std::cell::RefCell;

    #[derive(Default)]
    struct SpyTrasher {
        user: RefCell<Vec<String>>,
        priv_: RefCell<Vec<String>>,
        forgot: RefCell<Vec<String>>,
        user_calls: RefCell<u32>,
        priv_calls: RefCell<u32>,
    }
    impl Trasher for SpyTrasher {
        fn trash(&self, paths: &[String]) -> Result<(), String> {
            *self.user_calls.borrow_mut() += 1;
            self.user.borrow_mut().extend_from_slice(paths);
            Ok(())
        }
        fn trash_privileged(&self, paths: &[String], forget: Option<&str>) -> Result<(), String> {
            *self.priv_calls.borrow_mut() += 1;
            self.priv_.borrow_mut().extend_from_slice(paths);
            if let Some(f) = forget {
                self.forgot.borrow_mut().push(f.to_string());
            }
            Ok(())
        }
    }

    fn bundle(path: &str, scope: Scope) -> PluginBundle {
        PluginBundle {
            id: path.into(),
            name: "N".into(),
            vendor: "V".into(),
            version: "1".into(),
            format: Format::Vst3,
            bundle_id: "com.v.n".into(),
            path: path.into(),
            size_bytes: 1,
            scope,
            package_id: None,
        }
    }

    #[test]
    fn is_system_path_detects_library() {
        assert!(is_system_path(
            "/Library/Audio/Plug-Ins/VST3/X.vst3",
            "/Users/me"
        ));
        assert!(!is_system_path(
            "/Users/me/Library/Audio/Plug-Ins/VST3/X.vst3",
            "/Users/me"
        ));
    }

    #[test]
    fn user_bundles_trashed_in_one_batch() {
        let spy = SpyTrasher::default();
        let a = bundle("/Users/me/Library/Audio/Plug-Ins/VST3/A.vst3", Scope::User);
        let b = bundle(
            "/Users/me/Library/Audio/Plug-Ins/Components/B.component",
            Scope::User,
        );
        let res = remove_bundles(&[a, b], "/Users/me", &spy);
        assert!(res.iter().all(|r| r.status == RemovalStatus::Trashed));
        assert_eq!(*spy.user_calls.borrow(), 1); // single batched Trash call
        assert_eq!(spy.user.borrow().len(), 2);
        assert!(spy.priv_.borrow().is_empty());
    }

    #[test]
    fn system_bundles_trashed_in_one_privileged_batch() {
        let spy = SpyTrasher::default();
        let a = bundle("/Library/Audio/Plug-Ins/VST3/A.vst3", Scope::System);
        let b = bundle(
            "/Library/Audio/Plug-Ins/Components/B.component",
            Scope::System,
        );
        let res = remove_bundles(&[a, b], "/Users/me", &spy);
        assert!(res.iter().all(|r| r.status == RemovalStatus::Trashed));
        assert_eq!(*spy.priv_calls.borrow(), 1); // single admin prompt for both
        assert_eq!(spy.priv_.borrow().len(), 2);
        assert!(spy.forgot.borrow().is_empty());
        assert!(spy.user.borrow().is_empty());
    }

    #[test]
    fn mixed_scope_splits_into_one_call_each() {
        let spy = SpyTrasher::default();
        let u = bundle("/Users/me/Library/Audio/Plug-Ins/VST3/U.vst3", Scope::User);
        let s = bundle("/Library/Audio/Plug-Ins/VST3/S.vst3", Scope::System);
        let res = remove_bundles(&[u, s], "/Users/me", &spy);
        assert_eq!(res.len(), 2);
        assert_eq!(*spy.user_calls.borrow(), 1);
        assert_eq!(*spy.priv_calls.borrow(), 1);
    }
}
