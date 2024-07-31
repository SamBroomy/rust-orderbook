use super::orders::*;
use super::price_levels::SparseVec;
use super::types::*;

use std::collections::{BTreeSet, HashMap, VecDeque};

#[derive(Debug)]
pub struct HalfBook {
    s: Side,
    // Price & Index of price Level
    price_map: BTreeSet<Price>,
    price_levels: SparseVec<Price, PriceLevel>,
}

impl HalfBook {
    pub fn new(s: Side) -> HalfBook {
        HalfBook {
            s,
            price_map: BTreeSet::new(),
            price_levels: SparseVec::with_capacity(10_000),
        }
    }

    pub fn add_order(&mut self, price: Price, order: TradeOrder) {
        if let Some(level) = self.price_levels.get_mut(&price) {
            level.push_back(order);
        } else {
            self.price_map.insert(price);
            self.price_levels.insert(price, VecDeque::from(vec![order]));
        }
    }

    pub fn remove_order(&mut self, price: &Price, order_id: OrderId) -> Option<TradeOrder> {
        let level = self.price_levels.get_mut(price)?;
        let removed_order = level
            .iter()
            .position(|o| o.id == order_id)
            .map(|i| level.remove(i))??;
        if level.is_empty() {
            self.price_levels.remove(price);
            self.price_map.remove(price);
        }
        Some(removed_order)
    }

    pub fn best_price(&self) -> Option<Price> {
        match self.s {
            Side::Ask => self.price_levels.min_index(),
            Side::Bid => self.price_levels.max_index(),
        }
    }

    pub fn get_price_level(&self, price: &Price) -> Option<&PriceLevel> {
        self.price_levels.get(price)
    }

    // TODO: Improve this
    pub fn iter_prices(&self) -> impl Iterator<Item = Price> {
        match self.s {
            Side::Ask => self
                .price_map
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter(),
            Side::Bid => self
                .price_map
                .iter()
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    pub fn show_depth(&self) {
        let prices: Vec<_> = match self.s {
            Side::Ask => self.price_map.iter().rev().cloned().collect(),
            Side::Bid => self.price_map.iter().rev().cloned().collect(),
        };
        self.print_price_levels(prices.iter());
    }

    fn print_price_levels<'a, I>(&self, prices: I)
    where
        I: Iterator<Item = &'a Price>,
    {
        for price in prices {
            let level = self.get_price_level(price).unwrap();
            println!(
                "Price: {} Qty: {}",
                price,
                level.iter().fold(0, |acc, o| acc + o.remaining_qty)
            );
        }
    }

    pub fn get_total_qty(&self, price: &Price) -> Option<Price> {
        Some(
            self.price_levels
                .get(price)?
                .iter()
                .fold(0, |acc, o| acc + o.remaining_qty),
        )
    }

    pub fn get_available_quantity(&self, target_price: Price) -> Quantity {
        self.iter_prices()
            .take_while(|&p| match self.s {
                Side::Ask => p <= target_price,
                Side::Bid => p >= target_price,
            })
            .map(|p| self.get_total_qty(&p).unwrap_or(0))
            .sum()
    }
}

#[derive(Debug)]
pub struct OrderBook {
    pub asks: HalfBook,
    pub bids: HalfBook,
    // For fast order lookup / cancel OrderId -> (Side, PriceLevelIndex)
    pub order_loc: HashMap<OrderId, (Side, Price)>,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            asks: HalfBook::new(Side::Ask),
            bids: HalfBook::new(Side::Bid),
            order_loc: HashMap::with_capacity(50_000),
        }
    }

    pub fn show_depth(&self) {
        println!("Asks:");
        self.asks.show_depth();
        println!("Bids:");
        self.bids.show_depth();
    }

