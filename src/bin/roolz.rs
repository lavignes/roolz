use std::{
    cell::Cell,
    error::Error,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    thread::{self, JoinHandle},
    time::Duration,
};

use async_ctrlc::CtrlC;
use clap::Clap;
use futures::future::FutureExt;
use futures_core::{Future, Stream};
use notify::{self, DebouncedEvent, RecursiveMode, Watcher};
use tokio::{stream::StreamExt, sync::mpsc};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use uuid::Uuid;

use roolz::api::v1alpha::service::{
    RulesService, RulesServiceServer, SessionRequest, SessionResponse,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opts::parse();
    simple_logger::init_with_level(opts.log_level).expect("Failed to initialize logger");

    let sig_handler = CtrlC::new().expect("Cannot create signal handler").shared();

    tokio::try_join!(
        start_server(opts.address, sig_handler.clone()),
        watch_files(opts.rules, sig_handler.clone()),
        watch_files(opts.facts, sig_handler),
    )?;

    Ok(())
}

/// A distributed rules engine
#[derive(Clap)]
#[clap(version = "0.1.0")]
struct Opts {
    /// todo
    address: SocketAddr,

    /// todo
    #[clap(short = "l", long = "log-level", default_value = "info")]
    log_level: log::Level,

    /// todo
    #[clap(short = "r", long = "rules", required = true)]
    rules: Vec<PathBuf>,

    /// todo
    #[clap(short = "f", long = "facts", required = true)]
    facts: Vec<PathBuf>,
}

async fn watch_files<S: Future<Output = ()>>(
    paths: Vec<PathBuf>,
    sig_handler: S,
) -> Result<(), Box<dyn Error>> {
    let watcher_id = Uuid::new_v4();
    log::info!(
        "Starting file watcher ({}) for {} paths...",
        watcher_id,
        paths.len()
    );
    let join_handle: Cell<Option<JoinHandle<()>>> = Cell::new(None);
    // Race with the sig_handler
    let result = tokio::select! {
        _ = sig_handler => Ok(()),
        result = async {
            // notify-rs uses sync channels so we spin off a worker to block
            // on that channel and notify the async task via an async channel
            let (sync_tx, sync_rx) = std::sync::mpsc::channel();
            let mut watcher = notify::watcher(sync_tx, Duration::from_secs(1))?;
            for path in paths {
                watcher.watch(path, RecursiveMode::Recursive)?;
            }

            let (tx, mut rx) = mpsc::unbounded_channel();
            let builder = thread::Builder::new()
                .name(watcher_id.to_string());
            join_handle.set(Some(builder.spawn(move || loop {
                if let Err(_) = tx.send(sync_rx.recv()) {
                    // Channel closed
                    break;
                }
            }).expect("Failed to spawn thread")));

            while let Some(result) = rx.recv().await {
                match result? {
                    DebouncedEvent::Write(path) => {
                        log::debug!("File updated: {:?}", path);
                    }
                    DebouncedEvent::Remove(path) => {
                        log::debug!("File removed: {:?}", path);
                    }
                    DebouncedEvent::Rename(old, new) => {
                        log::debug!("File moved: {:?} to {:?}", old, new);
                    }
                    _ => {}
                }
            }

            Ok(())
        } => result
    };
    log::info!("File watcher ({}) terminating...", watcher_id);
    // Thread will exit when async channel closes
    if let Some(join_handle) = join_handle.take() {
        join_handle.join().expect("Thread panicked");
    }
    result
}

async fn start_server<S: Future<Output = ()>>(
    addr: SocketAddr,
    sig_handler: S,
) -> Result<(), Box<dyn Error>> {
    let service = RulesServiceServer::new(RulesServiceState {});

    log::info!("Starting server on {}...", addr);
    Server::builder()
        .add_service(service)
        .serve_with_shutdown(addr, sig_handler)
        .await?;
    log::info!("Shutting down server...");

    Ok(())
}

#[derive(Debug)]
struct RulesServiceState;

#[tonic::async_trait]
impl RulesService for RulesServiceState {
    type SessionStream =
        Pin<Box<dyn Stream<Item = Result<SessionResponse, Status>> + Send + Sync + 'static>>;

    async fn session(
        &self,
        request: Request<Streaming<SessionRequest>>,
    ) -> Result<Response<Self::SessionStream>, Status> {
        let mut stream = request.into_inner();

        let handler = async_stream::try_stream! {
            while let Some(req) = stream.next().await {
                log::debug!("{:?}", req);
                yield SessionResponse::default();
            }
        };

        Ok(Response::new(Box::pin(handler)))
    }
}
