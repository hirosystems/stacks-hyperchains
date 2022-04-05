use std::convert::Infallible;
use std::sync::Arc;

use crate::burnchains::BurnchainChannel;
use stacks::burnchains::events::NewBlock;
use std::thread;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::Sender;
use tokio::task::JoinError;
use warp;
use warp::Filter;
pub const EVENT_OBSERVER_PORT: u16 = 50303;

/// Adds in `channel` to downstream functions.
fn with_db(
    channel: Arc<dyn BurnchainChannel>,
) -> impl Filter<Extract = (Arc<dyn BurnchainChannel>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || channel.clone())
}

/// Route handler.
async fn handle_new_block(
    block: serde_json::Value,
    channel: Arc<dyn BurnchainChannel>,
) -> Result<impl warp::Reply, Infallible> {
    let parsed_block: NewBlock =
        serde_json::from_str(&block.to_string()).expect("Failed to parse events JSON");
    info!("handle_new_block receives new block {:?}", &parsed_block);
    channel.push_block(parsed_block);
    Ok(warp::http::StatusCode::OK)
}

/// Define and run the `warp` server.
async fn serve(
    signal_receiver: Receiver<()>,
    channel: Arc<dyn BurnchainChannel>,
) -> Result<(), JoinError> {
    let first_part = warp::path!("new_block")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(channel));
    let new_blocks = first_part.and_then(handle_new_block);

    info!("Binding warp server.");
    let (_addr, server) = warp::serve(new_blocks).bind_with_graceful_shutdown(
        ([127, 0, 0, 1], EVENT_OBSERVER_PORT),
        async {
            signal_receiver.await.ok();
        },
    );

    // Spawn the server into a runtime
    info!("Spawning warp server");
    tokio::task::spawn(server).await
}

/// Spawn a thread with a `warp` server.
pub fn spawn(channel: Arc<dyn BurnchainChannel>) -> Sender<()> {
    let (signal_sender, signal_receiver) = oneshot::channel();
    thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("Failed to initialize tokio");
        rt.block_on(serve(signal_receiver, channel))
            .expect("block_on failed");
    });
    signal_sender
}
