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
    asks: HalfBook,
    bids: HalfBook,
    // For fast order lookup / cancel OrderId -> (Side, PriceLevelIndex)
    order_loc: HashMap<OrderId, (Side, Price)>,
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

        let (mut new_order, id) = TradeOrder::new_with_id(order.qty);
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
