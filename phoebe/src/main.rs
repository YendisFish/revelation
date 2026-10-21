// Copyright 2026 Revelation OS Project
//
// Phoebe Greeter - Text-Based Greeter for Revelation Linux Userspace
//
// Phoebe is a simple, robust, self-contained greeter.
// It contains no unsafe code, no raw pointers, and no assumptions
// about GNU or other system binaries existing on the host system.
//
// Cryptographic hashing and verification are outsourced to vetted libraries
// from crates.io (pwhash, argon2, subtle), ensuring high security without rolling
// custom crypto.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod auth;
mod session;
mod user;

use std::env;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use user::{NewUserParams, User};

fn print_banner() {
    println!("\x1b[1;36m====================================================\x1b[0m");
    println!("\x1b[1;37m                 P H O E B E                        \x1b[0m");
    println!("\x1b[0;36m          Revelation Linux Userspace Greeter        \x1b[0m");
    println!("\x1b[1;36m====================================================\x1b[0m\n");
}

/// Reads a line from stdin after displaying a prompt; returns None on EOF (Ctrl+D).
fn prompt_line(prompt: &str) -> io::Result<Option<String>> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(line.trim_end_matches(&['\r', '\n'][..]).to_string())),
        Err(e) => Err(e),
    }
}

/// Reads a password from stdin and clears the line with ANSI escapes to protect credentials.
fn prompt_password(prompt: &str) -> io::Result<Option<String>> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) => Ok(None),
        Ok(_) => {
            println!("\x1b[1A\x1b[2K{}********", prompt);
            let _ = io::stdout().flush();
            Ok(Some(line.trim_end_matches(&['\r', '\n'][..]).to_string()))
        }
        Err(e) => Err(e),
    }
}

/// Holds filesystem paths for user configuration databases (configurable via env vars).
struct ConfigPaths {
    passwd: PathBuf,
    shadow: PathBuf,
    group: PathBuf,
}

impl ConfigPaths {
    fn from_env() -> Self {
        let resolve = |var: &str, def: &str| match env::var(var) {
            Ok(v) if !v.is_empty() => PathBuf::from(v),
            _ => PathBuf::from(def),
        };
        ConfigPaths {
            passwd: resolve("PHOEBE_PASSWD", user::DEFAULT_PASSWD_PATH),
            shadow: resolve("PHOEBE_SHADOW", user::DEFAULT_SHADOW_PATH),
            group: resolve("PHOEBE_GROUP", user::DEFAULT_GROUP_PATH),
        }
    }
}

/// Walkthrough wizard for initial account creation if no regular users exist.
fn run_account_creation_wizard(paths: &ConfigPaths) -> io::Result<Option<User>> {
    println!("\x1b[1;33m[!] No regular user accounts found on this system.\x1b[0m");
    println!("Revelation requires an account to log in and begin a session.\n");

    let answer = match prompt_line("Would you like to create a user account now? [Y/n]: ")? {
        Some(a) => a,
        None => return Ok(None),
    };

    let trimmed = answer.trim();
    if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("y") && !trimmed.eq_ignore_ascii_case("yes") {
        println!("Skipping user account creation.\n");
        return Ok(None);
    }

    println!("\n--- New User Setup ---");
    let username = loop {
        let input = match prompt_line("Username: ")? {
            Some(u) => u.trim().to_string(),
            None => return Ok(None),
        };
        match user::validate_username(&input) {
            Ok(_) => match user::find_user_by_name(&paths.passwd, &input)? {
                Some(_) => println!("\x1b[1;31mError: User '{}' already exists.\x1b[0m", input),
                None => break input,
            },
            Err(msg) => println!("\x1b[1;31mError: {}\x1b[0m", msg),
        }
    };

    let gecos = prompt_line("Full Name (optional): ")?.unwrap_or_default().trim().to_string();

    // Query the filesystem for installed shells and present options to the user.
    // Dynamically list detected shells, explicitly noting when none are found.
    let detected_shells = session::detect_available_shells();
    if detected_shells.is_empty() {
        println!("Available shells: None detected on the system.");
    } else {
        let shell_names: Vec<String> = detected_shells.iter().map(|p| p.display().to_string()).collect();
        println!("Available shells: {}", shell_names.join(", "));
    }

    // Default to the first detected shell, or standard /bin/sh if none detected yet
    let default_shell = detected_shells
        .first()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| session::FALLBACK_SHELL.to_string());

    let shell = loop {
        let prompt_str = format!("Login Shell [{}]: ", default_shell);
        let input = prompt_line(&prompt_str)?
            .unwrap_or_default()
            .trim()
            .to_string();

        let chosen_path = if input.is_empty() {
            PathBuf::from(&default_shell)
        } else {
            PathBuf::from(input)
        };

        if !chosen_path.is_absolute() {
            println!("\x1b[1;31mError: Shell path must be absolute (e.g. /bin/sh).\x1b[0m");
            continue;
        }

        // Check if the selected shell binary exists on the filesystem.
        // NOTE: For now, users are allowed to proceed with a shell that does not exist
        // (to support bootstrap environments where packages/shells are provisioned later).
        // Eventually, Phoebe should strictly disallow non-existent shells and enforce
        // that only valid, installed shells can be selected.
        if !chosen_path.exists() {
            println!(
                "\x1b[1;33mWarning: Shell '{}' does not currently exist on the system.\x1b[0m",
                chosen_path.display()
            );

            let confirm = prompt_line("Are you sure you want to proceed with this shell? [y/N]: ")?
                .unwrap_or_default()
                .trim()
                .to_string();

            if confirm.eq_ignore_ascii_case("y") || confirm.eq_ignore_ascii_case("yes") {
                break chosen_path;
            } else {
                // User rejected using the non-existent shell; re-prompt until valid or confirmed
                continue;
            }
        }

        break chosen_path;
    };

    let password = loop {
        let p1 = match prompt_password("New Password: ")? {
            Some(p) if !p.is_empty() => p,
            Some(_) => { println!("\x1b[1;31mPassword cannot be empty.\x1b[0m"); continue; }
            None => return Ok(None),
        };
        let p2 = match prompt_password("Confirm Password: ")? {
            Some(p) => p,
            None => return Ok(None),
        };
        if p1 != p2 {
            println!("\x1b[1;31mPasswords do not match. Please try again.\x1b[0m");
            continue;
        }
        break p1;
    };

    let uid = user::find_next_regular_uid(&paths.passwd)?;
    let params = NewUserParams {
        username: username.clone(),
        uid,
        gid: uid,
        gecos,
        home: PathBuf::from(format!("/home/{}", username)),
        shell,
    };

    let new_user = user::create_user_entry(&paths.passwd, &paths.group, &params)?;
    let password_hash = auth::hash_password(&password)?;
    auth::write_shadow_entry(&paths.shadow, &username, &password_hash)?;

    println!("\x1b[1;32mUser account '{}' successfully created!\x1b[0m\n", username);
    Ok(Some(new_user))
}

