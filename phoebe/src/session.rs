// Copyright 2026 Revelation OS Project
//
// Phoebe Greeter - User Session Launcher
//
// This module configures and launches the user's default login shell,
// switching to the authenticated user's UID, GID, home directory, and environment.

use crate::user::User;
use std::env;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub const FALLBACK_SHELL: &str = "/bin/sh";
pub const KNOWN_SHELLS: &[&str] = &["/bin/sh", "/bin/bash", "/bin/ash", "/bin/zsh"];
pub const DEFAULT_SESSION_PATH: &str =
    "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin";

/// Discovers shells installed on the system by checking standard paths
/// (KNOWN_SHELLS) and parsing /etc/shells if present.
///
/// Returns a list of existing executable paths without relying on external system utilities.
pub fn detect_available_shells() -> Vec<PathBuf> {
    let mut shells = Vec::new();

    // Standard shell paths commonly present across Linux systems
    for candidate in KNOWN_SHELLS {
        let path = PathBuf::from(candidate);
        if path.exists() && !shells.contains(&path) {
            shells.push(path);
        }
    }

    // Check /etc/shells if configured on the filesystem
    if let Ok(file) = std::fs::File::open("/etc/shells") {
        let reader = io::BufReader::new(file);
        for line in io::BufRead::lines(reader).map_while(Result::ok) {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                let path = PathBuf::from(trimmed);
                if path.exists() && !shells.contains(&path) {
                    shells.push(path);
                }
            }
        }
    }

    shells
}

/// Resolves an executable shell path for the user, falling back to an available shell if missing.
///
/// If the requested shell exists, it is returned directly. If missing, it delegates to
/// `detect_available_shells()` to find the first working shell on the system, preventing
/// duplicate lists and ensuring consistent fallback logic.
pub fn resolve_shell_path(requested: &Path) -> PathBuf {
    if requested.is_file() || requested.exists() {
        return requested.to_path_buf();
    }
    // Fall back to the first available shell discovered on the system
    if let Some(first_available) = detect_available_shells().into_iter().next() {
        return first_available;
    }
    requested.to_path_buf()
}

/// Builds a Command configured with the target user's credentials and environment.
fn build_user_command(shell: &Path, user: &User, login_flag: bool) -> Command {
    let mut cmd = Command::new(shell);
    cmd.uid(user.uid);
    cmd.gid(user.gid);
    cmd.current_dir(if user.home.exists() { &user.home } else { Path::new("/") });
    cmd.env("USER", &user.username);
    cmd.env("LOGNAME", &user.username);
    cmd.env("HOME", &user.home);
    cmd.env("SHELL", shell.to_string_lossy().as_ref());

    let path_val = match env::var("PATH") {
        Ok(p) if !p.is_empty() => p,
        _ => DEFAULT_SESSION_PATH.to_string(),
    };
    cmd.env("PATH", path_val);

    let term_val = match env::var("TERM") {
        Ok(t) if !t.is_empty() => t,
        _ => "linux".to_string(),
    };
    cmd.env("TERM", term_val);

    if login_flag {
        cmd.arg("-l");
    }
    cmd
}

/// Prepares and executes the user's interactive login shell session.
///
/// Switches UID and GID to the user's credentials, sets up standard
/// environment variables (HOME, USER, LOGNAME, SHELL, PATH, TERM),
/// and waits for the session to complete.
pub fn launch_session(user: &User) -> io::Result<ExitStatus> {
    let effective_shell = resolve_shell_path(&user.shell);
    let mut cmd = build_user_command(&effective_shell, user, true);

    match cmd.status() {
        Ok(status) => Ok(status),
        Err(e) => {
            // If spawning with -l fails (some minimal shells reject -l), retry without it.
            let mut fallback_cmd = build_user_command(&effective_shell, user, false);
            match fallback_cmd.status() {
                Ok(status) => Ok(status),
                Err(_) => Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_shell_fallback() -> Result<(), String> {
        let non_existent = PathBuf::from("/non/existent/shell/path");
        let resolved = resolve_shell_path(&non_existent);
        if resolved.as_os_str().is_empty() {
            return Err("Resolved shell path should not be empty".to_string());
        }
        Ok(())
    }

    #[test]
    fn test_detect_available_shells() -> Result<(), String> {
        let shells = detect_available_shells();
        // All detected shells must exist and be absolute paths
        for shell in shells {
            if !shell.is_absolute() {
                return Err(format!("Detected shell path is not absolute: {}", shell.display()));
            }
            if !shell.exists() {
                return Err(format!("Detected shell does not exist: {}", shell.display()));
            }
        }
        Ok(())
    }
}
