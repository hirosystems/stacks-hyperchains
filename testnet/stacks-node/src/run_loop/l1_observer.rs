use std::convert::Infallible;
use std::sync::Arc;
use std::sync::Mutex;

use std::thread;
use std::thread::JoinHandle;
use stacks::types::chainstate::StacksBlockId;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::Sender;
use warp;
use warp::Filter;
use stacks::burnchains::events::NewBlock;
pub const EVENT_OBSERVER_PORT: u16 = 50303;

lazy_static! {
    static ref LOCAL_CHANNEL_COPY: MockChannels = MockChannels {
        blocks: Arc::new(Mutex::new(vec![])),
        minimum_recorded_height: Arc::new(Mutex::new(0)),
    };
}

async fn handle_new_block(block: serde_json::Value) -> Result<impl warp::Reply, Infallible> {
    info!("handle_new_block receives new block");
    // let mut blocks = NEW_BLOCKS.lock().unwrap();
    // blocks.push(block);
    Ok(warp::http::StatusCode::OK)
}

use tokio::task::JoinError;

use crate::burnchains::mock_events::MockChannels;
async fn serve(signal_receiver: Receiver<()>) -> Result<(), JoinError> {
    let new_blocks = warp::path!("new_block")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(handle_new_block);

    info!("Binding warp server");
    let (addr, server) = warp::serve(new_blocks).bind_with_graceful_shutdown(
        ([127, 0, 0, 1], EVENT_OBSERVER_PORT),
        async {
            signal_receiver.await.ok();
        },
    );

    // Spawn the server into a runtime
    info!("Spawning warp server");
    // Spawn the server into a runtime
    tokio::task::spawn(server).await
}

pub fn spawn(channel_blocks:Arc<Mutex<Vec<NewBlock>>>) -> Sender<()> {
    let (signal_sender, signal_receiver) = oneshot::channel();
    thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("Failed to initialize tokio");
        rt.block_on(serve(signal_receiver)).expect("block_on failed");
    });
    signal_sender
}
