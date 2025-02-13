use rust_decimal::Decimal;
use uuid::Uuid;

use super::TradeOrder;

pub type OrderId = uuid::Uuid;

pub type PriceLevel = std::collections::VecDeque<TradeOrder>;
pub type Timestamp = std::time::SystemTime;

mod side;

pub type Price = Decimal;
pub type Quantity = Decimal;

pub use side::Side;

pub fn timestamp() -> Timestamp {
    std::time::SystemTime::now()
}

pub fn create_order_id() -> OrderId {
    uuid::Uuid::now_v7()
}

pub fn create_id_from_bytes(bytes: impl AsRef<[u8]>) -> OrderId {
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, bytes.as_ref())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_timestamp() {
        let ts1 = timestamp();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = timestamp();
        assert!(ts2 > ts1);
    }

    #[test]
    fn test_create_order_id() {
        let id1 = create_order_id();
        let id2 = create_order_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_create_id_from_bytes() {
        let id1 = create_id_from_bytes("hello");
        let id2 = create_id_from_bytes("hello");
        let id3 = create_id_from_bytes("world");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
}
