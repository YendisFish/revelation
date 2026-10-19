use crate::types::{SysConfig, SysService};

pub fn begin_lifecycle(mut conf: SysConfig) {
    conf.services.sort_by_key(|s| {
        match s {
            SysService::Unsafe { priority, .. }
            | SysService::Careless { priority, .. }
            | SysService::Safe { priority, .. } => *priority
        }
    });

    for service in conf.services.iter() {
        match service {
            SysService::Unsafe { name, binary, .. } => {

            },
            _ => {},
        }
    }
}
