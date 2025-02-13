use env_logger::Builder;
use futures_util::StreamExt;
use log::{debug, info, warn};
use rust_decimal::Decimal;
use serde::{de, Deserialize, Deserializer};
use std::{
    collections::{BTreeMap, VecDeque},
    str::FromStr,
    time::Duration,
};

use tokio_tungstenite::{connect_async, tungstenite::Message};

use tokio::sync::{mpsc, oneshot};
static BINANCE_WS_API: &str = "wss://stream.binance.com:9443";

// Custom error types
use anyhow::Result;
#[derive(Debug, Deserialize)]
pub struct OfferData {
    #[serde(deserialize_with = "de_float_from_str")]
    pub price: Decimal,
    #[serde(deserialize_with = "de_float_from_str")]
    pub size: Decimal,
}
pub fn de_float_from_str<'a, D>(deserializer: D) -> std::result::Result<Decimal, D::Error>
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

#[derive(Debug)]
enum DataMessage {
    Update(DepthUpdate),
    Error(String),
}

#[derive(Debug)]
enum ControlMessage {
    Start,
    Stop,
    Error(String),
}

#[derive(Debug)]
enum QueryMessage {
    Bids(oneshot::Sender<BTreeMap<Decimal, Decimal>>),
    Asks(oneshot::Sender<BTreeMap<Decimal, Decimal>>),
    LastUpdateId(oneshot::Sender<u64>),
}
trait Component {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn handle_error(&mut self, error: String) -> Result<()>;
}

// ---------- WebSocket Component ----------
struct WebSocketComponent {
    symbol: String,
    data_tx: mpsc::Sender<DataMessage>,
    control_rx: mpsc::Receiver<ControlMessage>,
    reconnect_timeout: Duration,
}

