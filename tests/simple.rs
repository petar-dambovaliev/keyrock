use std::error::Error;

use tonic::transport::Channel;
use tonic::Request;

use orderbook::orderbook_aggregator_client::OrderbookAggregatorClient;
use orderbook::Empty;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

const EMPTY: Empty = Empty {};

async fn print_orderbook(
    client: &mut OrderbookAggregatorClient<Channel>,
) -> Result<(), Box<dyn Error>> {
    let mut stream = client.book_summary(Request::new(EMPTY)).await?.into_inner();

    while let Some(summary) = stream.message().await? {
        println!("NOTE = {:?}", summary);
    }

    Ok(())
}

#[tokio::test]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = OrderbookAggregatorClient::connect("http://[::1]:50051").await?;

    println!("\n*** SERVER STREAMING ***");
    print_orderbook(&mut client).await?;

    Ok(())
}
