use std::{collections::HashMap, fmt::format, fs::{self, OpenOptions}, io::{pipe, stdout}, iter::Map, os::fd::{AsFd, AsRawFd}, path::Path, process::Stdio, sync::Arc, time::Duration};

use libcontainer::{container::{self, Container, builder::ContainerBuilder}, oci_spec::{self, runtime::{LinuxBuilder, LinuxNamespaceBuilder, LinuxNamespaceType, MountBuilder, ProcessBuilder, RootBuilder, Spec, SpecBuilder}}, syscall::syscall::SyscallType};
use oracle::{debug, io::Logger};
use tokio::{process::{Child, ChildStdout, Command}, runtime::Builder, sync::{Mutex, mpsc::{Receiver, Sender, channel}}, task::JoinError, time::sleep};

use crate::types::{SysConfig, SysService};

pub enum EngineError {
    TokioFailure(Arc<dyn std::error::Error + Send + Sync>),
    TokioJoin(JoinError),
    TokioCommandError(Arc<dyn std::error::Error + Send + Sync>),
    TokioPidFailure(String),
    Io(Arc<dyn std::error::Error + Send + Sync>),
    AppNotFound(String),
    ContainerGenerationError,
    ContainerStartError,
    ContainerPidError(String),
}

pub fn setup_lifecycle(mut conf: SysConfig, eng: Arc<GenesisEngine>) {
    // sort the services vector by priority so we can iterate through it correctly
    conf.services.sort_by_key(|s| {
        match s {
            SysService::Unsafe { priority, .. }
            | SysService::Careless { priority, .. }
            | SysService::Safe { priority, .. } => *priority
        }
    });

    // iterate through the services and dispatch them to the engine
    for service in conf.services.iter() {
        match service {
            SysService::Unsafe { name, binary, .. } => {
                // binary can literally just be started as a detached process
                eng.push_task(service.clone());
            },
            SysService::Safe { priority, name, binary, flags } => {
                // binary needs to be started in a container
                eng.push_task(service.clone());
            }
            _ => {},
        }
    }
}

pub struct GenesisProcess {
    pub sys_service: SysService,
    pub inf: GenesisProcInfo,
}

pub enum GenesisProcInfo {
    Cmd {
       pid: u32,
       child: Child,
    },
    Container {
        container: Container,
    },
}

pub struct GenesisEngine {
    pub proc_queue: (Mutex<Receiver<SysService>>, Mutex<Arc<Sender<SysService>>>),
    pub procs: Mutex<HashMap<u32, GenesisProcess>>,
    logger: Arc<Logger>,
}

impl GenesisEngine {
    pub fn new(log: Arc<Logger>) -> Arc<GenesisEngine> {
        // open async channel for queuing processes to open
        let chan = channel(20);

        return Arc::new(GenesisEngine {
            proc_queue: (
                Mutex::new(chan.1),
                Mutex::new(Arc::new(chan.0))
            ), // deconstruct channel into custom tuple
            procs: Mutex::new(HashMap::new()),
            logger: log,
        })
    }

    pub fn start_engine(self: Arc<Self>) -> Result<(), EngineError> {
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| EngineError::TokioFailure(Arc::new(e)))?;

        // start a blocking async task that concurrently handles process lifecycles
        let ret: Result<(), EngineError> = rt.block_on(async {
            // capture any errors that get returned from the engine
            tokio::try_join!(
                self.clone().handle_ipc(),
                self.clone().handle_unsafe_lifecycles(),
                self.clone().handle_safe_lifecycles(),
            )?;

            return Ok(());
        });

