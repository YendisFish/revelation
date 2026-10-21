// Copyright 2026 Revelation OS Project
//
// Phoebe Greeter - User Account Management
//
// This module handles detecting, reading, and creating user accounts
// directly on the filesystem (/etc/passwd and /etc/shadow) without invoking
// external binaries or utilities.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_PASSWD_PATH: &str = "/etc/passwd";
pub const DEFAULT_SHADOW_PATH: &str = "/etc/shadow";
pub const DEFAULT_GROUP_PATH: &str = "/etc/group";

pub const FIRST_REGULAR_UID: u32 = 1000;
pub const LAST_REGULAR_UID: u32 = 60000;
pub const NOBODY_UID: u32 = 65534;

/// Represents a user account parsed from /etc/passwd.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub gecos: String,
    pub home: PathBuf,
    pub shell: PathBuf,
}

impl User {
    /// Determines whether this account qualifies as a regular interactive user account.
    /// Filters out system daemons (UID < 1000, nobody, or non-login shells).
    pub fn is_regular_user(&self) -> bool {
        if self.uid < FIRST_REGULAR_UID || self.uid == NOBODY_UID || self.uid > LAST_REGULAR_UID {
            return false;
        }
        let shell_str = self.shell.to_string_lossy();
        !shell_str.ends_with("/nologin") && !shell_str.ends_with("/false")
    }
}

/// Parses a single line from an /etc/passwd formatted file:
/// `username:password:uid:gid:gecos:home_dir:shell`
pub fn parse_passwd_line(line: &str) -> Option<User> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let fields: Vec<&str> = trimmed.split(':').collect();
    if fields.len() < 7 {
        return None;
    }

    Some(User {
        username: (*fields.first()?).to_string(),
        uid: fields.get(2)?.parse::<u32>().ok()?,
        gid: fields.get(3)?.parse::<u32>().ok()?,
        gecos: (*fields.get(4)?).to_string(),
        home: PathBuf::from(*fields.get(5)?),
        shell: PathBuf::from(*fields.get(6)?),
    })
}

/// Reads all user accounts from the specified passwd file path.
pub fn load_users_from_path(passwd_path: &Path) -> io::Result<Vec<User>> {
    let file = match File::open(passwd_path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let mut users = Vec::new();
    for line in BufReader::new(file).lines() {
        if let Some(user) = parse_passwd_line(&line?) {
            users.push(user);
        }
    }
    Ok(users)
}

/// Finds a user by their username in the given passwd file.
pub fn find_user_by_name(passwd_path: &Path, username: &str) -> io::Result<Option<User>> {
    Ok(load_users_from_path(passwd_path)?.into_iter().find(|u| u.username == username))
}

/// Checks if there are any regular interactive user accounts configured.
pub fn has_regular_users(passwd_path: &Path) -> io::Result<bool> {
    Ok(load_users_from_path(passwd_path)?.iter().any(|u| u.is_regular_user()))
}

/// Determines the next available UID for a new regular user (>= 1000).
pub fn find_next_regular_uid(passwd_path: &Path) -> io::Result<u32> {
    let highest = load_users_from_path(passwd_path)?
        .into_iter()
        .filter(|u| u.uid >= FIRST_REGULAR_UID && u.uid < NOBODY_UID)
        .map(|u| u.uid)
        .max()
        .unwrap_or(FIRST_REGULAR_UID.saturating_sub(1));

    Ok(highest.saturating_add(1))
}

/// Validates that a requested username matches standard Linux conventions.
pub fn validate_username(username: &str) -> Result<(), String> {
    if username.is_empty() {
        return Err("Username cannot be empty.".to_string());
    }
    if username.len() > 32 {
        return Err("Username is too long (max 32 characters).".to_string());
    }

    let mut chars = username.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first == '_' => {}
        _ => return Err("Username must start with a lowercase letter or underscore.".to_string()),
    }

    for ch in chars {
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '_' && ch != '-' {
            return Err(format!(
                "Invalid character '{}' in username. Use lowercase letters, numbers, '-', or '_'.",
                ch
            ));
        }
    }
    Ok(())
}

/// Parameters for creating a new user account.
#[derive(Debug, Clone)]
pub struct NewUserParams {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub gecos: String,
    pub home: PathBuf,
    pub shell: PathBuf,
}

/// Creates a user entry in /etc/passwd and /etc/group without invoking external tools.
pub fn create_user_entry(
    passwd_path: &Path,
    group_path: &Path,
    params: &NewUserParams,
) -> io::Result<User> {
    if let Some(parent) = passwd_path.parent().filter(|p| !p.as_os_str().is_empty() && !p.exists()) {
        fs::create_dir_all(parent)?;
    }

    let passwd_line = format!(
        "{}:x:{}:{}:{}:{}:{}\n",
        params.username,
        params.uid,
        params.gid,
        params.gecos,
        params.home.to_string_lossy(),
        params.shell.to_string_lossy()
    );

    let mut passwd_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(passwd_path)?;
    passwd_file.write_all(passwd_line.as_bytes())?;

    // Record group entry if group file can be opened
    let group_line = format!("{}:x:{}:\n", params.username, params.gid);
    if let Ok(mut group_file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(group_path)
    {
        let _ = group_file.write_all(group_line.as_bytes());
    }

    // Provision home directory
    if !params.home.as_os_str().is_empty() && !params.home.exists() {
        let _ = fs::create_dir_all(&params.home);
    }

    Ok(User {
        username: params.username.clone(),
        uid: params.uid,
        gid: params.gid,
        gecos: params.gecos.clone(),
        home: params.home.clone(),
        shell: params.shell.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_user_line() -> Result<(), String> {
        let line = "yendy:x:1000:1000:Yendy Admin:/home/yendy:/bin/sh";
        let user = match parse_passwd_line(line) {
            Some(u) => u,
            None => return Err("Failed to parse valid passwd line".to_string()),
        };

        if user.username != "yendy" || user.uid != 1000 || user.gid != 1000 {
            return Err("User fields mismatch".to_string());
        }
        if user.home != PathBuf::from("/home/yendy") || user.shell != PathBuf::from("/bin/sh") {
            return Err("Path fields mismatch".to_string());
        }
        if !user.is_regular_user() {
            return Err("Expected regular user".to_string());
        }
        Ok(())
    }

    #[test]
    fn test_parse_system_user_detected_not_regular() -> Result<(), String> {
        let root = match parse_passwd_line("root:x:0:0:root:/root:/bin/sh") {
            Some(u) => u,
            None => return Err("Failed to parse root".to_string()),
        };
        if root.is_regular_user() {
            return Err("Root should not be regular user".to_string());
        }

        let daemon = match parse_passwd_line("daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin") {
            Some(u) => u,
            None => return Err("Failed to parse daemon".to_string()),
        };
        if daemon.is_regular_user() {
            return Err("Daemon should not be regular user".to_string());
        }
        Ok(())
    }

    #[test]
    fn test_username_validation() -> Result<(), String> {
        if validate_username("alice").is_err() || validate_username("dev_user-1").is_err() {
            return Err("Valid usernames rejected".to_string());
        }
        if validate_username("").is_ok() || validate_username("1invalid").is_ok() || validate_username("Upper").is_ok() {
            return Err("Invalid usernames accepted".to_string());
        }
        Ok(())
    }
}
