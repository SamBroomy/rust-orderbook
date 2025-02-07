use super::{OrderId, Timestamp};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    Ask,
    Bid,
}

impl Side {
    pub fn opposite(&self) -> Side {
        match self {
            Side::Ask => Side::Bid,
            Side::Bid => Side::Ask,
        }
    }
}

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

    #[test]
    fn test_create_id_from_bytes() {
        let id1 = create_id_from_bytes("hello");
        let id2 = create_id_from_bytes("hello");
        let id3 = create_id_from_bytes("world");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_side_opposite() {
        assert_eq!(Side::Ask.opposite(), Side::Bid);
        assert_eq!(Side::Bid.opposite(), Side::Ask);
    }
}
