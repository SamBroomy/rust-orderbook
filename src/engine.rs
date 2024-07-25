use crate::orderbook::{Price, Quantity, Side};

use super::orderbook::OrderBook;

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

#[derive(Debug, Default)]
pub struct MatchingEngine {
    orderbooks: HashMap<TradingPair, OrderBook>,
}

impl MatchingEngine {
    pub fn new() -> MatchingEngine {
        MatchingEngine {
            orderbooks: HashMap::new(),
        }
    }
    pub fn add_new_market(&mut self, pair: TradingPair) {
        self.orderbooks.insert(pair.clone(), OrderBook::default());
        println!("Opening new orderbook for market {:?}", pair.to_string());
    }
    pub fn place_limit_order(
        &mut self,
        pair: TradingPair,
        side: Side,
        price: Price,
        qty: Quantity,
    ) -> Result<(), String> {
        match self.orderbooks.get_mut(&pair) {
            Some(orderbook) => {
                let _ = orderbook.add_limit_order(side, price, qty);
                println!("Placed limit order at price level{:?}", price);
                Ok(())
            }
            None => Err(format!(
                "The order book for the given trading pair ({}) does not exist",
                pair
            )),
        }
    }
}
