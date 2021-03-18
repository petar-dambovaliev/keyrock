use crate::currency::Conversion;
use crate::order::{CommonOrderBook, Level};
use binance::errors::Result as BiError;
use binance::model::OrderBook;
use binance::websockets::*;
use std::sync::atomic::{AtomicBool, Ordering as AOrdering};
use std::time::Duration;
use tokio::sync::mpsc;

const NAME: &str = "binance";

pub struct Binance {
    endpoint: String,
}

impl Into<CommonOrderBook> for OrderBook {
    fn into(self) -> CommonOrderBook {
        let mut bids = Vec::with_capacity(self.bids.len());

        for bid in self.bids.into_iter() {
            bids.push(Level {
                exchange: NAME.to_string(),
                price: bid.price,
                amount: bid.qty,
            });
        }

        let mut asks = Vec::with_capacity(self.asks.len());

        for ask in self.asks.into_iter() {
            asks.push(Level {
                exchange: NAME.to_string(),
                price: ask.price,
                amount: ask.qty,
            });
        }

        CommonOrderBook { bids, asks }
    }
}

impl Binance {
    pub fn new(depth: u32, window: Duration, conversion: Conversion) -> Self {
        let endpoint = format!(
            "{}@depth{}@{}ms",
            conversion.to_string(),
            depth,
            window.as_millis()
        );
        Self { endpoint }
    }

    pub async fn connect(&self) -> BiError<mpsc::Receiver<CommonOrderBook>> {
        let endpoint = self.endpoint.clone();
        let (tx, rx) = mpsc::channel(1);

        tokio::task::spawn_blocking(move || -> BiError<()> {
            let keep_running = AtomicBool::new(true);

            let mut web_socket: WebSockets<'_> = WebSockets::new(|event: WebsocketEvent| {
                if let WebsocketEvent::OrderBook(order_book) = event {
                    let r = tx.blocking_send(order_book.into());

                    if let Err(e) = r {
                        println!("could not send binance orderbook: {}", e);
                        keep_running.store(false, AOrdering::Relaxed);
                    }
                }
                Ok(())
            });

            web_socket.connect(&endpoint)?;
            web_socket.event_loop(&keep_running)?;
            web_socket.disconnect()?;
            Ok(())
        });

        Ok(rx)
    }
}
