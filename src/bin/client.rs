use std::error::Error;

use tokio::sync::mpsc;
use tonic::Request;

use roolz::api::v1alpha::service::{RulesServiceClient, SessionRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = RulesServiceClient::connect("http://127.0.0.1:1234").await?;

    let (tx, mut rx) = mpsc::unbounded_channel();
    let handler = async_stream::stream! {
        loop {
            yield rx.recv().await.unwrap();
        }
    };

    let response = client.session(Request::new(handler)).await?;
    let mut inbound = response.into_inner();

    tx.send(SessionRequest::default()).unwrap();
    while let Some(resp) = inbound.message().await? {
        println!("{:?}", resp);
        tx.send(SessionRequest::default()).unwrap();
    }

    Ok(())
}
