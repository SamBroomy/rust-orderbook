use rust_decimal::Decimal;

use tracing::{info, warn};

use super::orders::*;
use super::price_levels::SparseVec;
use super::types::*;

use std::collections::{BTreeSet, HashMap, VecDeque};

#[derive(Debug)]
pub struct HalfBook {
    s: Side,
    // Price & Index of price Level
    price_set: BTreeSet<Price>,
    price_levels: SparseVec<Price, PriceLevel>,
}

impl HalfBook {
    pub fn new(s: Side) -> HalfBook {
        HalfBook {
            s,
            price_set: BTreeSet::new(),
            price_levels: SparseVec::with_capacity(10_000),
        }
    }

    pub fn add_order(&mut self, price: impl Into<Price>, order: TradeOrder) {
        let price = price.into();
        if let Some(level) = self.price_levels.get_mut(&price) {
            level.push_back(order);
        } else {
            self.price_set.insert(price);
            self.price_levels.insert(price, VecDeque::from(vec![order]));
        }
    }

    pub fn remove_order(&mut self, price: &Price, order_id: OrderId) -> Option<TradeOrder> {
        let level = self.price_levels.get_mut(price)?;
        let removed_order = level
            .iter()
            .position(|o| o.id == order_id)
            .map(|i| level.remove(i))?;
        if level.is_empty() {
            self.price_levels.remove(price);
            self.price_set.remove(price);
        }
        removed_order
    }

    pub fn match_order(
        &mut self,
        incoming_order: &mut TradeOrder,
        price: impl Into<Price>,
    ) -> Vec<TradeExecution> {
        let price = price.into();
        let mut executions = Vec::new();
        if let Some(price_level) = self.price_levels.get_mut(&price) {
            while !price_level.is_empty() && incoming_order.remaining_qty > Decimal::ZERO {
                if let Some(mut existing_order) = price_level.pop_front() {
                    let fill_qty = existing_order.filled_by(incoming_order, price);
                    executions.push(TradeExecution::new(
                        fill_qty,
                        price,
                        incoming_order,
                        &existing_order,
                        self.s.opposite(),
                    ));

                    if existing_order.remaining_qty > Decimal::ZERO {
                        price_level.push_front(existing_order);
                    }
                }
            }
            if price_level.is_empty() {
                self.price_levels.remove(&price);
                self.price_set.remove(&price);
            }
        }
        executions
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
                .price_set
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter(),
            Side::Bid => self
                .price_set
                .iter()
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    pub fn show_depth(&self) {
        let prices: Vec<_> = match self.s {
            Side::Ask => self.price_set.iter().rev().cloned().collect(),
            Side::Bid => self.price_set.iter().rev().cloned().collect(),
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
                level
                    .iter()
                    .fold(Decimal::ZERO, |acc, o| acc + o.remaining_qty)
            );
        }
    }

    pub fn get_total_qty(&self, price: &Price) -> Option<Price> {
        Some(
            self.price_levels
                .get(price)?
                .iter()
                .fold(Decimal::ZERO, |acc, o| acc + o.remaining_qty),
        )
    }

    pub fn get_available_quantity(&self, target_price: impl Into<Price>) -> Quantity {
        let target_price = target_price.into();
        self.iter_prices()
            .take_while(|&p| match self.s {
                Side::Ask => p <= target_price,
                Side::Bid => p >= target_price,
            })
            .map(|p| self.get_total_qty(&p).unwrap_or(Decimal::ZERO))
            .sum()
    }

    pub fn get_levels(&self) -> Vec<(Price, Quantity)> {
        self.iter_prices()
            .map(|price| (price, self.get_total_qty(&price).unwrap_or(Decimal::ZERO)))
            .collect()
    }

    pub fn get_total_volume(&self) -> Quantity {
        self.iter_prices()
            .map(|price| self.get_total_qty(&price).unwrap_or(Decimal::ZERO))
            .sum()
    }

    pub fn get_depth(&self) -> usize {
        self.price_set.len()
    }

    pub fn get_price_range(&self) -> Option<Price> {
        if self.price_set.is_empty() {
            return None;
        }
        let min = *self.price_set.iter().next()?;
        let max = *self.price_set.iter().next_back()?;
        Some(max - min)
    }

