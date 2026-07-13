use crate::error::CmdError;
use crate::model::{RemovalResult, RemovalStatus};
use std::process::Command;

pub trait Trasher {
    fn trash(&self, paths: &[String]) -> Result<(), CmdError>;
    fn trash_privileged(&self, paths: &[String]) -> Result<(), CmdError>;
}

pub fn is_system_path(path: &str, home: &str) -> bool {
    // Boundary-aware: "/Users/tim" must not match "/Users/timothy/…".
    !path.starts_with(&format!("{home}/"))
}

/// Trash the given bundle paths (a bundle's id IS its path). Batched by scope:
/// all user-scope paths go in one Trash call, and all system-scope paths go in a
/// single privileged call — so the user sees at most one admin prompt per
/// removal, not one per plugin.
pub fn remove_paths(paths: &[String], home: &str, trasher: &dyn Trasher) -> Vec<RemovalResult> {
    let (sys_paths, user_paths): (Vec<String>, Vec<String>) =
        paths.iter().cloned().partition(|p| is_system_path(p, home));

    let user_result = if user_paths.is_empty() {
        Ok(())
    } else {
        trasher.trash(&user_paths)
    };
    let sys_result = if sys_paths.is_empty() {
        Ok(())
    } else {
        trasher.trash_privileged(&sys_paths)
    };

    paths
        .iter()
        .map(|p| {
            let outcome = if is_system_path(p, home) {
                &sys_result
            } else {
                &user_result
            };
            let (status, message) = match outcome {
                Ok(()) => (RemovalStatus::Trashed, None),
                Err(CmdError::Canceled) => (RemovalStatus::Canceled, None),
                Err(e) => (RemovalStatus::Failed, Some(e.to_string())),
            };
            RemovalResult {
                id: p.clone(),
                path: p.clone(),
                status,
                message,
            }
        })
        .collect()
}

pub struct RealTrasher;

impl Trasher for RealTrasher {
    fn trash(&self, paths: &[String]) -> Result<(), CmdError> {
        trash::delete_all(paths).map_err(|e| CmdError::Internal(e.to_string()))
    }

    fn trash_privileged(&self, paths: &[String]) -> Result<(), CmdError> {
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

        let apple = format!(
            "do shell script \"{}\" with administrator privileges",
            as_escape(&sh)
        );
        let out = Command::new("osascript")
            .arg("-e")
            .arg(apple)
            .output()
            .map_err(CmdError::from)?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        let canceled = stderr.contains("User canceled") || stderr.contains("(-128)");
        if canceled {
            return Err(CmdError::Canceled);
        }
        Err(CmdError::PermissionDenied(stderr.trim().to_string()))
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
    use std::cell::RefCell;

    #[derive(Default)]
    struct SpyTrasher {
        user: RefCell<Vec<String>>,
        priv_: RefCell<Vec<String>>,
        user_calls: RefCell<u32>,
        priv_calls: RefCell<u32>,
        fail_privileged: Option<CmdError>,
    }
    impl Trasher for SpyTrasher {
        fn trash(&self, paths: &[String]) -> Result<(), CmdError> {
            *self.user_calls.borrow_mut() += 1;
            self.user.borrow_mut().extend_from_slice(paths);
            Ok(())
        }
        fn trash_privileged(&self, paths: &[String]) -> Result<(), CmdError> {
            *self.priv_calls.borrow_mut() += 1;
            self.priv_.borrow_mut().extend_from_slice(paths);
            match &self.fail_privileged {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }
    }

    fn paths(ps: &[&str]) -> Vec<String> {
        ps.iter().map(|p| p.to_string()).collect()
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
    fn user_paths_trashed_in_one_batch() {
        let spy = SpyTrasher::default();
        let res = remove_paths(
            &paths(&[
                "/Users/me/Library/Audio/Plug-Ins/VST3/A.vst3",
                "/Users/me/Library/Audio/Plug-Ins/Components/B.component",
            ]),
            "/Users/me",
            &spy,
        );
        assert!(res.iter().all(|r| r.status == RemovalStatus::Trashed));
        assert_eq!(*spy.user_calls.borrow(), 1); // single batched Trash call
        assert_eq!(spy.user.borrow().len(), 2);
        assert!(spy.priv_.borrow().is_empty());
    }

    #[test]
    fn system_paths_trashed_in_one_privileged_batch() {
        let spy = SpyTrasher::default();
        let res = remove_paths(
            &paths(&[
                "/Library/Audio/Plug-Ins/VST3/A.vst3",
                "/Library/Audio/Plug-Ins/Components/B.component",
            ]),
            "/Users/me",
            &spy,
        );
        assert!(res.iter().all(|r| r.status == RemovalStatus::Trashed));
        assert_eq!(*spy.priv_calls.borrow(), 1); // single admin prompt for both
        assert_eq!(spy.priv_.borrow().len(), 2);
        assert!(spy.user.borrow().is_empty());
    }

    #[test]
    fn mixed_scope_splits_into_one_call_each() {
        let spy = SpyTrasher::default();
        let res = remove_paths(
            &paths(&[
                "/Users/me/Library/Audio/Plug-Ins/VST3/U.vst3",
                "/Library/Audio/Plug-Ins/VST3/S.vst3",
            ]),
            "/Users/me",
            &spy,
        );
        assert_eq!(res.len(), 2);
        assert_eq!(*spy.user_calls.borrow(), 1);
        assert_eq!(*spy.priv_calls.borrow(), 1);
    }

    #[test]
    fn privileged_failure_marks_only_system_paths_failed() {
        let spy = SpyTrasher {
            fail_privileged: Some(CmdError::PermissionDenied("denied".into())),
            ..Default::default()
        };
        let res = remove_paths(
            &paths(&[
                "/Users/me/Library/Audio/Plug-Ins/VST3/U.vst3",
                "/Library/Audio/Plug-Ins/VST3/S.vst3",
            ]),
            "/Users/me",
            &spy,
        );
        assert_eq!(res[0].status, RemovalStatus::Trashed);
        assert_eq!(res[1].status, RemovalStatus::Failed);
        assert_eq!(res[1].message.as_deref(), Some("permission denied: denied"));
    }

    #[test]
    fn privileged_cancellation_marks_all_system_paths_canceled_user_still_trashed() {
        let spy = SpyTrasher {
            fail_privileged: Some(CmdError::Canceled),
            ..Default::default()
        };
        let res = remove_paths(
            &paths(&[
                "/Users/me/Library/Audio/Plug-Ins/VST3/U.vst3",
                "/Library/Audio/Plug-Ins/VST3/S1.vst3",
                "/Library/Audio/Plug-Ins/VST3/S2.vst3",
            ]),
            "/Users/me",
            &spy,
        );
        assert_eq!(res[0].status, RemovalStatus::Trashed);
        assert!(
            res[1..]
                .iter()
                .all(|r| r.status == RemovalStatus::Canceled && r.message.is_none())
        );
    }
}
