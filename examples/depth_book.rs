use binance_spot_connector_rust::{hyper::BinanceHttpClient, market};

use env_logger::Builder;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use rust_decimal::Decimal;
use serde::{de, Deserialize, Deserializer};
use std::{
    collections::{BTreeMap, VecDeque},
    str::FromStr,
    sync::Arc,
};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite;

use tokio::sync::{mpsc, RwLock};

#[derive(Debug, Deserialize)]
pub struct OfferData {
    #[serde(deserialize_with = "de_float_from_str")]
    pub price: Decimal,
    #[serde(deserialize_with = "de_float_from_str")]
    pub size: Decimal,
}
pub fn de_float_from_str<'a, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'a>,
{
    let str_val = String::deserialize(deserializer)?;
    Decimal::from_str(&str_val).map_err(de::Error::custom)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepthSnapshot {
    pub last_update_id: u64,
    pub bids: Vec<OfferData>,
    pub asks: Vec<OfferData>,
}
#[derive(Debug, Deserialize)]
pub struct DepthStreamWrapper {
    pub stream: String,
    pub data: DepthSnapshot,
}

#[derive(Debug, Deserialize)]
struct DepthUpdate {
    #[serde(rename = "U")]
    first_update_id: u64,
    #[serde(rename = "u")]
    final_update_id: u64,
    #[serde(rename = "b")]
    bids: Vec<OfferData>,
    #[serde(rename = "a")]
    asks: Vec<OfferData>,
}

#[derive(Debug, Clone)]
enum BookState {
    Buffering,
    Processing,
}

#[derive(Debug, Clone)]
struct OrderBookState {
    bids: BTreeMap<Decimal, Decimal>,
    asks: BTreeMap<Decimal, Decimal>,
    last_update_id: u64,
    state: BookState,
}

impl OrderBookState {
    fn new() -> Self {
        OrderBookState {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update_id: 0,
            state: BookState::Buffering,
        }
    }

    async fn from_snapshot(symbol: String) -> Result<Self, Box<dyn std::error::Error>> {
        let url = format!(
            "https://api.binance.com/api/v3/depth?symbol={}&limit=1000",
            symbol.to_uppercase()
        );
        let snapshot: DepthSnapshot = reqwest::get(url).await?.json().await?;
        let mut state = OrderBookState::new();
        state.apply_snapshot(snapshot);
        Ok(state)
    }

    fn apply_snapshot(&mut self, snapshot: DepthSnapshot) {
        info!(
            "Applying snaphot with last_update_id: {}",
            snapshot.last_update_id
        );

        self.bids.clear();
        self.asks.clear();

        for OfferData { price, size } in snapshot.bids {
            if size > Decimal::ZERO {
                self.bids.insert(price, size);
            }
        }

        for OfferData { price, size } in snapshot.asks {
            if size > Decimal::ZERO {
                self.asks.insert(price, size);
            }
        }

        self.last_update_id = snapshot.last_update_id;
        info!(
            "Local orderbook state initialized with last_update_id: {}",
            self.last_update_id
        );
    }

    fn process_update(&mut self, update: DepthUpdate) -> Result<(), String> {
        if update.final_update_id <= self.last_update_id {
            debug!("Ignoring old update");
            return Ok(()); // Silently ignore old updates
        }
        if update.first_update_id > self.last_update_id + 1 {
            return Err(format!(
                "Update sequence gap detected. Local: {}, Update: [{}, {}]",
                self.last_update_id, update.first_update_id, update.final_update_id
            ));
        }

        self.apply_update_changes(update)
    }

    fn apply_update_changes(&mut self, update: DepthUpdate) -> Result<(), String> {
        for OfferData { price, size } in &update.bids {
            if *size > Decimal::ZERO {
                self.bids.insert(*price, *size);
            } else {
                self.bids.remove(price);
            }
        }

        for OfferData { price, size } in &update.asks {
            if *size > Decimal::ZERO {
                self.asks.insert(*price, *size);
            } else {
                self.asks.remove(price);
            }
        }

        debug!(
            "Update applied successfully, new last_update_id: {}",
            update.final_update_id
        );
        self.last_update_id = update.final_update_id;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DepthBook {
    state: Arc<RwLock<OrderBookState>>,

    symbol: String,
}
static BINANCE_WS_API: &str = "wss://stream.binance.com:9443";

impl DepthBook {
    pub fn new(symbol: String) -> Self {
        Self {
            state: Arc::new(RwLock::new(OrderBookState::new())),
            symbol,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting depth book processing");
        // Step 1: Create channels for communication
        let (tx, mut rx) = mpsc::channel(1000);

        // Step 2: Start WebSocket connection first and begin buffering
        let ws_task = self.start_websocket(tx.clone());

        // Step 3: Wait a moment to ensure we're buffering
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Step 4: Get snapshot
        info!("Getting initial snapshot");
        let snapshot = self.fetch_snapshot().await?;
        info!(
            "Received snapshot with lastUpdateId: {}",
            snapshot.last_update_id
        );
        // Step 5: Initialize order book with snapshot
        let mut state = OrderBookState::from_snapshot(self.symbol.clone()).await?;

        // Step 6: Process buffered updates

        info!("Processing buffered updates...");
        let mut buffer: Vec<DepthUpdate> = Vec::new();
        let buffered = rx.recv_many(&mut buffer, 1000).await;
        let mut buffer = VecDeque::from(buffer);

        info!("Received {} buffered updates", buffered);
        let buffer_size = buffer.len();
        info!("Processing {} buffered updates", buffer_size);

        let mut valid_updates = Vec::new();

        while let Some(update) = buffer.pop_front() {
            if update.final_update_id <= state.last_update_id {
                debug!("Ignoring old update: {}", update.final_update_id);
                continue;
            }
            if update.first_update_id <= state.last_update_id + 1 {
                valid_updates.push(update);
            } else {
                warn!(
                    "Out of sequence update during initial buffering: {}",
                    update.final_update_id
                );
                return Err("Out of sequence update during initial buffering".into());
            }
        }

        // Change state to processing
        state.state = BookState::Processing;

        for update in valid_updates {
            state.apply_update_changes(update)?;
        }
        drop(state);

        // Start normal processing
        info!("Starting normal update processing...");
        while let Some(update) = rx.recv().await {
            let mut state = self.state.write().await;
            if let Err(e) = state.process_update(update) {
                error!("Error processing update: {}", e);
                // Here we could implement resync logic
                return Err(e.into());
            }
        }

        Ok(())
    }

    async fn fetch_snapshot(&self) -> Result<DepthSnapshot, Box<dyn std::error::Error>> {
        // let client = BinanceHttpClient::default();
        // let request = market::depth(&self.symbol).limit(1000);

        // let data = client
        //     .send(request)
        //     .await
        //     .expect("Request failed")
        //     .into_body_str()
        //     .await
        //     .expect("Failed to read response body");

        // let snapshot: DepthSnapshot = serde_json::from_str(&data)?;

        let url = format!(
            "https://api.binance.com/api/v3/depth?symbol={}&limit=1000",
            self.symbol.to_uppercase()
        );
        let snapshot: DepthSnapshot = reqwest::get(url).await?.json().await?;
        Ok(snapshot)
    }

    fn start_websocket(&self, tx: mpsc::Sender<DepthUpdate>) -> tokio::task::JoinHandle<()> {
        let symbol = self.symbol.clone();

        tokio::spawn(async move {
            // let (mut conn, response) = BinanceWebSocketClient::connect_async_default()
            //     .await
            //     .expect("Failed to connect");

            // conn.subscribe(vec![&DiffDepthStream::from_100ms(&symbol).into()])
            //     .await;
            // while let Some(msg) = conn.as_mut().next().await {
            //     match msg {
            //         Ok(msg) => {
            //             if msg.is_text() {
            //                 println!("Received text message: {:?}", msg);
            //                 let update: DepthUpdate =
            //                     serde_json::from_slice(&msg.into_data()).expect("Can't parse");
            //                 tx.send(update).await.unwrap();
            //             } else {
            //                 println!("Received binary message: {:?}", msg);
            //             }
            //         }
            //         Err(e) => {
            //             println!("{:?}", e);
            //             break;
            //         }
            //     }
            let url = format!(
                "{}/ws/{}@depth@100ms",
                BINANCE_WS_API,
                symbol.to_lowercase()
            );

            loop {
                info!("Connecting to WebSocket...");
                match connect_async(&url).await {
                    Ok((mut socket, _)) => {
                        info!("WebSocket connected, starting update buffer");

                        while let Some(msg) = socket.next().await {
                            match msg {
                                Ok(msg) => {
                                    if let tungstenite::Message::Text(text) = msg {
                                        match serde_json::from_str::<DepthUpdate>(&text) {
                                            Ok(update) => {
                                                if let Err(e) = tx.send(update).await {
                                                    error!("Error sending update: {}", e);
                                                    break;
                                                }
                                            }
                                            Err(e) => error!("Error parsing update: {}", e),
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("WebSocket error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("WebSocket connection error: {}", e);
                    }
                }

                // Wait before reconnecting
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        })
    }

    // Read access to current state
    pub async fn get_bids(&self) -> BTreeMap<Decimal, Decimal> {
        self.state.read().await.bids.clone()
    }

    pub async fn get_asks(&self) -> BTreeMap<Decimal, Decimal> {
        self.state.read().await.asks.clone()
    }

    pub async fn get_last_update_id(&self) -> u64 {
        self.state.read().await.last_update_id
    }
}

// Example usage
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::from_default_env()
        .filter(None, log::LevelFilter::Info)
        .init();
    let depth_book = DepthBook::new("btcusdt".to_string());

    // Spawn a task to periodically print the state
    let book_handle = depth_book.clone();
    tokio::spawn(async move {
        loop {
            let bids = book_handle.get_bids().await;
            let asks = book_handle.get_asks().await;
            info!(
                "Top 5 bids: {:?}",
                bids.iter().rev().take(5).collect::<Vec<_>>()
            );
            info!("Top 5 asks: {:?}", asks.iter().take(5).collect::<Vec<_>>());
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        }
    });

    // Start the main processing
    depth_book.start().await?;

    Ok(())
}