/// The main greeter login loop.
/// Shows a login page, authenticates credentials via /etc/shadow,
/// and launches the user's default shell with their UID and GID permissions.
fn run_login_loop(paths: &ConfigPaths) {
    loop {
        print_banner();

        let username = match prompt_line("revelation login: ") {
            Ok(Some(u)) if !u.trim().is_empty() => u.trim().to_string(),
            Ok(Some(_)) => continue,
            Ok(None) => {
                println!();
                thread::sleep(Duration::from_millis(500));
                continue;
            }
            Err(e) => {
                println!("I/O Error: {}", e);
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        let password = match prompt_password("Password: ") {
            Ok(Some(p)) => p,
            Ok(None) => { println!(); continue; }
            Err(e) => {
                println!("I/O Error: {}", e);
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        let user_entry = match user::find_user_by_name(&paths.passwd, &username) {
            Ok(opt) => opt,
            Err(e) => {
                println!("Error reading user database: {}", e);
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        let shadow_map = match auth::load_shadow_map(&paths.shadow) {
            Ok(map) => map,
            Err(e) => {
                println!("Error reading shadow database: {}", e);
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        let authenticated = match (user_entry.as_ref(), shadow_map.get(&username)) {
            (Some(_user), Some(hash)) => auth::verify_password(&password, hash),
            (Some(_user), None) => false,
            (None, _) => {
                // Prevent user enumeration timing leaks by running dummy verification
                let _ = pwhash::unix::verify(&password, "$6$revelationsalt$dummyplaceholderhash");
                false
            }
        };

        if !authenticated {
            thread::sleep(Duration::from_secs(2));
            println!("\x1b[1;31mLogin incorrect\x1b[0m\n");
            continue;
        }

        let user = match user_entry {
            Some(u) => u,
            None => continue,
        };

        let effective_shell = session::resolve_shell_path(&user.shell);
        println!("\x1b[1;32mWelcome to Revelation Linux, {}!\x1b[0m", user.username);
        if effective_shell != user.shell {
            println!(
                "\x1b[0;33mNotice: Configured shell '{}' not found, falling back to '{}'.\x1b[0m",
                user.shell.display(),
                effective_shell.display()
            );
        }
        println!(
            "Starting session for {} (UID: {}, GID: {}, Shell: {})...\n",
            user.username,
            user.uid,
            user.gid,
            effective_shell.display()
        );

        match session::launch_session(&user) {
            Ok(status) => {
                println!("\n[Phoebe] Session ended for {} (exit: {}).", user.username, status);
                thread::sleep(Duration::from_millis(800));
            }
            Err(e) => {
                println!(
                    "\x1b[1;31m[Phoebe] Failed to launch shell '{}' for {}: {}\x1b[0m",
                    effective_shell.display(),
                    user.username,
                    e
                );
                thread::sleep(Duration::from_secs(2));
            }
        }
        println!("\n\n");
    }
}

fn main() {
    let paths = ConfigPaths::from_env();

    // Requirement 1: Check if there are user accounts on the Linux system.
    // If not, walk the user through creating a user account optionally.
    match user::has_regular_users(&paths.passwd) {
        Ok(false) => {
            print_banner();
            if let Err(e) = run_account_creation_wizard(&paths) {
                println!("Account wizard error: {}", e);
            }
        }
        Ok(true) => {}
        Err(e) => println!("Warning: Could not check existing users: {}", e),
    }

    // Requirement 2 & 3: Display greeter login prompt and launch sessions.
    run_login_loop(&paths);
}
