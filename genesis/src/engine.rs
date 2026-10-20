use std::{collections::HashMap, iter::Map, sync::{Arc}};

use oracle::io::Logger;
use tokio::{process::{Child, Command}, runtime::Builder, sync::{Mutex, mpsc::{Receiver, Sender, channel}}, task::JoinError};

use crate::types::{SysConfig, SysService};

pub enum EngineError {
    TokioFailure(Arc<dyn std::error::Error + Send + Sync>),
    TokioJoin(JoinError),
    TokioCommandError(Arc<dyn std::error::Error + Send + Sync>),
    TokioPidFailure(String),
}

pub fn begin_lifecycle(mut conf: SysConfig, eng: Arc<GenesisEngine>) {
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
            },
            SysService::Safe { priority, name, binary, flags } => {
                // binary needs to be started in a container
            }
            _ => {},
        }
    }
}

pub struct GenesisProcess {
    pub sys_service: SysService,
    pub pid: u32,
    pub proc: Child,
}

pub struct GenesisEngine {
    pub proc_queue: Mutex<(Receiver<SysService>, Arc<Sender<SysService>>)>,
    pub procs: Mutex<HashMap<u32, GenesisProcess>>,
    logger: Arc<Logger>,
}

impl GenesisEngine {
    pub fn new(log: Arc<Logger>) -> Arc<GenesisEngine> {
        // open async channel for queuing processes to open
        let chan = channel(20);

        return Arc::new(GenesisEngine {
            proc_queue: Mutex::new((chan.1, Arc::new(chan.0))), // deconstruct channel into custom tuple
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
            // these processes will yield voluntarily to the async event loop, instead of lazily with io processes
            let ipc = tokio::spawn(self.clone().handle_ipc());
            let unsafe_lifecycle = tokio::spawn(self.clone().handle_unsafe_lifecycles());

            // capture any errors that get returned from the engine
            let ret = tokio::try_join!(
                ipc,
                unsafe_lifecycle
            ).map_err(|e| EngineError::TokioJoin(e))?;

            // return captured result/errors
            return ret.1;
        });

        return ret;
    }

    async fn handle_ipc(self: Arc<Self>) {
        // handle incoming requests or data on the io_uring IPC ring buffer
    }

    async fn handle_unsafe_lifecycles(self: Arc<Self>) -> Result<(), EngineError> {
        // look for any queued processes, and start them
        while let Some(proc_req) = self.clone().recv().await {
            match proc_req.clone() {
                // we only want to look at unsafe processes
                SysService::Unsafe { priority, name, binary } => {
                    let child = match Command::new(binary).spawn() {
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
                        pid: pid,
                        proc: child,
                    };

                    // lock the process hashmap and insert our new process
                    let mut procs = self.procs.lock().await;
                    procs.insert(pid, genesis_proc);
                }
                _ => {}
            }
        }

        /*
         * here we need to cycle through all unsafe processes and
         * watch for them to end or be killed. We then update our
         * hashmap accordingly
         */

        Ok(())
    }

    // grab any processes that are in our process queue (helper function)
    async fn recv(self: Arc<Self>) -> Option<SysService> {
        let mut queue = self.proc_queue.lock().await;
        return queue.0.recv().await;
    }
}