    pub fn best_price_liq(&self) -> Option<()> {
        println!("Best Bid Price: {}", self.best_bid()?);
        println!(
            "Bid price quantity: {}",
            self.bids.get_total_qty(&self.best_bid()?)?
        );
        println!("Best Ask Price: {}", self.best_ask()?);
        println!(
            "Ask price quantity: {}",
            self.asks.get_total_qty(&self.best_ask()?)?
        );
        println!(
            "Spread: {}",
            ((self.best_ask()? - self.best_bid()?) as f64 / self.best_ask()? as f64) as f32
        );
        Some(())
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.bids.best_price()
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.best_price()
    }

    pub fn best_prices(&self) -> (Option<Price>, Option<Price>) {
        (self.bids.best_price(), self.asks.best_price())
    }

    pub fn remove_order(&mut self, order_id: OrderId) -> Option<OrderResult> {
        let (side, price) = self.order_loc.remove(&order_id)?;
        let book = match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        let order = book.remove_order(&price, order_id)?;
        let order_request = OrderRequest::new(side, order.remaining_qty, OrderType::Limit(price));
        Some(OrderResult::new(order_request, order))
    }

    pub fn add_order(&mut self, order: OrderRequest) -> (OrderResult, Vec<TradeExecution>) {
        fn match_at_price_level(
            price: Price,
            price_level: &mut VecDeque<TradeOrder>,
            incoming_order: &mut TradeOrder,
            order_loc: &mut HashMap<OrderId, (Side, Price)>,
            taker_side: &Side,
            executions: &mut Vec<TradeExecution>,
        ) {
            while !price_level.is_empty() && incoming_order.remaining_qty > 0 {
                if let Some(mut existing_order) = price_level.pop_front() {
                    let fill_qty = existing_order.filled_by(incoming_order, price);
                    executions.push(TradeExecution::new(
                        fill_qty,
                        price,
                        incoming_order,
                        &existing_order,
                        taker_side.clone(),
                    ));

                    if existing_order.remaining_qty > 0 {
                        price_level.push_front(existing_order);
                    } else {
                        order_loc.remove(&existing_order.id);
                    }
                    if incoming_order.remaining_qty == 0 {
                        break;
                    }
                }
            }
        }

        if let OrderType::FOK(_) = order.order_type {
            let opposite_book = match order.side {
                Side::Bid => &self.asks,
                Side::Ask => &self.bids,
            };

            let available_qty = opposite_book.get_available_quantity(order.price().unwrap());
            println!("Available qty: {}", available_qty);
            println!("Order qty: {}", order.qty);
            if available_qty < order.qty {
                return (OrderResult::new_cancelled(order), Vec::new());
            }
        };

        let mut new_order = TradeOrder::new(order.qty);
        let mut executions = Vec::new();

        let opposite_book = match order.side {
            Side::Bid => &mut self.asks,
            Side::Ask => &mut self.bids,
        };

        for p in opposite_book
            .iter_prices()
            .filter(|p| match order.order_type {
                OrderType::Limit(price) | OrderType::IOC(price) | OrderType::FOK(price) => {
                    match order.side {
                        Side::Bid => price >= *p,
                        Side::Ask => price <= *p,
                    }
                }
                // Market order no filtering required
                OrderType::Market => true,
            })
        {
            if let Some(price_level) = opposite_book.price_levels.get_mut(&p) {
                match_at_price_level(
                    p,
                    price_level,
                    &mut new_order,
                    &mut self.order_loc,
                    &order.side,
                    &mut executions,
                );
                if price_level.is_empty() {
                    opposite_book.price_levels.remove(&p);
                    opposite_book.price_map.remove(&p);
                }
                if new_order.remaining_qty == 0 {
                    break;
                }
            }
        }

        let result = match order.order_type {
            OrderType::Market => OrderResult::new(order, new_order),
            OrderType::Limit(price) => {
                if new_order.remaining_qty > 0 {
                    self.add_limit_order(order.side.clone(), price, new_order.clone());
                }
                OrderResult::new(order, new_order)
            }
            OrderType::IOC(_) => OrderResult::new(order, new_order),
            OrderType::FOK(_) => OrderResult::new(order, new_order),
        };
        (result, executions)
    }

