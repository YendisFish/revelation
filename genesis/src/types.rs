use std::{fs::File, io::ErrorKind, mem::needs_drop, path::Path};

pub enum SysConfError {
    IncompatibleFlags {
        first: String,
        second: String,
    },
    BinaryAbsent {
        binary: String,
        error: String,
    },
    DuplicatePriority {
        num: usize,
        first: String,
        second: String,
    },
}

pub struct SysConfig {
    pub services: Vec<SysService>,
}

pub enum SysService {
    Unsafe {
        priority: usize,
        name: String,
        binary: String,
    },
    Careless {
        priority: usize,
        name: String,
        binary: String,
        flags: Vec<String>,
    },
    Safe {
        priority: usize,
        name: String,
        binary: String,
        flags: Vec<String>,
    },
}

pub fn type_check_config(conf: SysConfig) -> Result<SysConfig, SysConfError> {
    let mut seen: Vec<(usize, String)> = Vec::new();
    for service in &conf.services {
        match service {
            SysService::Unsafe { priority, name, binary }
            | SysService::Careless { priority, name, binary, .. }
            | SysService::Safe { priority, name, binary, .. }=> {
                if let Some(dup) = find_duplicate_priorities(&mut seen, (*priority, name)) {
                    return Err(SysConfError::DuplicatePriority {
                        num: dup.0,
                        first: dup.1,
                        second: dup.2,
                    });
                }

                if let Some(err) = does_binary_exist(binary) {
                    return Err(err);
                }
            },
        }

        match service {
            SysService::Careless { flags, .. }
            | SysService::Safe { flags, .. }=> {
                // check for duplicate flags
            },
            _ => {},
        }
    }

    Ok(conf)
}

pub fn find_duplicate_priorities(
    haystack: &mut Vec<(usize, String)>,
    needle: (usize, &String),
) -> Option<(usize, String, String)> {
    for elem in haystack.iter() {
        if needle.0 == elem.0 {
            return Some((needle.0, elem.1.clone(), needle.1.clone()))
        }
    }

    haystack.push((needle.0, needle.1.clone()));
    None
}

pub fn does_binary_exist(bin: &String) -> Option<SysConfError> {
    match File::open(bin.clone()) {
        Ok(_) => None,
        Err(e) if e.kind() == ErrorKind::NotFound => Some(SysConfError::BinaryAbsent {
            binary: bin.clone(),
            error: "Binary not found".to_string(),
        }),
        Err(e) if e.kind() == ErrorKind::PermissionDenied => Some(SysConfError::BinaryAbsent {
            binary: bin.clone(),
            error: "Permission denied to file".to_string(),
        }),
        _ => Some(SysConfError::BinaryAbsent {
            binary: bin.clone(),
            error: "Binary unavailable for unknown reason".to_string()
        })
    }
}