    pub fn get_orders_at_price(&self, price: impl Into<Price>) -> Option<Vec<&TradeOrder>> {
        let price = price.into();
        self.price_levels
            .get(&price)
            .map(|level| level.iter().collect())
    }

    pub fn is_empty(&self) -> bool {
        self.price_set.is_empty()
    }

    pub fn get_order(&self, price: impl Into<Price>, order_id: OrderId) -> Option<&TradeOrder> {
        let price = price.into();
        self.price_levels
            .get(&price)
            .and_then(|level| level.iter().find(|o| o.id == order_id))
    }

    pub fn get_order_mut(&mut self, price: &Price, order_id: &OrderId) -> Option<&mut TradeOrder> {
        self.price_levels
            .get_mut(price)
            .and_then(|level| level.iter_mut().find(|o| o.id == *order_id))
    }

    pub fn get_order_count(&self) -> usize {
        self.price_levels.iter().map(|(_, level)| level.len()).sum()
    }

    pub fn clear(&mut self) {
        self.price_set.clear();
        self.price_levels = SparseVec::with_capacity(10_000);
    }
}
#[derive(Debug)]
pub struct OrderBookState {
    pub asks: Vec<(Price, Quantity)>,
    pub bids: Vec<(Price, Quantity)>,
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
        Self {
            asks: HalfBook::new(Side::Ask),
            bids: HalfBook::new(Side::Bid),
            order_loc: HashMap::with_capacity(10_000),
        }
    }
}

impl OrderBook {
    fn get_mut_opposite_book(&mut self, side: &Side) -> &mut HalfBook {
        match side {
            Side::Ask => &mut self.bids,
            Side::Bid => &mut self.asks,
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
            ((self.best_ask()? - self.best_bid()?) / self.best_ask()?)
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

    pub fn delete_order(&mut self, order_id: OrderId) -> Option<OrderResult> {
        let (side, price) = self.order_loc.remove(&order_id)?;
        let book = self.get_mut_book(&side);
        let order = book.remove_order(&price, order_id)?;
        Some(OrderResult::cancelled(order))
    }

    pub fn cancel_order(
        &mut self,
        order_id: OrderId,
        qty: impl Into<Quantity>,
    ) -> Option<OrderResult> {
        let trade_order = self.get_order_mut(&order_id)?;
        trade_order.cancel(qty);
        if trade_order.remaining_qty == Decimal::ZERO {
            return self.delete_order(order_id);
        }
        Some(OrderResult::from(trade_order.clone()))
    }

    pub fn add_order(&mut self, order: OrderRequest) -> (OrderResult, Vec<TradeExecution>) {
        let opposite_book = self.get_mut_opposite_book(&order.side);
        let mut executions = Vec::new();

        if let OrderType::FOK(price) = order.order_type {
            let available_qty = opposite_book.get_available_quantity(price);
            info!("Available qty: {}", available_qty);
            info!("Order qty: {}", order.qty);
            if available_qty < order.qty {
                warn!("FOK order failed");
                return (OrderResult::from(order), executions);
            }
        };
        let mut trade_order = TradeOrder::from(order);

        let filtered_prices = opposite_book
            .iter_prices()
            .filter(|p| match &trade_order.order_type {
                // Market order no filtering required
                OrderType::Market => true,
                OrderType::Limit(price)
                | OrderType::IOC(price)
                | OrderType::FOK(price)
                | OrderType::SystemLevel(price) => match &trade_order.side {
                    Side::Bid => price >= p,
                    Side::Ask => price <= p,
                },
            })
            .collect::<Vec<_>>();

        for p in filtered_prices {
            let mut price_executions = opposite_book.match_order(&mut trade_order, p);
            executions.append(&mut price_executions);
            if trade_order.remaining_qty == Decimal::ZERO {
                break;
            }
        }

        match &trade_order.order_type {
            OrderType::Limit(price) => {
                if price > &Decimal::ZERO && trade_order.remaining_qty > Decimal::ZERO {
                    self.add_limit_order(trade_order.side, *price, trade_order.clone());
                }
            }
            OrderType::SystemLevel(price) => {
                if price > &Decimal::ZERO && trade_order.remaining_qty > Decimal::ZERO {
                    self.add_system_order(trade_order.side, *price, trade_order.clone());
                }
            }
            OrderType::Market | OrderType::IOC(_) | OrderType::FOK(_) => {}
        }
        (OrderResult::from(trade_order), executions)
    }

    pub fn add_limit_order(&mut self, side: Side, price: impl Into<Price>, order: TradeOrder) {
        let price = price.into();
        assert_eq!(self.order_loc.insert(order.id, (side, price)), None);
        self.get_mut_book(&side).add_order(price, order);
    }

    // When a system order is added to the orderbook we need to merge it with the existing order if it exists
    pub fn add_system_order(&mut self, side: Side, price: impl Into<Price>, order: TradeOrder) {
        let price = price.into();
        match self.get_order_mut(&order.id) {
            Some(existing_order) => {
                assert_eq!(existing_order.merge(order), None);
            }
            None => {
                self.order_loc.insert(order.id, (side, price));
                self.get_mut_book(&side).add_order(price, order);
            }
        };
    }

    pub fn spread(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) if ask > bid => Some(ask - bid),
            _ => None,
        }
    }

