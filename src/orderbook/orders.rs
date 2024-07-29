use super::types::*;

#[derive(Debug, Clone)]
pub struct TradeOrder {
    pub id: OrderId,
    pub remaining_qty: Quantity,
    pub initial_qty: Quantity,
    pub fills: Vec<Fill>,
    pub timestamp: Timestamp,
}

impl TradeOrder {
    pub fn new(qty: Quantity) -> Self {
        Self {
            id: create_order_id(),
            remaining_qty: qty,
            initial_qty: qty,
            fills: Vec::new(),
            timestamp: timestamp(),
        }
    }

    pub fn new_with_id(qty: Quantity) -> (Self, OrderId) {
        let new = Self::new(qty);
        let id = new.id;
        (new, id)
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
}

pub struct OrderRequest {
    pub side: Side,
    pub qty: Quantity,
    pub order_type: OrderType,
}

impl From<OrderRequest> for (Side, TradeOrder, OrderId, OrderType) {
    fn from(val: OrderRequest) -> Self {
        let (trade_order, id) = TradeOrder::new_with_id(val.qty);
        (val.side, trade_order, id, val.order_type)
    }
}

impl OrderRequest {
    pub fn new(side: Side, qty: Quantity, order_type: OrderType) -> Self {
        Self {
            side,
            qty,
            order_type,
        }
    }

    pub fn price(&self) -> Option<Price> {
        match &self.order_type {
            OrderType::Limit(price) => Some(*price),
            OrderType::IOC(price) => Some(*price),
            OrderType::FOK(price) => Some(*price),
            OrderType::Market => None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum OrderType {
    Market,
    Limit(Price),
    // Immediate or Cancel
    IOC(Price),
    // Fill or Kill
    FOK(Price),
}

#[derive(Debug, Default, PartialEq)]
pub enum OrderStatus {
    #[default]
    Uninitialized,
    Open,
    Filled,
    PartiallyFilledMarket,
    PartiallyFilled,
    Cancelled,
}

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub struct OrderResult {
    trade_id: OrderId,
    side: Side,
    order_type: OrderType, // Price in order type
    initial_qty: Quantity,
    remaining_qty: Quantity,
    fills: Vec<Fill>,
    pub status: OrderStatus,
}

impl OrderResult {
    pub fn new(order: OrderRequest, trade_order: TradeOrder) -> Self {
        let status = if trade_order.remaining_qty == 0 {
            OrderStatus::Filled
        } else if trade_order.fills.is_empty() {
            match order.order_type {
                OrderType::Market => OrderStatus::Cancelled,
                OrderType::Limit(_) => OrderStatus::Open,
                OrderType::IOC(_) => OrderStatus::Cancelled,
                OrderType::FOK(_) => OrderStatus::Cancelled,
            }
        } else {
            match order.order_type {
                OrderType::Market => OrderStatus::PartiallyFilledMarket,
                OrderType::IOC(_) => OrderStatus::PartiallyFilled,
                OrderType::FOK(_) => OrderStatus::Cancelled,
                OrderType::Limit(_) => OrderStatus::PartiallyFilled,
            }
        };
        Self {
            trade_id: trade_order.id,
            side: order.side,
            order_type: order.order_type,
            initial_qty: order.qty,
            remaining_qty: trade_order.remaining_qty,
            fills: trade_order.fills,
            status,
        }
    }

    pub fn new_cancelled(order: OrderRequest) -> Self {
        Self {
            trade_id: uuid::Uuid::nil(),
            side: order.side,
            order_type: order.order_type,
            initial_qty: order.qty,
            remaining_qty: order.qty,
            fills: Vec::new(),
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

    pub fn update_remaining_qty(&mut self, qty: u64) {
        self.remaining_qty = qty;
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
        let trade_order = TradeOrder::new(100);
        let result = OrderResult::new(request, trade_order);
        assert_eq!(result.status, OrderStatus::Open);
    }
}
