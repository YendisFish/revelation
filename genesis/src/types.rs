use std::{fs::{self, File}, io::ErrorKind, mem::needs_drop, path::Path};

use serde::{Deserialize, Deserializer};

#[derive(Debug)]
pub enum SysConfError {
    IncompatibleFlags {
        service: String,
        flag: String,
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

#[derive(Debug, Clone)]
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
    // just a vector keeping track of which processes and their requested priority have been seen
    let mut seen: Vec<(usize, String)> = Vec::new();
    for service in &conf.services {
        match service {
            SysService::Unsafe { priority, name, binary }
            | SysService::Careless { priority, name, binary, .. }
            | SysService::Safe { priority, name, binary, .. }=> {
                // check to see if any processes are requesting the same priority
                if let Some(dup) = find_duplicate_priorities(&mut seen, (*priority, name)) {
                    return Err(SysConfError::DuplicatePriority {
                        num: dup.0,
                        first: dup.1,
                        second: dup.2,
                    });
                }

                // check to see if system actually has the requested binary
                if let Some(err) = does_binary_exist(binary) {
                    return Err(err);
                }
            },
        }

        // THIS MATCH NEEDS TO MAKE SURE THE FLAGS IN THE CONFIG MAKE SENSE
        match service {
            SysService::Careless { flags, .. } => {

            },
            SysService::Safe {  flags, .. } => {

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
    // check if the values in needle appear in the haystack vector
    for elem in haystack.iter() {
        if needle.0 == elem.0 {
            return Some((needle.0, elem.1.clone(), needle.1.clone()))
        }
    }

    // push the needle to the haystack if it is unique
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

#[derive(Deserialize)]
struct RawSafe {
    priority: usize,
    name: String,
    bin: String,
    #[serde(default)]
    needs: Vec<String>,
}

#[derive(Deserialize)]
struct RawCareless {
    priority: usize,
    name: String,
    bin: String,
    #[serde(default)]
    needs: Vec<String>,
}

#[derive(Deserialize)]
struct RawUnsafe {
    priority: usize,
    name: String,
    bin: String,
}

#[derive(Deserialize)]
struct RawSysConfig {
    #[serde(default)]
    safe: Vec<RawSafe>,
    #[serde(default)]
    careless: Vec<RawCareless>,
    #[serde(default)]
    r#unsafe: Vec<RawUnsafe>,
}

// I genuinely have no clue what this code does!
impl<'de> Deserialize<'de> for SysConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSysConfig::deserialize(deserializer)?;
        let mut services = Vec::new();

        for s in raw.safe {
            services.push(SysService::Safe {
                priority: s.priority,
                name: s.name,
                binary: s.bin,
                flags: s.needs,
            });
        }

        for c in raw.careless {
            services.push(SysService::Careless {
                priority: c.priority,
                name: c.name,
                binary: c.bin,
                flags: c.needs,
            });
        }

        for u in raw.r#unsafe {
            services.push(SysService::Unsafe {
                priority: u.priority,
                name: u.name,
                binary: u.bin,
            });
        }

        Ok(SysConfig { services })
    }
}

pub fn parse_config() -> Option<SysConfig> {
    // safely try and parse/open the config then return it
    let Some(content) = fs::read_to_string("/genesis.toml").ok() else { return None; };
    return toml::from_str::<SysConfig>(&content.as_str()).ok();
}