    pub fn get_depth(&self) -> (usize, usize) {
        (self.asks.get_depth(), self.bids.get_depth())
    }

    fn get_book(&self, side: &Side) -> &HalfBook {
        match side {
            Side::Ask => &self.asks,
            Side::Bid => &self.bids,
        }
    }

    fn get_mut_book(&mut self, side: &Side) -> &mut HalfBook {
        match side {
            Side::Ask => &mut self.asks,
            Side::Bid => &mut self.bids,
        }
    }

    pub fn get_order_book_state(&self) -> OrderBookState {
        let mut ask = self.asks.get_levels();
        ask.reverse();

        OrderBookState {
            asks: ask,
            bids: self.bids.get_levels(),
        }
    }

    pub fn get_orders_at_price(
        &self,
        side: Side,
        price: impl Into<Price>,
    ) -> Option<Vec<&TradeOrder>> {
        self.get_book(&side).get_orders_at_price(price)
    }

    pub fn get_total_volume(&self) -> Quantity {
        self.asks.get_total_volume() + self.bids.get_total_volume()
    }

    pub fn get_price_range(&self) -> Option<(Price, Price)> {
        Some((self.asks.get_price_range()?, self.bids.get_price_range()?))
    }

    pub fn get_order(&self, order_id: OrderId) -> Option<&TradeOrder> {
        self.order_loc
            .get(&order_id)
            .and_then(|(side, price)| self.get_book(side).get_order(*price, order_id))
    }

    pub fn get_order_mut(&mut self, order_id: &OrderId) -> Option<&mut TradeOrder> {
        self.order_loc
            .get(order_id)
            .and_then(|(side, price)| match side {
                Side::Ask => self.asks.get_order_mut(price, order_id),
                Side::Bid => self.bids.get_order_mut(price, order_id),
            })
    }

    pub fn get_volume_at_price(&self, side: &Side, price: &Price) -> Option<Quantity> {
        self.get_book(side).get_total_qty(price)
    }

    pub fn get_order_count(&self) -> usize {
        self.order_loc.len()
    }

    pub fn is_empty(&self) -> bool {
        self.asks.is_empty() && self.bids.is_empty()
    }

    pub fn clear(&mut self) {
        self.asks.clear();
        self.bids.clear();
        self.order_loc.clear();
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
        assert_eq!(book.best_price(), Some(10.into()));
    }

    #[test]
    fn test_half_book_remove_order() {
        let mut book = HalfBook::new(Side::Ask);
        let order = TradeOrder::new(100);
        let order_id = order.id;
        book.add_order(10, order);
        assert!(book.remove_order(&10.into(), order_id).is_some());
        assert!(book.best_price().is_none());
    }

    #[test]
    fn test_order_book_add_order() {
        let mut book = OrderBook::default();
        let order = OrderRequest::new(Side::Ask, 100, OrderType::limit(10));
        let (result, executions) = book.add_order(order);
        assert_eq!(result.status, OrderStatus::Open);
        assert!(executions.is_empty());
        assert_eq!(book.best_ask(), Some(10.into()));
    }

