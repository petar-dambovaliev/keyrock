use futures::Stream;
use orderbook_aggregator_server::OrderbookAggregator;
pub use orderbook_aggregator_server::OrderbookAggregatorServer;
use std::pin::Pin;
use tokio::sync::mpsc;
use tonic::{async_trait, Code, Request, Response, Status};
tonic::include_proto!("orderbook");
use crate::binance::Binance;
use crate::bitstamp::BitStamp;
use std::cmp::Ordering;
use std::sync::Arc;

#[derive(Default)]
pub struct CommonOrderBook {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

impl CommonOrderBook {
    fn summary(mut self, n: usize) -> Summary {
        let first_bid = match self.bids.first() {
            Some(f) => f,
            None => return Default::default(),
        };

        let first_ask = match self.asks.first() {
            Some(f) => f,
            None => return Default::default(),
        };

        let spread = first_bid.price - first_ask.price;

        self.bids
            .sort_by(|a, b| match a.price / a.amount < b.price / b.amount {
                true => Ordering::Less,
                false => Ordering::Greater,
            });
        self.bids.truncate(n);

        self.asks
            .sort_by(|a, b| match a.price / a.amount < b.price / b.amount {
                true => Ordering::Greater,
                false => Ordering::Less,
            });

        self.asks.truncate(n);

        Summary {
            spread: spread.abs(),
            bids: self.bids,
            asks: self.asks,
        }
    }

    fn extend(&mut self, other: CommonOrderBook) {
        self.bids.extend(other.bids);
        self.asks.extend(other.asks);
    }
}

pub struct OrderbookAggregatorService {
    binance: Arc<Binance>,
    bitstamp: Arc<BitStamp>,
    top: usize,
}

impl OrderbookAggregatorService {
    pub fn new(binance: Binance, bitstamp: BitStamp, top: usize) -> Self {
        Self {
            binance: Arc::new(binance),
            bitstamp: Arc::new(bitstamp),
            top,
        }
    }
}

#[async_trait]
impl OrderbookAggregator for OrderbookAggregatorService {
    type BookSummaryStream =
        Pin<Box<dyn Stream<Item = Result<Summary, Status>> + Send + Sync + 'static>>;

    async fn book_summary(
        &self,
        _: Request<Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        let (tx_inner, mut rx_inner) = mpsc::channel(2);
        let binance = self.binance.clone();
        let bitstamp = self.bitstamp.clone();
        let top = self.top;

        let mut tx_binance_inner = match binance.connect().await {
            Ok(k) => k,
            Err(e) => return Err(Status::new(Code::Unknown, e.to_string())),
        };

        let mut tx_bitstamp_inner = match bitstamp.connect().await {
            Ok(k) => k,
            Err(e) => return Err(Status::new(Code::Unknown, e.to_string())),
        };

        tokio::spawn(async move {
            loop {
                let (bin_book, stamp_book) =
                    tokio::join!(tx_binance_inner.recv(), tx_bitstamp_inner.recv());

                if bin_book.is_none() {
                    println!("binance returned nothing");
                }

                if stamp_book.is_none() {
                    println!("bitstamp returned nothing");
                }
                let bin_book = bin_book.unwrap_or(Default::default());
                let stamp_book = stamp_book.unwrap_or(Default::default());

                if let Err(e) = tx_inner.send((bin_book, stamp_book)).await {
                    println!("error sending books: {}", e);
                    return;
                }
            }
        });

        let (tx, rx) = mpsc::channel(2);

        tokio::spawn(async move {
            while let Some((mut one, two)) = rx_inner.recv().await {
                one.extend(two);
                if let Err(e) = tx.send(Ok(one.summary(top))).await {
                    println!("err: {}", e);
                    return;
                }
            }
        });

        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        )))
    }
}
