use super::TradeOrder;

pub type OrderId = uuid::Uuid;
pub type Quantity = u64;
pub type Price = u64;
pub type PriceLevel = std::collections::VecDeque<TradeOrder>;
pub type Timestamp = std::time::SystemTime;

#[derive(Debug, Clone)]
pub enum Side {
    Ask,
    Bid,
}

pub fn timestamp() -> Timestamp {
    std::time::SystemTime::now()
}

pub fn create_order_id() -> OrderId {
    uuid::Uuid::now_v7()
}
