use std::{fs, path::Path, sync::Arc};

use crate::io::Logger;

pub fn dump_dir(logger: Arc<Logger>, dir: impl Into<String>) {
    let p = dir.into();
    let Ok(entries) = fs::read_dir(p.clone()) else {
        logger.error(&format!("Failed to dump directory {}", p.clone()));
        return;
    };

    logger.write(&format!("Dumping directory {}", p.clone()));
    for entry in entries.flatten() {
        logger.write(&format!("File: {}", entry.path().to_string_lossy().to_string()));
    }
}
