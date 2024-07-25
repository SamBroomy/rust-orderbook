mod order_book;
mod price_levels;

pub use order_book::*;

use rand::Rng;
use std::collections::VecDeque;

pub type OrderId = u64;
pub type Quantity = u64;
pub type Price = u64;
pub type PriceLevel = VecDeque<TradeOrder>;

#[derive(Debug)]
pub enum Side {
    Ask,
    Bid,
}

#[derive(Debug)]
pub struct TradeOrder {
    id: OrderId,
    qty: Quantity,
}

impl TradeOrder {
    pub fn new(qty: Quantity) -> Self {
        let mut rng = rand::thread_rng();
        let id = rng.gen::<OrderId>();

        Self { id, qty }
    }

    pub fn id(&self) -> OrderId {
        self.id
    }

    pub fn qty(&self) -> Quantity {
        self.qty
    }
}

pub struct Order {
    side: Side,
    qty: Quantity,
    order_type: OrderType,
}

#[derive(Debug, PartialEq)]
pub enum OrderType {
    Market,
    Limit(Price),
}

#[derive(Debug, Default)]
pub enum OrderStatus {
    #[default]
    Uninitialized,
    Open(OrderId),
    Filled,
    PartiallyFilledMarket,
    PartiallyFilled(OrderId),
    Cancelled,
}

#[derive(Debug)]
pub struct FillResult {
    // Orders filled (qty, price)
    filled_orders: Vec<(u64, Price)>,
    remaining_qty: u64,
    pub status: OrderStatus,
}

impl Default for FillResult {
    fn default() -> Self {
        FillResult {
            filled_orders: Vec::new(),
            remaining_qty: u64::MAX,
            status: OrderStatus::default(),
        }
    }
}

impl FillResult {
    pub fn avr_fill_price(&self) -> f32 {
        let mut total = 0;
        let mut qty = 0;
        for (q, p) in &self.filled_orders {
            total += q * p;
            qty += q;
        }
        total as f32 / qty as f32
    }

    pub fn update_remaining_qty(&mut self, qty: u64) {
        self.remaining_qty = qty;
    }
}