impl Component for WebSocketComponent {
    async fn start(&mut self) -> Result<()> {
        info!("Starting WebSocket component");
        loop {
            match self.connect_and_stream().await {
                Ok(()) => {
                    // Normal shutdown
                    break;
                }
                Err(e) => {
                    self.handle_error(e.to_string()).await?;
                    tokio::time::sleep(self.reconnect_timeout).await;
                }
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping WebSocket component");
        Ok(())
    }

    async fn handle_error(&mut self, error: String) -> Result<()> {
        warn!("WebSocket error: {}", error);
        let _ = self.data_tx.send(DataMessage::Error(error)).await;
        Ok(())
    }
}

impl WebSocketComponent {
    async fn connect_and_stream(&mut self) -> Result<()> {
        info!("Connecting to WebSocket...");
        let url = format!(
            "{}/ws/{}@depth@100ms",
            BINANCE_WS_API,
            self.symbol.to_lowercase()
        );

        let (mut socket, response) = connect_async(&url).await?;
        info!("Connected to binance stream.");
        info!("HTTP status code: {}", response.status());
        info!("Response headers:");
        for (ref header, header_value) in response.headers() {
            info!("- {}: {:?}", header, header_value);
        }
        info!("WebSocket connected, starting update buffer");

        loop {
            tokio::select! {
                Some(msg) = socket.next() => {
                    match msg {
                        Ok(msg) => {
                            if let Message::Text(text) = msg {
                                match serde_json::from_str::<DepthUpdate>(&text) {
                                    Ok(update) => {
                                        self.data_tx.send(DataMessage::Update(update)).await?;
                                    }
                                    Err(e) => {
                                        self.handle_error(e.to_string()).await?;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            return Err(e.into());
                        }
                    }
                }
                Some(control) = self.control_rx.recv() => {
                    match control {
                        ControlMessage::Stop => return self.stop().await,
                        ControlMessage::Start => {}
                        ControlMessage::Error(e) => {
                            self.handle_error(e).await?;
                        }
                    }
                }
            }
        }
    }
}

struct StateComponent {
    state: OrderBookState,
    symbol: String,
    data_rx: mpsc::Receiver<DataMessage>,
    query_rx: mpsc::Receiver<QueryMessage>,
    control_tx: mpsc::Sender<ControlMessage>,
}
impl Component for StateComponent {
    async fn start(&mut self) -> Result<()> {
        // Step 4: Get snapshot

        let snapshot = self.fetch_snapshot().await?;
        info!(
            "Received snapshot with lastUpdateId: {}",
            snapshot.last_update_id
        );
        self.state.apply_snapshot(snapshot);

        // Step 6: Process buffered updates

        info!("Processing buffered updates...");
        let mut buffer = Vec::new();
        self.data_rx.recv_many(&mut buffer, usize::MAX).await;
        let buffer = buffer
            .into_iter()
            .filter_map(|msg| match msg {
                DataMessage::Update(update) => Some(update),
                _ => None,
            })
            .collect::<VecDeque<_>>();

        self.state.process_buffer(buffer)?;

        // Start normal processing
        info!("Starting normal update processing...");

        loop {
            tokio::select! {
                Some(msg) = self.data_rx.recv() => {
                    match msg {
                        DataMessage::Update(update) => {
                            if let Err(e) = self.state.process_update(update) {
                                self.handle_error(e.to_string()).await?;
                            }
                        }
                        DataMessage::Error(e) => {
                            self.handle_error(e).await?;
                        }
                    }
                }
                Some(query) = self.query_rx.recv() => {
                    self.handle_query(query).await;
                }

            }
        }

        // while let Some(msg) = self.data_rx.recv().await {
        //     match msg {
        //         DataMessage::Update(update) => {
        //             if let Err(e) = self.state.process_update(update) {
        //                 self.handle_error(e.to_string()).await?;
        //             }

        //             info!(
        //                 "Top 5 bids: {:?}",
        //                 self.state.bids.iter().rev().take(5).collect::<Vec<_>>()
        //             );
        //             info!(
        //                 "Top 5 asks: {:?}",
        //                 self.state.asks.iter().take(5).collect::<Vec<_>>()
        //             );
        //         }
        //         DataMessage::Error(e) => {
        //             self.handle_error(e).await?;
        //         }
        //     }
        // }
        // Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        Ok(())
    }

    async fn handle_error(&mut self, error: String) -> Result<()> {
        self.control_tx.send(ControlMessage::Error(error)).await?;
        Ok(())
    }
}

impl StateComponent {
    async fn fetch_snapshot(&self) -> Result<DepthSnapshot> {
        info!("Getting initial snapshot");
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
            "https://api.binance.com/api/v3/depth?symbol={}&limit=5000",
            self.symbol.to_uppercase()
        );
        let snapshot: DepthSnapshot = reqwest::get(url).await?.json().await?;
        Ok(snapshot)
    }
    async fn handle_query(&self, query: QueryMessage) {
        match query {
            QueryMessage::Bids(respond_to) => {
                let _ = respond_to.send(self.state.bids.clone());
            }
            QueryMessage::Asks(respond_to) => {
                let _ = respond_to.send(self.state.asks.clone());
            }
            QueryMessage::LastUpdateId(respond_to) => {
                let _ = respond_to.send(self.state.last_update_id);
            }
        }
    }
}

pub struct DepthBook {
    control_tx: mpsc::Sender<ControlMessage>,
    query_tx: mpsc::Sender<QueryMessage>,
}

impl DepthBook {
    pub fn new(symbol: String) -> (Self, DepthBookCoordinator) {
        let (control_tx, control_rx) = mpsc::channel(100);
        let (data_tx, data_rx) = mpsc::channel(1000);
        let (query_tx, query_rx) = mpsc::channel(100);

        let coordinator = DepthBookCoordinator {
            ws_component: Some(WebSocketComponent {
                symbol: symbol.clone(),
                data_tx: data_tx.clone(),
                control_rx,
                reconnect_timeout: Duration::from_secs(5),
            }),
            state_component: Some(StateComponent {
                symbol,
                state: OrderBookState::default(),
                data_rx,
                query_rx,
                control_tx: control_tx.clone(),
            }),
        };

        (
            Self {
                control_tx,
                query_tx,
            },
            coordinator,
        )
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting depth book processing");
        self.control_tx.send(ControlMessage::Start).await?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        self.control_tx.send(ControlMessage::Stop).await?;
        Ok(())
    }

    pub async fn get_bids(&self) -> Result<BTreeMap<Decimal, Decimal>> {
        let (tx, rx) = oneshot::channel();
        self.query_tx.send(QueryMessage::Bids(tx)).await?;
        Ok(rx.await?)
    }

    pub async fn get_asks(&self) -> Result<BTreeMap<Decimal, Decimal>> {
        let (tx, rx) = oneshot::channel();
        self.query_tx.send(QueryMessage::Asks(tx)).await?;
        Ok(rx.await?)
    }

    pub async fn get_last_update_id(&self) -> Result<u64> {
        let (tx, rx) = oneshot::channel();
        self.query_tx.send(QueryMessage::LastUpdateId(tx)).await?;
        Ok(rx.await?)
    }
}

pub struct DepthBookCoordinator {
    ws_component: Option<WebSocketComponent>,
    state_component: Option<StateComponent>,
}

impl DepthBookCoordinator {
    pub fn spawn(mut self) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move { self.run().await })
    }
    pub async fn run(&mut self) -> Result<()> {
        // Start components in separate tasks
        let mut ws_component = self
            .ws_component
            .take()
            .expect("WebSocket component missing");
        let mut state_component = self
            .state_component
            .take()
            .expect("State component missing");

        let ws_handle = tokio::spawn(async move { ws_component.start().await });
        info!("Waiting for initial buffering...");
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let state_handle = tokio::spawn(async move { state_component.start().await });

        let _ = tokio::try_join!(ws_handle, state_handle)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct OrderBookState {
    bids: BTreeMap<Decimal, Decimal>,
    asks: BTreeMap<Decimal, Decimal>,
    last_update_id: u64,
}

impl OrderBookState {
    fn new(last_update_id: u64) -> Self {
        OrderBookState {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update_id,
        }
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

    fn process_update(&mut self, update: DepthUpdate) -> Result<()> {
        warn!(
            "Processing update: [{}, {}]",
            update.first_update_id, update.final_update_id
        );
        if update.final_update_id <= self.last_update_id {
            debug!("Ignoring old update");
            return Ok(()); // Silently ignore old updates
        }
        if update.first_update_id > self.last_update_id + 1 {
            return Err(anyhow::Error::msg(format!(
                "Update sequence gap detected. Local: {}, Update: [{}, {}]",
                self.last_update_id, update.first_update_id, update.final_update_id
            )));
        }

        self.apply_update_changes(update)
    }

    fn process_buffer(&mut self, mut buffer: VecDeque<DepthUpdate>) -> Result<()> {
        let buffer_size = buffer.len();
        warn!("Processing {} buffered updates", buffer_size);

        while let Some(update) = buffer.pop_front() {
            if update.final_update_id <= self.last_update_id {
                debug!("Ignoring old update: {}", update.final_update_id);
                continue;
            }
            if update.first_update_id <= self.last_update_id + 1 {
                self.apply_update_changes(update)?;
            } else {
                warn!(
                    "Out of sequence update during initial buffering: {}",
                    update.final_update_id
                );
                return Err(anyhow::Error::msg(
                    "Out of sequence update during initial buffering",
                ));
            }
        }
        Ok(())
    }

    fn apply_update_changes(&mut self, update: DepthUpdate) -> Result<()> {
        for OfferData { price, size } in &update.bids {
            if *size > Decimal::ZERO {
                let price = *price;
                let size = *size;
                match self.bids.insert(price, size) {
                    Some(existing_size) => {
                        if existing_size != size {
                            debug!(
                                "Updated bid price: {} from {} to {} diff: {}",
                                price,
                                existing_size,
                                size,
                                existing_size - size
                            );
                        } else {
                            debug!("Bid price: {} size unchanged: {}", price, size);
                        }
                    }
                    None => {
                        debug!("New bid price: {} with size: {}", price, size);
                    }
                }
            } else {
                match self.bids.remove(price) {
                    Some(existing_size) => {
                        debug!("Removed bid price: {} with size: {}", price, existing_size);
                    }
                    None => {
                        debug!("Ignoring zero size bid price: {}", price);
                    }
                }
            }
        }

        for OfferData { price, size } in &update.asks {
            if *size > Decimal::ZERO {
                let price = *price;
                let size = *size;
                match self.asks.insert(price, size) {
                    Some(existing_size) => {
                        if existing_size != size {
                            debug!(
                                "Updated ask price: {} from {} to {} diff: {}",
                                price,
                                existing_size,
                                size,
                                existing_size - size
                            );
                        } else {
                            debug!("Ask price: {} size unchanged: {}", price, size);
                        }
                    }
                    None => {
                        debug!("New ask price: {} with size: {}", price, size);
                    }
                }
            } else {
                match self.asks.remove(price) {
                    Some(existing_size) => {
                        debug!("Removed ask price: {} with size: {}", price, existing_size);
                    }
                    None => {
                        warn!("Ignoring zero size ask price: {}", price);
                    }
                }
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
#[tokio::main]
async fn main() -> Result<()> {
    Builder::from_default_env()
        .filter(None, log::LevelFilter::Debug)
        .init();
    let (depth_book, coordinator) = DepthBook::new("btcusdt".to_string());

    // Start the depth book
    depth_book.start().await?;

    // Run the coordinator in a separate task
    let _coordinator_handle = coordinator.spawn();

    // Example query loop
    loop {
        let last_update_id = depth_book.get_last_update_id().await?;
        info!("Current last_update_id: {}", last_update_id);
        let bids = depth_book.get_bids().await?;
        info!(
            "Top 5 bids: {:?}",
            bids.iter().rev().take(5).collect::<Vec<_>>()
        );
        let asks = depth_book.get_asks().await?;
        info!("Top 5 asks: {:?}", asks.iter().take(5).collect::<Vec<_>>());

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

// #[derive(Debug, Clone)]
// pub struct DepthBook {
//     state: Arc<RwLock<OrderBookState>>,

//     symbol: String,
// }

// impl DepthBook {
//     pub fn new(symbol: String) -> Self {
//         Self {
//             state: Arc::new(RwLock::new(OrderBookState::default())),
//             symbol,
//         }
//     }

//     pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
//         info!("Starting depth book processing");
//         // Step 1: Create channels for communication
//         let (tx, rx) = unbounded();

//         // Step 2: Start WebSocket connection first and begin buffering
//         let ws_task = self.start_websocket(tx.clone());

//         // Step 3: Wait a moment to ensure we're buffering
//         tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

//         // Step 4: Get snapshot
//         info!("Getting initial snapshot");
//         let snapshot = self.fetch_snapshot().await?;
//         info!(
//             "Received snapshot with lastUpdateId: {}",
//             snapshot.last_update_id
//         );
//         // Step 5: Initialize order book with snapshot
//         let mut state = self.state.write().await;
//         state.apply_snapshot(snapshot);

//         // Step 6: Process buffered updates

//         info!("Processing buffered updates...");

//         let buffer = rx.try_iter().collect::<VecDeque<_>>();

//         state.process_buffer(buffer)?;

//         drop(state);

//         // Start normal processing
//         info!("Starting normal update processing...");
//         while let Ok(update) = rx.recv() {
//             let mut state = self.state.write().await;
//             if let Err(e) = state.process_update(update) {
//                 error!("Error processing update: {}", e);
//                 // Here we could implement resync logic
//                 return Err(e.into());
//             }
//         }

//         Ok(())
//     }

//     async fn fetch_snapshot(&self) -> Result<DepthSnapshot, Box<dyn std::error::Error>> {
//         // let client = BinanceHttpClient::default();
//         // let request = market::depth(&self.symbol).limit(1000);

//         // let data = client
//         //     .send(request)
//         //     .await
//         //     .expect("Request failed")
//         //     .into_body_str()
//         //     .await
//         //     .expect("Failed to read response body");

//         // let snapshot: DepthSnapshot = serde_json::from_str(&data)?;

//         let url = format!(
//             "https://api.binance.com/api/v3/depth?symbol={}&limit=1000",
//             self.symbol.to_uppercase()
//         );
//         let snapshot: DepthSnapshot = reqwest::get(url).await?.json().await?;
//         Ok(snapshot)
//     }

//     fn start_websocket(&self, tx: Sender<DepthUpdate>) -> tokio::task::JoinHandle<()> {
//         let symbol = self.symbol.clone();

//         tokio::spawn(async move {
//             // let (mut conn, response) = BinanceWebSocketClient::connect_async_default()
//             //     .await
//             //     .expect("Failed to connect");

//             // conn.subscribe(vec![&DiffDepthStream::from_100ms(&symbol).into()])
//             //     .await;
//             // while let Some(msg) = conn.as_mut().next().await {
//             //     match msg {
//             //         Ok(msg) => {
//             //             if msg.is_text() {
//             //                 println!("Received text message: {:?}", msg);
//             //                 let update: DepthUpdate =
//             //                     serde_json::from_slice(&msg.into_data()).expect("Can't parse");
//             //                 tx.send(update).await.unwrap();
//             //             } else {
//             //                 println!("Received binary message: {:?}", msg);
//             //             }
//             //         }
//             //         Err(e) => {
//             //             println!("{:?}", e);
//             //             break;
//             //         }
//             //     }
//             let url = format!(
//                 "{}/ws/{}@depth@100ms",
//                 BINANCE_WS_API,
//                 symbol.to_lowercase()
//             );

//             info!("Connecting to WebSocket...");
//             match connect_async(&url).await {
//                 Ok((mut socket, _)) => {
//                     info!("WebSocket connected, starting update buffer");

//                     while let Some(msg) = socket.next().await {
//                         match msg {
//                             Ok(msg) => {
//                                 if let tungstenite::Message::Text(text) = msg {
//                                     match serde_json::from_str::<DepthUpdate>(&text) {
//                                         Ok(update) => {
//                                             if let Err(e) = tx.send(update) {
//                                                 error!("Error sending update: {}", e);
//                                                 break;
//                                             }
//                                         }
//                                         Err(e) => error!("Error parsing update: {}", e),
//                                     }
//                                 }
//                             }
//                             Err(e) => {
//                                 error!("WebSocket error: {}", e);
//                                 break;
//                             }
//                         }
//                     }
//                 }
//                 Err(e) => {
//                     error!("WebSocket connection error: {}", e);
//                 }
//             }
//         })
//     }

//     // Read access to current state
//     pub async fn get_bids(&self) -> BTreeMap<Decimal, Decimal> {
//         self.state.read().await.bids.clone()
//     }

//     pub async fn get_asks(&self) -> BTreeMap<Decimal, Decimal> {
//         self.state.read().await.asks.clone()
//     }

//     pub async fn get_last_update_id(&self) -> u64 {
//         self.state.read().await.last_update_id
//     }
// }

// Example usage
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     Builder::from_default_env()
//         .filter(None, log::LevelFilter::Info)
//         .init();
//     let depth_book = DepthBook::new("btcusdt".to_string());

//     // Spawn a task to periodically print the state
//     let book_handle = depth_book.clone();
//     tokio::spawn(async move {
//         loop {
//             let bids = book_handle.get_bids().await;
//             let asks = book_handle.get_asks().await;
//             info!(
//                 "Top 5 bids: {:?}",
//                 bids.iter().rev().take(5).collect::<Vec<_>>()
//             );
//             info!("Top 5 asks: {:?}", asks.iter().take(5).collect::<Vec<_>>());
//             tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
//         }
//     });

//     // Start the main processing
//     depth_book.start().await?;

//     Ok(())
// }