    #[test]
    fn test_order_book_add_system_order() {
        let mut book = OrderBook::default();
        let order1 = OrderRequest::new(Side::Ask, 100, OrderType::system_level(10));
        let id1 = order1.id();
        let (result, executions) = book.add_order(order1);
        assert_eq!(result.status, OrderStatus::Open);
        assert!(executions.is_empty());
        assert_eq!(book.best_ask(), Some(10.into()));
        // Test adding a second order at the same price level to test merging
        let order2 = OrderRequest::new(Side::Ask, 50, OrderType::system_level(10));
        let id2 = order2.id();
        assert_eq!(id1, id2);
        let (result, executions) = book.add_order(order2);
        assert_eq!(result.status, OrderStatus::Open);
        assert!(executions.is_empty());
        assert_eq!(book.best_ask(), Some(10.into()));

        let order1 = book.get_order(id1).unwrap();
        assert_eq!(order1.remaining_qty, 150.into());
        let order3 = OrderRequest::new(Side::Bid, 200, OrderType::system_level(10));
        let (result, executions) = book.add_order(order3);
        assert_eq!(result.status, OrderStatus::PartiallyFilled);
        assert!(!executions.is_empty());
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.best_bid(), Some(10.into()));
    }

    #[test]
    fn test_order_book_match_orders() {
        let mut book = OrderBook::default();
        let ask_order = OrderRequest::new(Side::Ask, 100, OrderType::limit(10));
        book.add_order(ask_order);
        let bid_order = OrderRequest::new(Side::Bid, 50, OrderType::limit(10));
        let (result, executions) = book.add_order(bid_order);
        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 50.into());
    }

    #[test]
    fn test_order_book_market_order() {
        let mut book = OrderBook::default();
        let ask_order = OrderRequest::new(Side::Ask, 100, OrderType::limit(10));
        book.add_order(ask_order);
        let market_order = OrderRequest::new(Side::Bid, 50, OrderType::Market);
        let (result, executions) = book.add_order(market_order);
        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 50.into());
        assert_eq!(executions[0].price, 10.into());
    }

    // Helper function to create a limit order request
    fn limit_order(side: Side, qty: impl Into<Quantity>, price: impl Into<Price>) -> OrderRequest {
        OrderRequest::new(side, qty, OrderType::limit(price))
    }

    #[test]
    fn test_empty_order_book() {
        let book = OrderBook::default();
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.spread(), None);
    }

    #[test]
    fn test_add_and_remove_orders() {
        let mut book = OrderBook::default();

        // Add a bid order
        let (bid_result, _) = book.add_order(limit_order(Side::Bid, 100, 10));
        assert_eq!(book.best_bid(), Some(10.into()));

        // Add an ask order
        let (ask_result, _) = book.add_order(limit_order(Side::Ask, 100, 11));
        assert_eq!(book.best_ask(), Some(11.into()));

        // Remove the bid order
        book.delete_order(bid_result.get_id());
        assert_eq!(book.best_bid(), None);

        // Remove the ask order
        book.delete_order(ask_result.get_id());
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn test_order_matching() {
        let mut book = OrderBook::default();

        // Add some initial orders
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 100, 11));
        book.add_order(limit_order(Side::Bid, 100, 9));

        // Add a matching bid order
        let (result, executions) = book.add_order(limit_order(Side::Bid, 150, 10));

        assert_eq!(result.status, OrderStatus::PartiallyFilled);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 100.into());
        assert_eq!(executions[0].price, 10.into());
        assert_eq!(book.best_ask(), Some(11.into()));
        assert_eq!(book.best_bid(), Some(10.into()));
    }

    #[test]
    fn test_market_order() {
        let mut book = OrderBook::default();

        // Add some limit orders
        book.add_order(limit_order(Side::Ask, 50, 10));
        book.add_order(limit_order(Side::Ask, 100, 11));

        // Add a market buy order
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 200, OrderType::Market));

        assert_eq!(result.status, OrderStatus::PartiallyFilled);
        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 50.into());
        assert_eq!(executions[0].price, 10.into());
        assert_eq!(executions[1].qty, 100.into());
        assert_eq!(executions[1].price, 11.into());
    }

    #[test]
    fn test_ioc_order() {
        let mut book = OrderBook::default();

        // Add a limit sell order
        book.add_order(limit_order(Side::Ask, 100, 10));

        // Add an IOC buy order that partially fills
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 150, OrderType::ioc(10)));

        assert_eq!(result.status, OrderStatus::PartiallyFilled);
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 100.into());
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.best_bid(), None);
    }

    #[test]
    fn test_fok_order() {
        let mut book = OrderBook::default();

        // Add some limit sell orders
        book.add_order(limit_order(Side::Ask, 50, 10));
        book.add_order(limit_order(Side::Ask, 50, 10));

        // Add a FOK buy order that fully fills
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 100, OrderType::fok(10)));

        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 50.into());
        assert_eq!(executions[1].qty, 50.into());
        assert_eq!(book.best_ask(), None);

        // Add a FOK buy order that doesn't fill
        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Bid, 100, OrderType::fok(9)));

        assert_eq!(result.status, OrderStatus::Cancelled);
        assert!(executions.is_empty());
    }

    #[test]
    fn test_price_levels() {
        let mut book = OrderBook::default();

        // Add multiple orders at the same price level
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 100, 11));

        assert_eq!(book.best_ask(), Some(10.into()));

        // Match against the first price level
        let (_, executions) = book.add_order(limit_order(Side::Bid, 150, 10));

        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 100.into());
        assert_eq!(executions[1].qty, 50.into());
        assert_eq!(book.best_ask(), Some(10.into()));

        // Match the remaining order at the first price level
        let (_, executions) = book.add_order(limit_order(Side::Bid, 100, 10));

        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].qty, 50.into());
        assert_eq!(book.best_ask(), Some(11.into()));
    }

    #[test]
    fn test_order_deletion() {
        let mut book = OrderBook::default();

        // Add some orders
        let (bid_result, _) = book.add_order(limit_order(Side::Bid, 100, 10));
        let (ask_result, _) = book.add_order(limit_order(Side::Ask, 100, 11));

        // Cancel the bid order
        let cancelled_bid = book.delete_order(bid_result.get_id()).unwrap();
        assert_eq!(cancelled_bid.status, OrderStatus::Cancelled);
        assert_eq!(book.best_bid(), None);

        // Try to cancel the same order again
        assert!(book.delete_order(bid_result.get_id()).is_none());

        // Cancel the ask order
        let cancelled_ask = book.delete_order(ask_result.get_id()).unwrap();
        assert_eq!(cancelled_ask.status, OrderStatus::Cancelled);
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn test_complex_matching_scenario() {
        let mut book = OrderBook::default();

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

        assert_eq!(result.status, OrderStatus::PartiallyFilled);
        assert_eq!(executions.len(), 3);
        assert_eq!(executions[0].qty, 100.into());
        assert_eq!(executions[0].price, 10.into());
        assert_eq!(executions[1].qty, 200.into());
        assert_eq!(executions[1].price, 11.into());
        assert_eq!(executions[2].qty, 300.into());
        assert_eq!(executions[2].price, 12.into());

        let (result, executions) =
            book.add_order(OrderRequest::new(Side::Ask, 110, OrderType::Market));

        assert_eq!(result.status, OrderStatus::Filled);
        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 100.into());
        assert_eq!(executions[0].price, 8.into());
        assert_eq!(executions[1].qty, 10.into());
        assert_eq!(executions[1].price, 7.into());

        assert_eq!(book.get_depth(), (0, 1));
        assert_eq!(book.bids.get_available_quantity(7), 190.into());

        println!("{:#?}", book);
        println!("{:#?}", result);
        println!("{:#?}", executions);

        assert_eq!(book.best_ask(), None);
        assert_eq!(book.best_bid(), Some(7.into()));
    }
    #[test]
    fn test_half_book_get_levels() {
        let mut book = HalfBook::new(Side::Ask);
        assert_eq!(book.get_levels(), vec![]);
        book.add_order(10, TradeOrder::new(100));
        book.add_order(10, TradeOrder::new(50));
        book.add_order(11, TradeOrder::new(75));

        let levels = book.get_levels();
        assert_eq!(
            levels,
            vec![(10.into(), 150.into()), (11.into(), 75.into())]
        );
    }

    #[test]
    fn test_half_book_get_total_volume() {
        let mut book = HalfBook::new(Side::Bid);
        assert_eq!(book.get_total_volume(), 0.into());
        book.add_order(10, TradeOrder::new(100));
        book.add_order(11, TradeOrder::new(50));
        book.add_order(9, TradeOrder::new(75));

        assert_eq!(book.get_total_volume(), 225.into());
    }

    #[test]
    fn test_half_book_get_orders_at_price() {
        let mut book = HalfBook::new(Side::Ask);
        let order1 = TradeOrder::new(100);
        let order2 = TradeOrder::new(50);
        book.add_order(10, order1.clone());
        book.add_order(10, order2.clone());

        let orders = book.get_orders_at_price(10).unwrap();
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].remaining_qty, 100.into());
        assert_eq!(orders[1].remaining_qty, 50.into());
    }

    #[test]
    fn test_order_book_get_order_book_state() {
        let mut book = OrderBook::default();
        let state = book.get_order_book_state();
        assert_eq!(state.asks, vec![]);
        assert_eq!(state.bids, vec![]);
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 50, 11));
        book.add_order(limit_order(Side::Bid, 75, 9));
        book.add_order(limit_order(Side::Bid, 25, 8));

        let state = book.get_order_book_state();
        assert_eq!(
            state.asks,
            vec![(11.into(), 50.into()), (10.into(), 100.into())]
        );
        assert_eq!(
            state.bids,
            vec![(9.into(), 75.into()), (8.into(), 25.into())]
        );
    }

    #[test]
    fn test_order_book_get_orders_at_price() {
        let mut book = OrderBook::default();
        assert_eq!(book.get_orders_at_price(Side::Ask, 10), None);
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 50, 10));

        let ask_orders = book.get_orders_at_price(Side::Ask, 10).unwrap();
        assert_eq!(ask_orders.len(), 2);
        assert_eq!(ask_orders[0].remaining_qty, 100.into());
        assert_eq!(ask_orders[1].remaining_qty, 50.into());

        assert!(book.get_orders_at_price(Side::Bid, 10).is_none());
    }

    #[test]
    fn test_order_book_get_total_volume() {
        let mut book = OrderBook::default();
        assert_eq!(book.get_total_volume(), 0.into());
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 50, 11));
        book.add_order(limit_order(Side::Bid, 75, 9));
        book.add_order(limit_order(Side::Bid, 25, 8));

        assert_eq!(book.get_total_volume(), 250.into());
    }

    #[test]
    fn test_order_book_depth_and_range() {
        let mut book = OrderBook::default();
        assert_eq!(book.get_depth(), (0, 0));
        assert_eq!(book.get_price_range(), None);

        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 50, 15));
        book.add_order(limit_order(Side::Bid, 75, 8));
        book.add_order(limit_order(Side::Bid, 25, 5));

        assert_eq!(book.get_depth(), (2, 2));
        assert_eq!(book.get_price_range(), Some((5.into(), 3.into())));
    }

    #[test]
    fn test_half_book_match_order() {
        let mut book = HalfBook::new(Side::Ask);
        book.add_order(10, TradeOrder::new(100));
        book.add_order(10, TradeOrder::new(50));
        book.add_order(11, TradeOrder::new(75));

        let mut incoming_order = TradeOrder::new(125);
        let executions = book.match_order(&mut incoming_order, 10);

        assert_eq!(executions.len(), 2);
        assert_eq!(executions[0].qty, 100.into());
        assert_eq!(executions[1].qty, 25.into());
        assert_eq!(incoming_order.remaining_qty, 0.into());
        assert_eq!(book.get_total_qty(&10.into()), Some(25.into()));
        assert_eq!(book.get_total_qty(&11.into()), Some(75.into()));
    }

    #[test]
    fn test_orderbook_get_order() {
        let mut book = OrderBook::default();
        let (result, _) = book.add_order(limit_order(Side::Ask, 100, 10));
        let order_id = result.get_id();

        assert!(book.get_order(order_id).is_some());
        assert_eq!(book.get_order(order_id).unwrap().remaining_qty, 100.into());
        assert!(book.get_order(uuid::Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_orderbook_get_order_mut() {
        let mut book = OrderBook::default();
        let (result, _) = book.add_order(limit_order(Side::Ask, 100, 10));
        let order_id = result.get_id();

        if let Some(order) = book.get_order_mut(&order_id) {
            order.remaining_qty = 50.into();
        }

        assert_eq!(book.get_order(order_id).unwrap().remaining_qty, 50.into());
    }

    #[test]
    fn test_orderbook_get_volume_at_price() {
        let mut book = OrderBook::default();
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 50, 10));
        book.add_order(limit_order(Side::Bid, 75, 9));

        assert_eq!(
            book.get_volume_at_price(&Side::Ask, &10.into()),
            Some(150.into())
        );
        assert_eq!(
            book.get_volume_at_price(&Side::Bid, &9.into()),
            Some(75.into())
        );
        assert_eq!(book.get_volume_at_price(&Side::Ask, &11.into()), None);
    }

    #[test]
    fn test_orderbook_get_order_count() {
        let mut book = OrderBook::default();
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Ask, 50, 11));
        book.add_order(limit_order(Side::Bid, 75, 9));

        assert_eq!(book.get_order_count(), 3);
    }

    #[test]
    fn test_orderbook_is_empty() {
        let mut book = OrderBook::default();
        assert!(book.is_empty());

        book.add_order(limit_order(Side::Ask, 100, 10));
        assert!(!book.is_empty());
    }

    #[test]
    fn test_orderbook_clear() {
        let mut book = OrderBook::default();
        book.add_order(limit_order(Side::Ask, 100, 10));
        book.add_order(limit_order(Side::Bid, 75, 9));

        assert!(!book.is_empty());
        book.clear();
        assert!(book.is_empty());
        assert_eq!(book.get_order_count(), 0);
    }

    #[test]
    fn test_halfbook_is_empty() {
        let mut book = HalfBook::new(Side::Ask);
        assert!(book.is_empty());

        book.add_order(10, TradeOrder::new(100));
        assert!(!book.is_empty());
    }

    #[test]
    fn test_halfbook_get_order() {
        let mut book = HalfBook::new(Side::Ask);
        let order = TradeOrder::new(100);
        let order_id = order.id;
        book.add_order(10, order);

        assert!(book.get_order(10, order_id).is_some());
        assert_eq!(
            book.get_order(10, order_id).unwrap().remaining_qty,
            100.into()
        );
        assert!(book.get_order(10, uuid::Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_halfbook_get_order_mut() {
        let mut book = HalfBook::new(Side::Ask);
        let order = TradeOrder::new(100);
        let order_id = order.id;
        book.add_order(10, order);

        if let Some(order) = book.get_order_mut(&10.into(), &order_id) {
            order.remaining_qty = 50.into();
        }

        assert_eq!(
            book.get_order(10, order_id).unwrap().remaining_qty,
            50.into()
        );
    }

    #[test]
    fn test_halfbook_get_order_count() {
        let mut book = HalfBook::new(Side::Ask);
        assert_eq!(book.get_order_count(), 0);
        book.add_order(10, TradeOrder::new(100));
        book.add_order(10, TradeOrder::new(50));
        book.add_order(11, TradeOrder::new(75));

        assert_eq!(book.get_order_count(), 3);
    }

    #[test]
    fn test_halfbook_clear() {
        let mut book = HalfBook::new(Side::Ask);
        assert!(book.is_empty());
        book.add_order(10, TradeOrder::new(100));
        book.add_order(11, TradeOrder::new(75));

        assert!(!book.is_empty());
        book.clear();
        assert!(book.is_empty());
        assert_eq!(book.get_order_count(), 0);
    }

    #[test]
    fn test_order_cancellation() {
        let mut book = OrderBook::default();

        // Add some orders
        let (bid_result, _) = book.add_order(limit_order(Side::Bid, 100, 10));
        let (ask_result, _) = book.add_order(limit_order(Side::Ask, 100, 11));

        // Cancel the bid order
        let cancelled_bid = book.cancel_order(bid_result.get_id(), 50).unwrap();
        assert_eq!(cancelled_bid.status, OrderStatus::Open);
        assert_eq!(cancelled_bid.remaining_qty, 50.into());

        // Try to cancel the same order again
        let cancelled_bid = book.cancel_order(bid_result.get_id(), 50).unwrap();
        assert_eq!(cancelled_bid.status, OrderStatus::Cancelled);
        assert_eq!(cancelled_bid.remaining_qty, 0.into());

        // Cancel the ask order
        let cancelled_ask = book.cancel_order(ask_result.get_id(), 110).unwrap();
        assert_eq!(cancelled_ask.status, OrderStatus::Cancelled);
        assert_eq!(cancelled_ask.remaining_qty, 0.into());

        // Try to cancel the same order again
        book.cancel_order(ask_result.get_id(), 50);
        assert!(book.get_order(ask_result.get_id()).is_none());
    }
}
