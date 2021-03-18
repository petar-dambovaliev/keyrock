use crate::currency::Conversion;
use crate::order::{CommonOrderBook, Level};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as TError;
use tokio_tungstenite::tungstenite::Message;

const WEBSOCKET_URL: &str = "wss://ws.bitstamp.net";
const NAME: &str = "bitstamp";
const SUBS_SUCCEEDED: &str = "bts:subscription_succeeded";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SubsSucceeded {
    event: String,
    channel: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
struct Response {
    data: OrderBook,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub struct OrderBook {
    #[serde(with = "string")]
    pub timestamp: u64,
    #[serde(with = "string")]
    pub microtimestamp: u64,
    pub bids: Vec<Bids>,
    pub asks: Vec<Asks>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bids {
    #[serde(with = "string_or_float")]
    pub price: f64,
    #[serde(with = "string_or_float")]
    pub qty: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asks {
    #[serde(with = "string_or_float")]
    pub price: f64,
    #[serde(with = "string_or_float")]
    pub qty: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Event {
    event: String,
    data: HashMap<String, String>,
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

pub struct BitStamp {
    subs: serde_json::Value,
}

fn is_valid_subs_response(msg: &String) -> bool {
    let res: SubsSucceeded = match serde_json::from_str(msg) {
        Ok(r) => r,
        Err(e) => {
            println!("invalid subscription response json: {} err: {}", msg, e);
            return false;
        }
    };

    res.event == SUBS_SUCCEEDED
}

impl BitStamp {
    pub fn new(conver: Conversion) -> Self {
        let subs = {
            let mut data = HashMap::with_capacity(1);
            data.insert(
                "channel".to_string(),
                format!("order_book_{}", conver.to_string()),
            );

            json!(Event {
                event: "bts:subscribe".to_string(),
                data
            })
        };
        Self { subs }
    }

    pub async fn connect(&self) -> Result<mpsc::Receiver<CommonOrderBook>> {
        let (ws_stream, _) = connect_async(WEBSOCKET_URL).await?;
        let (mut write, mut read) = ws_stream.split();

        write.send(Message::Text(self.subs.to_string())).await?;

        let (rx, tx) = mpsc::channel(1);

        tokio::task::spawn(async move {
            let mut first = true;
            while let Some(msg) = read.next().await {
                let message = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        println!("bitstamp recv msg err: {:#?}", e);
                        continue;
                    }
                };

                match message {
                    Message::Text(msg) => {
                        if first {
                            let is_valid = is_valid_subs_response(&msg);
                            if !is_valid {
                                println!("bitstamp: subscription error {:#?}", msg);
                                return;
                            }
                            first = false;
                            continue;
                        }

                        let res: Response = match serde_json::from_str(&msg) {
                            Ok(r) => r,
                            Err(e) => {
                                println!("invalid json: {} err: {}", msg, e);
                                continue;
                            }
                        };

                        if let Err(e) = rx.send(res.data.into()).await {
                            println!("send error: {}", e);
                            return;
                        }
                    }
                    Message::Ping(_) | Message::Pong(_) => {}
                    Message::Binary(_) => (),
                    Message::Close(e) => {
                        println!("Disconnected {:?}", e);
                        return;
                    }
                }
            }
        });

        Ok(tx)
    }
}

pub(crate) mod string_or_float {
    use std::fmt;

    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: fmt::Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrFloat {
            String(String),
            Float(f64),
        }

        match StringOrFloat::deserialize(deserializer)? {
            StringOrFloat::String(s) => s.parse().map_err(de::Error::custom),
            StringOrFloat::Float(i) => Ok(i),
        }
    }
}

pub(crate) mod string {
    use std::fmt;

    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: fmt::Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrFloat {
            String(String),
        }

        match StringOrFloat::deserialize(deserializer)? {
            StringOrFloat::String(s) => s.parse().map_err(de::Error::custom),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ContentError {
    pub code: i16,
    pub msg: String,
}

error_chain! {
    errors {
        BinanceError(response: ContentError)
     }

    foreign_links {
        IoError(std::io::Error);
        ParseFloatError(std::num::ParseFloatError);
        Json(serde_json::Error);
        Tungstenite(TError);
        TimestampError(std::time::SystemTimeError);
    }
}
