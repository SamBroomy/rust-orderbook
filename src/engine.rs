use uuid::Uuid;

use crate::{
    OrderBook, OrderBookState, OrderRequest, OrderResult, Price, Quantity, Side, TradeExecution,
};

use std::{collections::HashMap, fmt::Display};

//BTCUSD
//BTC -> Base
//USE -> Quote
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct TradingPair {
    base: String,
    quote: String,
}

impl TradingPair {
    pub fn new(base: String, quote: String) -> TradingPair {
        TradingPair { base, quote }
    }
}
impl Display for TradingPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_{}", self.base, self.quote)
    }
}

pub struct MatchingEngine {
    orderbooks: HashMap<TradingPair, OrderBook>,
}

impl MatchingEngine {
    pub fn new() -> Self {
        Self {
            orderbooks: HashMap::new(),
        }
    }

    pub fn add_market(&mut self, pair: TradingPair) -> Result<(), String> {
        if self.orderbooks.contains_key(&pair) {
            Err(format!("Market for {} already exists", pair))
        } else {
            self.orderbooks.insert(pair.clone(), OrderBook::default());
            Ok(())
        }
    }

    pub fn remove_market(&mut self, pair: &TradingPair) -> Result<(), String> {
        if self.orderbooks.remove(pair).is_some() {
            Ok(())
        } else {
            Err(format!("Market for {} does not exist", pair))
        }
    }

    pub fn place_order(
        &mut self,
        pair: &TradingPair,
        order: OrderRequest,
    ) -> Result<(OrderResult, Vec<TradeExecution>), String> {
        self.orderbooks
            .get_mut(pair)
            .map(|ob| ob.add_order(order))
            .ok_or_else(|| format!("Market for {} does not exist", pair))
    }

    pub fn cancel_order(
        &mut self,
        pair: &TradingPair,
        order_id: Uuid,
    ) -> Result<Option<OrderResult>, String> {
        self.orderbooks
            .get_mut(pair)
            .map(|ob| ob.delete_order(order_id))
            .ok_or_else(|| format!("Market for {} does not exist", pair))
    }

    pub fn get_order_book_state(&self, pair: &TradingPair) -> Result<OrderBookState, String> {
        self.orderbooks
            .get(pair)
            .map(|ob| ob.get_order_book_state())
            .ok_or_else(|| format!("Market for {} does not exist", pair))
    }

    pub fn get_best_bid_ask(
        &self,
        pair: &TradingPair,
    ) -> Result<(Option<Price>, Option<Price>), String> {
        self.orderbooks
            .get(pair)
            .map(|ob| ob.best_prices())
            .ok_or_else(|| format!("Market for {} does not exist", pair))
    }

    pub fn get_spread(&self, pair: &TradingPair) -> Result<Option<u64>, String> {
        self.orderbooks
            .get(pair)
            .map(|ob| ob.spread())
            .ok_or_else(|| format!("Market for {} does not exist", pair))
    }

    pub fn get_volume(&self, pair: &TradingPair) -> Result<Quantity, String> {
        self.orderbooks
            .get(pair)
            .map(|ob| ob.get_total_volume())
            .ok_or_else(|| format!("Market for {} does not exist", pair))
    }

    pub fn get_depth(&self, pair: &TradingPair) -> Result<(usize, usize), String> {
        self.orderbooks
            .get(pair)
            .map(|ob| ob.get_depth())
            .ok_or_else(|| format!("Market for {} does not exist", pair))
    }

    pub fn get_volume_at_price(
        &self,
        pair: &TradingPair,
        side: Side,
        price: Price,
    ) -> Result<Quantity, String> {
        self.orderbooks
            .get(pair)
            .map(|ob| ob.get_volume_at_price(&side, &price).unwrap_or(0))
            .ok_or_else(|| format!("Market for {} does not exist", pair))
    }

    pub fn get_markets(&self) -> Vec<TradingPair> {
        self.orderbooks.keys().cloned().collect()
    }

    pub fn market_exists(&self, pair: &TradingPair) -> bool {
        self.orderbooks.contains_key(pair)
    }
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}
