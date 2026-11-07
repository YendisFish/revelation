use std::{fs, sync::Arc, thread, time::Duration};

use oracle::io::Logger;
use serde::Deserialize;
use tokio::time::Instant;
use gpt::{disk::LogicalBlockSize, GptConfig};

pub struct DeviceManager {

}

#[derive(Deserialize)]
pub struct BootToml {
    pub uuid: String,
}

pub fn wait_for_partuuid(logger: Arc<Logger>) -> Option<String> {
    let timeout_loops = 100; // 10 seconds total

    let Some(content) = fs::read_to_string("/boot.toml").ok() else {
        logger.write("ERROR: Could not read /boot.toml");
        return None;
    };

    let Some(info) = toml::from_str::<BootToml>(&content.as_str()).ok() else {
        logger.write("ERROR: Could not parse /boot.toml");
        return None;
    };

    logger.write(&format!("Looking for PARTUUID: {}", info.uuid));

    for _ in 0..timeout_loops {
        if let Ok(entries) = fs::read_dir("/sys/class/block") {
            for entry in entries.flatten() {
                let dev_name = entry.file_name().into_string().unwrap();
                let dev_path = format!("/dev/{}", dev_name);

                // 1. Verify the node actually exists in user-space
                if !std::path::Path::new(&dev_path).exists() {
                    logger.write(&format!("WARNING: {} is in sysfs but missing from /dev! Is devtmpfs mounted?", dev_path));
                    continue; // Skip trying to open a non-existent file
                }

                // 2. Open the device and attempt to read the GPT header
                // Note: This will naturally (and correctly) fail on partitions like /dev/vda1
                // because partitions do not have GPT headers; only the parent disk (/dev/vda) does.
                // ... inside your wait_for_partuuid loop ...
                let mut cfg = gpt::GptConfig::new();
                match cfg.open(&dev_path) {
                    Ok(disk) => {
                        for (part_id, partition) in disk.partitions() {
                            if partition.part_guid.to_string().eq_ignore_ascii_case(&info.uuid) {
                                let part_path = format!("{}{}", dev_path, part_id);
                                logger.write(&format!("SUCCESS: Found UUID on {}, targeting partition {}", dev_name, part_path));
                                return Some(part_path);
                            }
                        }
                    }
                    Err(e) => {
                        // Log the error to see why it's failing
                        logger.write(&format!("Skipping {}: Not a valid GPT disk ({})", dev_path, e));
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    logger.write("TIMEOUT: Could not find PARTUUID");
    None
}