        return ret;
    }

    async fn handle_ipc(self: Arc<Self>) -> Result<(), EngineError> {
        // handle incoming requests or data on the io_uring IPC ring buffer
        loop {
            sleep(Duration::from_millis(100)).await
        }
    }

    async fn handle_unsafe_lifecycles(self: Arc<Self>) -> Result<(), EngineError> {
        // look for any queued processes, and start them
        loop {
            let proc_req = {
                let mut p_q = self.proc_queue.0.lock().await;
                p_q.recv().await
            };
            let Some(proc_req) = proc_req else { continue; };

            match proc_req.clone() {
                // we only want to look at unsafe processes
                SysService::Unsafe { priority, name, binary } => {
                    let child = match unsafe_command(binary,  vec![])?.spawn() {
                        Ok(c) => c,
                        Err(e) => return Err(EngineError::TokioCommandError(Arc::new(e))),
                    };

                    /*
                     * Here we capture the process pid and then
                     * put it into struct holding the process info
                     */
                    let Some(pid) = child.id() else {
                        return Err(EngineError::TokioPidFailure(name));
                    };

                    let genesis_proc = GenesisProcess {
                        sys_service: proc_req.clone(),
                        inf: GenesisProcInfo::Cmd { pid, child }
                    };

                    // lock the process hashmap and insert our new process
                    let mut procs = self.procs.lock().await;
                    procs.insert(pid, genesis_proc);
                }
                _ => {
                    // for now we recycle the request
                    let p_q = self.proc_queue.1.lock().await;
                    _ = p_q.send(proc_req).await;
                }
            }

            /*
             * here we need to cycle through all unsafe processes and
             * watch for them to end or be killed. We then update our
             * hashmap accordingly
             */

            sleep(Duration::from_millis(100)).await
        }
    }

    async fn handle_safe_lifecycles(self: Arc<Self>) -> Result<(), EngineError> {
        loop {
            let proc_req = {
                let mut p_q = self.proc_queue.0.lock().await;
                p_q.recv().await
            };
            let Some(proc_req) = proc_req else { continue; };

            match proc_req.clone() {
                SysService::Safe { priority, name, binary, flags } => {
                    let container = start_or_init_safe(self.logger.clone(), proc_req.clone())?;

                    let genesis_proc = GenesisProcess {
                        sys_service: proc_req.clone(),
                        inf: GenesisProcInfo::Container { container: container.clone() }
                    };

                    let pid = match container.pid() {
                        Some(pid) => pid,
                        None => return Err(EngineError::ContainerPidError("Could not get container pid".to_string()))
                    };

                    // lock the process hashmap and insert our new process
                    let mut procs = self.procs.lock().await;
                    procs.insert(pid.as_raw() as u32, genesis_proc);
                },
                _ => {
                    // recycle queue item if it doesn't belong here
                    let p_q = self.proc_queue.1.lock().await;
                    _ = p_q.send(proc_req).await;
                },
            }

            /*
             * Iterate through running containers and handle them
             */

            sleep(Duration::from_millis(100)).await
        }
    }

    pub fn push_task(self: &Arc<Self>, task: SysService) {
        let queue = self.proc_queue.1.blocking_lock();
        _ = queue.blocking_send(task);
    }
}

fn unsafe_command(bin: String, args: Vec<String>) -> Result<Command, EngineError> {
    let mut cmd = Command::new(bin);

    // attach stdio
    let stdio = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/console")
        .map_err(|e| EngineError::Io(Arc::new(e)))?;

    cmd.stdin(Stdio::from(stdio.try_clone().map_err(|e| EngineError::Io(Arc::new(e)))?));
    cmd.stdout(Stdio::from(stdio));

    // return command
    Ok(cmd)
}

