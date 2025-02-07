use super::TradeOrder;
use rust_decimal::Decimal;

pub type OrderId = uuid::Uuid;

pub type PriceLevel = std::collections::VecDeque<TradeOrder>;
pub type Timestamp = std::time::SystemTime;

mod side;

pub type Price = Decimal;
pub type Quantity = Decimal;
// pub type Price = DataContainer;
// pub type Quantity = DataContainer;

pub use side::{create_id_from_bytes, create_order_id, timestamp, Side};
