use std::fmt::Display;

use super::types::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum OrderType {
    Market,
    Limit(Price),
    // Immediate or Cancel
    IOC(Price),
    // Fill or Kill
    FOK(Price),
}

impl Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Market => write!(f, "Market"),
            OrderType::Limit(_) => write!(f, "Limit"),
            OrderType::IOC(_) => write!(f, "IOC"),
            OrderType::FOK(_) => write!(f, "FOK"),
        }
    }
}

impl OrderType {
    pub fn price(&self) -> Option<Price> {
        match self {
            OrderType::Limit(price) => Some(*price),
            OrderType::IOC(price) => Some(*price),
            OrderType::FOK(price) => Some(*price),
            OrderType::Market => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OrderStatus {
    Open,
    Filled,
    PartiallyFilled,
    Cancelled,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Fill {
    pub qty: Quantity,
    pub price: Price,
    pub timestamp: Timestamp,
    pub order_id: OrderId,
}

impl Fill {
    pub fn new(qty: Quantity, price: Price, order_id: OrderId) -> Self {
        Self {
            qty,
            price,
            timestamp: timestamp(),
            order_id,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct OrderRequest {
    id: OrderId,
    pub side: Side,
    pub qty: Quantity,
    pub order_type: OrderType,
}

impl OrderRequest {
    pub fn new(side: Side, qty: Quantity, order_type: OrderType) -> Self {
        Self {
            id: create_order_id(),
            side,
            qty,
            order_type,
        }
    }

    pub fn new_with_id(id: OrderId, side: Side, qty: Quantity, order_type: OrderType) -> Self {
        Self {
            id,
            side,
            qty,
            order_type,
        }
    }

    pub fn new_with_other_id(
        id: impl AsRef<[u8]>,
        side: Side,
        qty: Quantity,
        order_type: OrderType,
    ) -> Self {
        Self {
            id: create_id_from_bytes(id),
            side,
            qty,
            order_type,
        }
    }

    pub fn price(&self) -> Option<Price> {
        self.order_type.price()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TradeOrder {
    pub id: OrderId,
    pub side: Side,
    pub remaining_qty: Quantity,
    initial_qty: Quantity,
    fills: Vec<Fill>,
    pub order_type: OrderType,
    timestamp: Timestamp,
}

impl From<OrderRequest> for TradeOrder {
    fn from(order_request: OrderRequest) -> Self {
        Self {
            id: order_request.id,
            side: order_request.side,
            remaining_qty: order_request.qty,
            initial_qty: order_request.qty,
            fills: Vec::new(),
            order_type: order_request.order_type,
            timestamp: timestamp(),
        }
    }
}

impl TradeOrder {
    pub fn new(qty: Quantity) -> Self {
        Self {
            id: create_order_id(),
            side: Side::Ask,
            remaining_qty: qty,
            initial_qty: qty,
            fills: Vec::new(),
            order_type: OrderType::Market,
            timestamp: timestamp(),
        }
    }

    /// Fills the order with the given quantity and price and returns the remaining quantity if the order was fully filled.
    pub fn fill(&mut self, qty: &mut Quantity, price: Price, order_id: OrderId) {
        let fill_qty = (*qty).min(self.remaining_qty);
        self.remaining_qty -= fill_qty;
        self.fills.push(Fill::new(fill_qty, price, order_id));
        *qty -= fill_qty;
    }

    pub fn filled_by(&mut self, other: &mut TradeOrder, price: Price) -> Quantity {
        let fill_qty = other.remaining_qty.min(self.remaining_qty);
        self.remaining_qty -= fill_qty;
        other.remaining_qty -= fill_qty;
        self.fills.push(Fill::new(fill_qty, price, other.id));
        other.fills.push(Fill::new(fill_qty, price, self.id));
        fill_qty
    }

    pub fn filled_quantity(&self) -> Quantity {
        self.initial_qty - self.remaining_qty
    }

    /// Cancels the order with the given quantity and returns the remaining quantity if the order was fully cancelled.
    pub fn cancel(&mut self, qty: Quantity) {
        let qty = qty.min(self.remaining_qty);
        self.remaining_qty -= qty;
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OrderResult {
    trade_id: OrderId,
    side: Side,
    order_type: OrderType,
    initial_qty: Quantity,
    pub remaining_qty: Quantity,
    fills: Vec<Fill>,
    pub status: OrderStatus,
}

impl From<TradeOrder> for OrderResult {
    fn from(trade_order: TradeOrder) -> Self {
        let status = if trade_order.remaining_qty == 0 {
            OrderStatus::Filled
        } else if trade_order.fills.is_empty() {
            match trade_order.order_type {
                OrderType::Market => OrderStatus::Cancelled,
                OrderType::Limit(_) => OrderStatus::Open,
                OrderType::IOC(_) => OrderStatus::Cancelled,
                OrderType::FOK(_) => OrderStatus::Cancelled,
            }
        } else {
            match trade_order.order_type {
                OrderType::Market => OrderStatus::PartiallyFilled,
                OrderType::Limit(_) => OrderStatus::PartiallyFilled,
                OrderType::IOC(_) => OrderStatus::PartiallyFilled,
                OrderType::FOK(_) => OrderStatus::Cancelled,
            }
        };
        Self {
            trade_id: trade_order.id,
            side: trade_order.side,
            order_type: trade_order.order_type,
            initial_qty: trade_order.initial_qty,
            remaining_qty: trade_order.remaining_qty,
            fills: trade_order.fills,
            status,
        }
    }
}

impl From<OrderRequest> for OrderResult {
    fn from(order_request: OrderRequest) -> Self {
        Self {
            trade_id: order_request.id,
            side: order_request.side,
            order_type: order_request.order_type,
            initial_qty: order_request.qty,
            remaining_qty: order_request.qty,
            fills: Vec::new(),
            status: OrderStatus::Cancelled,
        }
    }
}

impl OrderResult {
    pub fn cancelled(trade_order: TradeOrder) -> Self {
        Self {
            trade_id: trade_order.id,
            side: trade_order.side,
            order_type: trade_order.order_type,
            initial_qty: trade_order.initial_qty,
            remaining_qty: trade_order.remaining_qty,
            fills: trade_order.fills,
            status: OrderStatus::Cancelled,
        }
    }

    pub fn avr_fill_price(&self) -> f32 {
        let mut total = 0;
        let mut qty = 0;
        for fill in &self.fills {
            total += fill.price * fill.qty;
            qty += fill.qty;
        }
        total as f32 / qty as f32
    }

    pub fn get_id(&self) -> OrderId {
        self.trade_id
    }
}

#[derive(Debug, Clone)]
pub struct TradeExecution {
    pub qty: Quantity,
    pub price: Price,
    pub taker_order_id: OrderId,
    pub maker_order_id: OrderId,
    pub take_side: Side,
    pub timestamp: Timestamp,
}

impl TradeExecution {
    pub fn new(
        qty: Quantity,
        price: Price,
        taker_order: &TradeOrder,
        maker_order: &TradeOrder,
        taker_side: Side,
    ) -> Self {
        Self {
            price,
            qty,
            taker_order_id: taker_order.id,
            maker_order_id: maker_order.id,
            take_side: taker_side,
            timestamp: timestamp(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_order_creation() {
        let order = TradeOrder::new(100);
        assert_eq!(order.remaining_qty, 100);
        assert_eq!(order.initial_qty, 100);
        assert!(order.fills.is_empty());
    }

    #[test]
    fn test_trade_order_fill() {
        let mut order = TradeOrder::new(100);
        let mut fill_qty = 60;
        order.fill(&mut fill_qty, 10, create_order_id());
        assert_eq!(order.remaining_qty, 40);
        assert_eq!(order.fills.len(), 1);
        assert_eq!(fill_qty, 0);
    }

    #[test]
    fn test_trade_order_fill_by_same_quantity() {
        let mut order1 = TradeOrder::new(100);
        let mut order2 = TradeOrder::new(100);
        let fill_qty = order1.filled_by(&mut order2, 10);
        assert_eq!(fill_qty, 100);
        assert_eq!(order1.remaining_qty, 0);
        assert_eq!(order2.remaining_qty, 0);
        assert_eq!(order1.fills.len(), 1);
        assert_eq!(order2.fills.len(), 1);
    }

    #[test]
    fn test_trade_order_fill_by_larger_quantity() {
        let mut order1 = TradeOrder::new(50);
        let mut order2 = TradeOrder::new(100);
        let fill_qty = order1.filled_by(&mut order2, 10);
        assert_eq!(fill_qty, 50);
        assert_eq!(order1.remaining_qty, 0);
        assert_eq!(order2.remaining_qty, 50);
        assert_eq!(order1.fills.len(), 1);
        assert_eq!(order2.fills.len(), 1);
    }

    #[test]
    fn test_trade_order_fill_by_many_smaller_quantities() {
        let mut order1 = TradeOrder::new(100);
        let mut order2 = TradeOrder::new(10);
        let mut order3 = TradeOrder::new(10);
        let mut order4 = TradeOrder::new(10);
        let fill_qty = order1.filled_by(&mut order2, 10);
        assert_eq!(fill_qty, 10);
        let fill_qty = order1.filled_by(&mut order3, 10);
        assert_eq!(fill_qty, 10);
        let fill_qty = order1.filled_by(&mut order4, 10);
        assert_eq!(fill_qty, 10);
        assert_eq!(order1.remaining_qty, 70);
        assert_eq!(order2.remaining_qty, 0);
        assert_eq!(order3.remaining_qty, 0);
        assert_eq!(order4.remaining_qty, 0);
        assert_eq!(order1.fills.len(), 3);
        assert_eq!(order2.fills.len(), 1);
        assert_eq!(order3.fills.len(), 1);
        assert_eq!(order4.fills.len(), 1);
    }

    #[test]
    fn test_order_request() {
        let request = OrderRequest::new(Side::Ask, 100, OrderType::Limit(10));
        assert_eq!(request.price(), Some(10));
        let request = OrderRequest::new(Side::Bid, 100, OrderType::Market);
        assert_eq!(request.price(), None);
    }

    #[test]
    fn test_order_result() {
        let request = OrderRequest::new(Side::Ask, 100, OrderType::Limit(10));
        let id = request.id;
        let trade_order = TradeOrder::from(request);
        let result = OrderResult::from(trade_order);
        assert_eq!(result.status, OrderStatus::Open);
        assert_eq!(result.remaining_qty, 100);
        assert_eq!(result.initial_qty, 100);
        assert_eq!(result.fills.len(), 0);
        assert_eq!(result.get_id(), id);
    }
}
