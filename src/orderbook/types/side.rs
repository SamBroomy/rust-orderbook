

/// Side of the order, either Ask or Bid.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_enum() {
        assert!(matches!(Side::Ask, Side::Ask));
        assert!(matches!(Side::Bid, Side::Bid));
    }

    #[test]
    fn test_side_opposite() {
        assert_eq!(Side::Ask.opposite(), Side::Bid);
        assert_eq!(Side::Bid.opposite(), Side::Ask);
    }
}