fn start_or_init_safe(logger: Arc<Logger>, conf_entry: SysService) -> Result<Container, EngineError> {
    match conf_entry {
        SysService::Safe { priority, name, binary, flags } => {
            let bin = Path::new(&binary);
            if !Path::exists(&bin) {
                // error if the requested binary doesn't exist
                return Err(EngineError::AppNotFound(binary.clone()));
            }

            let string_path = format!("/apps/{}/", name);
            let container_root = Path::new(&string_path);

            if !container_root.exists() {
                // create container root
                fs::create_dir_all(container_root.join("rootfs/")).map_err(|e| {
                    eprintln!("{}", e);
                    EngineError::ContainerGenerationError
                })?;

                // create dirs which libcontainer expects
                let base_dirs = ["proc", "sys", "dev", "dev/pts", "dev/shm"];
                for dir in base_dirs.iter() {
                    fs::create_dir_all(container_root.join("rootfs/").join(dir)).map_err(|e| {
                        eprintln!("Failed to create mount target {}: {}", dir, e);
                        EngineError::ContainerGenerationError
                    })?;
                }

                // create faux binary
                let target_bin = container_root.join("rootfs/").join(&name);
                fs::File::create(&target_bin).map_err(|e| {
                    eprintln!("Failed to create bind mount target: {}", e);
                    EngineError::ContainerGenerationError
                })?;
            }

            // generate, or regenerate, oci config
            let spec = generate_container_oci_spec(&container_root.join("rootfs/"), &name, &binary)?;
            spec.save(container_root.join("config.json"))
                .map_err(|_| EngineError::ContainerGenerationError)?;

            // attach stdio
            // this shouldn't be permanent, eventually there should be a journaling system
            let stdio = OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/console")
                .map_err(|e| EngineError::Io(Arc::new(e)))?;
            let stdio_clone = stdio.try_clone().map_err(|e| EngineError::Io(Arc::new(e)))?;

            // build container
            let mut container = ContainerBuilder::new(name.clone(), SyscallType::Linux)
                .with_root_path(string_path.clone())
                .map_err(|_| EngineError::ContainerGenerationError)?
                .with_stdout(stdio)
                .with_stdin(stdio_clone)
                .as_init(string_path.clone())
                .with_systemd(false)
                .build()
                .map_err(|e| {
                    eprintln!("{}", e);
                    EngineError::ContainerGenerationError
                })?;

            // run container
            container.start().map_err(|_| EngineError::ContainerStartError)?;
            return Ok(container);
        },
        _ => {},
    }

    Err(EngineError::ContainerGenerationError)
}

fn generate_container_oci_spec(rootfs: &Path, name: &String, binary: &String) -> Result<Spec, EngineError> {
    // process that will run in the container
    let process = ProcessBuilder::default()
        .terminal(false)
        .cwd(Path::new("/"))
        .args(vec![format!("/{}", name)])
        .build()
        .map_err(|_| EngineError::ContainerGenerationError)?;

    // define filesystem root
    let root = RootBuilder::default()
        .path(rootfs)
        .readonly(false)
        .build()
        .map_err(|_| EngineError::ContainerGenerationError)?;

    // attach namespaces/permissions for container
    let namespaces = vec![
        LinuxNamespaceBuilder::default().typ(LinuxNamespaceType::Pid).build().map_err(|_| EngineError::ContainerGenerationError)?,
        LinuxNamespaceBuilder::default().typ(LinuxNamespaceType::Network).build().map_err(|_| EngineError::ContainerGenerationError)?,
        LinuxNamespaceBuilder::default().typ(LinuxNamespaceType::Mount).build().map_err(|_| EngineError::ContainerGenerationError)?,
        LinuxNamespaceBuilder::default().typ(LinuxNamespaceType::Ipc).build().map_err(|_| EngineError::ContainerGenerationError)?,
        LinuxNamespaceBuilder::default().typ(LinuxNamespaceType::Uts).build().map_err(|_| EngineError::ContainerGenerationError)?,
    ];

    // Linux containers
    let linux = LinuxBuilder::default()
        .namespaces(namespaces)
        .build()
        .map_err(|_| EngineError::ContainerGenerationError)?;

    // mount our real binary into the container
    let bin_mount = MountBuilder::default()
        .destination(format!("/{}", name))
        .source(binary)
        .typ("bind")
        .options(vec![
            "ro".to_string(),
            "bind".to_string(),
        ])
        .build()
        .map_err(|_| EngineError::ContainerGenerationError)?;

    // combine our custom mount with some default mounts
    let mut mounts = oci_spec::runtime::get_default_mounts();
    mounts.push(bin_mount);

    // generate spec
    let spec = SpecBuilder::default()
        .version("1.0.2")
        .root(root)
        .process(process)
        .linux(linux)
        .mounts(mounts)
        .build()
        .map_err(|e| {
            eprintln!("{}", e);
            EngineError::ContainerGenerationError})?;

    Ok(spec)
}
