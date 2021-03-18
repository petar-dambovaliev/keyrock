mod binance;
mod bitstamp;
mod currency;
mod order;

use crate::binance::Binance;
use currency::{Conversion, Currency};
use order::{OrderbookAggregatorServer, OrderbookAggregatorService};
use std::time::Duration;
use tonic::transport::Server;

#[macro_use]
extern crate error_chain;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().expect("invalid socket address");
    let conv = Conversion::new(Currency::Eth, Currency::Btc);
    let binance = Binance::new(20, Duration::from_millis(100), conv.clone());
    let bit_stamp = bitstamp::BitStamp::new(conv);
    let order_book_aggregator = OrderbookAggregatorService::new(binance, bit_stamp, 10);

    println!("Order book aggregator server listening on {}", addr);

    Server::builder()
        .add_service(OrderbookAggregatorServer::new(order_book_aggregator))
        .serve(addr)
        .await?;

    Ok(())
}