    fn add_limit_order(&mut self, side: Side, price: Price, order: TradeOrder) {
        let book = match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };
        self.order_loc.insert(order.id, (side, price));
        book.add_order(price, order);
    }

    fn spread(&self) -> Option<u64> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) if ask > bid => Some(ask - bid),
            _ => None,
        }
    }

    fn depth(&self) -> Option<(usize, usize)> {
        Some((self.asks.price_map.len(), self.bids.price_map.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_half_book_add_order() {
        let mut book = HalfBook::new(Side::Ask);
        let order = TradeOrder::new(100);
        book.add_order(10, order);
        assert_eq!(book.best_price(), Some(10));
    }

    #[test]
    fn test_half_book_remove_order() {
        let mut book = HalfBook::new(Side::Ask);
        let order = TradeOrder::new(100);
        let order_id = order.id;
        book.add_order(10, order);
        assert!(book.remove_order(&10, order_id).is_some());
        assert!(book.best_price().is_none());
    }

    #[test]
    fn test_order_book_add_order() {
        let mut book = OrderBook::new();
        let order = OrderRequest::new(Side::Ask, 100, OrderType::Limit(10));
        let (result, executions) = book.add_order(order);
        assert_eq!(result.status, OrderStatus::Open);
        assert!(executions.is_empty());
        assert_eq!(book.best_ask(), Some(10));
    }

    #[test]
    fn test_order_book_match_orders() {
        let mut book = OrderBook::new();
        let ask_order = OrderRequest::new(Side::Ask, 100, OrderType::Limit(10));
        book.add_order(ask_order);
        let bid_order = OrderRequest::new(Side::Bid, 50, OrderType::Limit(10));
        let (result, executions) = book.add_order(bid_order);
        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 50);
    }

    #[test]
    fn test_order_book_market_order() {
        let mut book = OrderBook::new();
        let ask_order = OrderRequest::new(Side::Ask, 100, OrderType::Limit(10));
        book.add_order(ask_order);
        let market_order = OrderRequest::new(Side::Bid, 50, OrderType::Market);
        let (result, executions) = book.add_order(market_order);
        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 50);
        assert_eq!(executions[0].price, 10);
    }

    // Helper function to create a limit order request
    fn limit_order(side: Side, qty: Quantity, price: Price) -> OrderRequest {
        OrderRequest::new(side, qty, OrderType::Limit(price))
    }

    #[test]
    fn test_empty_order_book() {
        let book = OrderBook::new();
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.spread(), None);
    }

    #[test]
    fn test_add_and_remove_orders() {
        let mut book = OrderBook::new();

        // Add a bid order
        let (bid_result, _) = book.add_order(limit_order(Side::Bid, 100, 10));
        assert_eq!(book.best_bid(), Some(10));

        // Add an ask order
        let (ask_result, _) = book.add_order(limit_order(Side::Ask, 100, 11));
        assert_eq!(book.best_ask(), Some(11));

        // Remove the bid order
        book.remove_order(bid_result.get_id());
        assert_eq!(book.best_bid(), None);

        // Remove the ask order
        book.remove_order(ask_result.get_id());
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn test_order_matching() {
        let mut book = OrderBook::new();

        // Add some initial orders
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 100, 11));
        book.add_order(limit_order(Side::Bid, 100, 9));

        // Add a matching bid order
        let (result, executions) = book.add_order(limit_order(Side::Bid, 150, 10));

        assert_eq!(result.status, OrderStatus::PartiallyFilled);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 100);
        assert_eq!(executions[0].price, 10);
        assert_eq!(book.best_ask(), Some(11));
        assert_eq!(book.best_bid(), Some(10));
    }

    #[test]
    fn test_market_order() {
        let mut book = OrderBook::new();

        // Add some limit orders
        book.add_order(limit_order(Side::Ask, 50, 10));
        book.add_order(limit_order(Side::Ask, 100, 11));

        // Add a market buy order
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 200, OrderType::Market));

        assert_eq!(result.status, OrderStatus::PartiallyFilledMarket);
        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 50);
        assert_eq!(executions[0].price, 10);
        assert_eq!(executions[1].qty, 100);
        assert_eq!(executions[1].price, 11);
    }

    #[test]
    fn test_ioc_order() {
        let mut book = OrderBook::new();

        // Add a limit sell order
        book.add_order(limit_order(Side::Ask, 100, 10));

        // Add an IOC buy order that partially fills
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 150, OrderType::IOC(10)));

        assert_eq!(result.status, OrderStatus::PartiallyFilled);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 100);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.best_bid(), None);
    }

    #[test]
    fn test_fok_order() {
        let mut book = OrderBook::new();

        // Add some limit sell orders
        book.add_order(limit_order(Side::Ask, 50, 10));
        book.add_order(limit_order(Side::Ask, 50, 10));

        // Add a FOK buy order that fully fills
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 100, OrderType::FOK(10)));

        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 50);
        assert_eq!(executions[1].qty, 50);
        assert_eq!(book.best_ask(), None);

        // Add a FOK buy order that doesn't fill
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 100, OrderType::FOK(9)));

        assert_eq!(result.status, OrderStatus::Cancelled);
        assert!(executions.is_empty());
    }

    #[test]
    fn test_price_levels() {
        let mut book = OrderBook::new();

        // Add multiple orders at the same price level
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 100, 11));

        assert_eq!(book.best_ask(), Some(10));

        // Match against the first price level
        let (_, executions) = book.add_order(limit_order(Side::Bid, 150, 10));

        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 100);
        assert_eq!(executions[1].qty, 50);
        assert_eq!(book.best_ask(), Some(10));

        // Match the remaining order at the first price level
        let (_, executions) = book.add_order(limit_order(Side::Bid, 100, 10));

        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 50);
        assert_eq!(book.best_ask(), Some(11));
    }

    #[test]
    fn test_order_cancellation() {
        let mut book = OrderBook::new();

        // Add some orders
        let (bid_result, _) = book.add_order(limit_order(Side::Bid, 100, 10));
        let (ask_result, _) = book.add_order(limit_order(Side::Ask, 100, 11));

        // Cancel the bid order
        let cancelled_bid = book.remove_order(bid_result.get_id()).unwrap();
        assert_eq!(cancelled_bid.status, OrderStatus::Open);
        assert_eq!(book.best_bid(), None);

        // Try to cancel the same order again
        assert!(book.remove_order(bid_result.get_id()).is_none());

        // Cancel the ask order
        let cancelled_ask = book.remove_order(ask_result.get_id()).unwrap();
        assert_eq!(cancelled_ask.status, OrderStatus::Open);
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn test_complex_matching_scenario() {
        let mut book = OrderBook::new();

        // Add initial orders
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 200, 11));
        book.add_order(limit_order(Side::Ask, 300, 12));
        book.add_order(limit_order(Side::Bid, 100, 8));
        book.add_order(limit_order(Side::Bid, 200, 7));

        println!("{:#?}", book);
        // Add a large market buy order
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 650, OrderType::Market));

        assert_eq!(result.status, OrderStatus::PartiallyFilledMarket);
        assert_eq!(executions.len(), 3);
        assert_eq!(executions[0].qty, 100);
        assert_eq!(executions[0].price, 10);
        assert_eq!(executions[1].qty, 200);
        assert_eq!(executions[1].price, 11);
        assert_eq!(executions[2].qty, 300);
        assert_eq!(executions[2].price, 12);

        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Ask, 110, OrderType::Market));

        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 100);
        assert_eq!(executions[0].price, 8);
        assert_eq!(executions[1].qty, 10);
        assert_eq!(executions[1].price, 7);

        assert_eq!(book.depth(), Some((0, 1)));
        assert_eq!(book.bids.get_available_quantity(7), 190);

        println!("{:#?}", book);
        println!("{:#?}", result);
        println!("{:#?}", executions);

        assert_eq!(book.best_ask(), None);
        assert_eq!(book.best_bid(), Some(7));
    }
}
