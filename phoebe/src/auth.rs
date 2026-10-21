// Copyright 2026 Revelation OS Project
//
// Phoebe Greeter - Authentication & Cryptography
//
// This module outsources cryptographic operations (hashing and verification)
// to industry-standard, audited libraries from crates.io (pwhash, argon2, subtle)
// adhering to the rule: "Outsource for security. Do not roll your own hashing or security."

use argon2::Argon2;
use password_hash::{phc::PasswordHash, PasswordVerifier};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;

/// Hashes a plaintext password for a new user account.
///
/// We outsource this to `pwhash::sha512_crypt`, generating standard Linux `$6$`
/// SHA-512 crypt hashes natively understood by standard Linux shadow databases.
/// All errors are mapped into `io::Error` without unwrap or panic.
pub fn hash_password(password: &str) -> io::Result<String> {
    match pwhash::sha512_crypt::hash(password) {
        Ok(hash) => Ok(hash),
        Err(e) => Err(io::Error::other(format!(
            "Failed to hash password via pwhash: {}",
            e
        ))),
    }
}

/// Verifies a user-supplied password against a stored shadow entry.
///
/// Supports:
/// 1. Standard Linux crypt hashes ($6$ SHA-512, $5$ SHA-256, $2y$ bcrypt, $1$ MD5)
///    via `pwhash::unix::verify`.
/// 2. Argon2id hashes ($argon2id$) via the official `argon2` crate.
/// 3. Immediate rejection of locked, disabled, or empty shadow entries (!, *, empty).
/// 4. Fallback to constant-time plaintext comparison (only if bootstrap account is unhashed).
pub fn verify_password(input_password: &str, stored_hash: &str) -> bool {
    let trimmed = stored_hash.trim();

    // Accounts locked, disabled, or empty must never authenticate
    if trimmed.is_empty() || trimmed.starts_with('!') || trimmed.starts_with('*') {
        return false;
    }

    // Modern Argon2id hashes
    if trimmed.starts_with("$argon2") {
        if let Ok(parsed_hash) = PasswordHash::new(trimmed) {
            return Argon2::default()
                .verify_password(input_password.as_bytes(), &parsed_hash)
                .is_ok();
        }
        return false;
    }

    // Standard Linux crypt hashes ($6$, $5$, $2y$, $1$, etc.)
    if trimmed.starts_with('$') {
        return pwhash::unix::verify(input_password, trimmed);
    }

    // Constant-time plaintext comparison for non-crypt bootstrap setups
    let a = input_password.as_bytes();
    let b = trimmed.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

/// Reads the shadow database (/etc/shadow) into a map of username -> password_hash.
///
/// If the file does not exist, returns an empty map instead of failing,
/// allowing initial setup to proceed seamlessly.
pub fn load_shadow_map(shadow_path: &Path) -> io::Result<HashMap<String, String>> {
    let file = match File::open(shadow_path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(e),
    };

    let reader = BufReader::new(file);
    let mut map = HashMap::new();

    for line_result in reader.lines() {
        let line = line_result?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split(':').collect();
        if let (Some(user), Some(hash)) = (parts.first(), parts.get(1)) {
            map.insert((*user).to_string(), (*hash).to_string());
        }
    }

    Ok(map)
}

/// Appends a new user's password record to /etc/shadow.
///
/// Formats standard 9-field shadow entry:
/// `username:hash:days_since_1970:0:99999:7:::\n`
pub fn write_shadow_entry(
    shadow_path: &Path,
    username: &str,
    password_hash: &str,
) -> io::Result<()> {
    if let Some(parent) = shadow_path.parent().filter(|p| !p.as_os_str().is_empty() && !p.exists()) {
        let _ = std::fs::create_dir_all(parent);
    }

    let days = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() / 86400,
        Err(_) => 0,
    };

    let line = format!("{}:{}:{}:0:99999:7:::\n", username, password_hash, days);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(shadow_path)?;

    file.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha512_crypt_hash_and_verification() -> Result<(), String> {
        let password = "SuperSecretPassword2026!";
        let hashed = match hash_password(password) {
            Ok(h) => h,
            Err(e) => return Err(format!("Hashing failed: {}", e)),
        };

        // Standard Linux SHA-512 crypt starts with $6$
        if !hashed.starts_with("$6$") {
            return Err(format!("Expected $6$ prefix for sha512_crypt, got: {}", hashed));
        }

        // Verify positive authentication
        if !verify_password(password, &hashed) {
            return Err("Verification failed for valid password".to_string());
        }

        // Verify rejection of incorrect password
        if verify_password("WrongPassword123", &hashed) {
            return Err("Verification incorrectly passed for wrong password".to_string());
        }

        Ok(())
    }

    #[test]
    fn test_locked_and_empty_accounts() -> Result<(), String> {
        if verify_password("pass", "!$6$salt$hash") {
            return Err("Exclamation locked account should not verify".to_string());
        }
        if verify_password("pass", "*") {
            return Err("Star disabled account should not verify".to_string());
        }
        if verify_password("pass", "") {
            return Err("Empty hash should not verify".to_string());
        }
        Ok(())
    }
}
