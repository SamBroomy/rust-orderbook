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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_enum() {
        assert!(matches!(Side::Ask, Side::Ask));
        assert!(matches!(Side::Bid, Side::Bid));
    }

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
}
