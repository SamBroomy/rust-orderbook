use super::{
    price_levels::SparseVec, FillResult, Order, OrderId, OrderStatus, OrderType, Price, PriceLevel,
    Quantity, Side, TradeOrder,
};
use crate::Result;

use std::collections::{BTreeSet, HashMap, VecDeque};

#[derive(Debug)]
pub(super) struct HalfBook {
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
        if let Some(level) = self.price_levels.get_mut(price) {
            let removed = level
                .iter()
                .position(|o| o.id == order_id)
                .map(|i| level.remove(i))?;
            if level.is_empty() {
                self.price_levels.remove(price);
                self.price_map.remove(price);
            }
            removed
        } else {
            None
        }
    }

    pub fn best_prices(&self) -> Option<Price> {
        match self.s {
            Side::Ask => self.price_levels.min_index(),
            Side::Bid => self.price_levels.max_index(),
        }
    }

    pub fn best_price(&self) -> Option<Price> {
        match self.s {
            Side::Ask => self.price_map.iter().next().cloned(),
            Side::Bid => self.price_map.iter().next_back().cloned(),
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
        let prices: Vec<Price> = self.iter_prices().collect();
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
                level.iter().fold(0, |acc, o| acc + o.qty)
            );
        }
    }

    pub fn get_total_qty(&self, price: &Price) -> Option<Price> {
        Some(
            self.price_levels
                .get(price)?
                .iter()
                .fold(0, |acc, o| acc + o.qty),
        )
    }
}

#[derive(Debug)]
pub struct OrderBook {
    bids: HalfBook,
    asks: HalfBook,
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
            bids: HalfBook::new(Side::Bid),
            asks: HalfBook::new(Side::Ask),
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
        println!("Best Bid Price: {}", self.best_bid_price()?);
        println!(
            "Bid price quantity: {}",
            self.bids.get_total_qty(&self.best_bid_price()?)?
        );
        println!("Best Ask Price: {}", self.best_ask_price()?);
        println!(
            "Ask price quantity: {}",
            self.asks.get_total_qty(&self.best_ask_price()?)?
        );
        println!(
            "Spread: {}",
            ((self.best_ask_price()? - self.best_bid_price()?) as f64
                / self.best_ask_price()? as f64) as f32
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
        (self.bids.best_prices(), self.asks.best_prices())
    }
    // TODO: ARE THESE THE SAME?
    pub fn best_price(&self) -> (Option<Price>, Option<Price>) {
        (self.bids.best_price(), self.asks.best_price())
    }

    pub fn remove_order(&mut self, order_id: OrderId) -> Option<TradeOrder> {
        let (side, price) = self.order_loc.get(&order_id)?;
        match side {
            Side::Bid => self.bids.remove_order(price, order_id),
            Side::Ask => self.asks.remove_order(price, order_id),
        }
    }

    /// Remove an order from the order book, returning the order if it was successfully removed.
    pub fn cancel_order(&mut self, order_id: OrderId) -> Option<TradeOrder> {
        let (s, i) = self.order_loc.get(&order_id)?;
        let price = match s {
            Side::Bid => self.bids.price_levels.get_mut(i),
            Side::Ask => self.asks.price_levels.get_mut(i),
        }?;
        price
            .iter()
            .position(|o| o.id == order_id)
            .map(|i| price.remove(i))?
    }

    fn create_new_order(&mut self, order: &Order) -> Result<OrderId> {
        assert_ne!(order.order_type, OrderType::Market);
        let book = match order.side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };
        let order = TradeOrder::new(order.qty);
        let order_id = order.id;
        book.add_order(order_id, order);
        Ok(order_id)
    }

    pub fn best_bid_price(&self) -> Option<Price> {
        self.bids.best_price()
    }

    pub fn best_ask_price(&self) -> Option<Price> {
        self.asks.best_price()
    }

    pub fn add_order(&mut self, order: Order) -> Result<FillResult> {
        fn match_at_price_level(
            price_level: &mut VecDeque<TradeOrder>,
            incoming_order_qty: &mut Quantity,
            order_loc: &mut HashMap<OrderId, (Side, Price)>,
        ) -> Quantity {
            let mut matched_qty = 0;
            while let Some(mut order) = price_level.pop_front() {
                if order.qty > *incoming_order_qty {
                    order.qty -= *incoming_order_qty;
                    matched_qty += *incoming_order_qty;
                    price_level.push_front(order);
                    *incoming_order_qty = 0;
                    break;
                } else {
                    *incoming_order_qty -= order.qty;
                    matched_qty += order.qty;
                    order_loc.remove(&order.id);
                }
            }
            matched_qty
        }

        let mut remaining_order_qty = order.qty;
        let mut fill_result = FillResult::default();

        let book = match order.side {
            Side::Bid => &mut self.asks,
            Side::Ask => &mut self.bids,
        };

        for p in book.iter_prices().filter(|p| match order.order_type {
            OrderType::Limit(price) => match order.side {
                Side::Bid => price >= *p,
                Side::Ask => price <= *p,
            },
            // Market order no filtering required
            OrderType::Market => true,
        }) {
            if let Some(price_level) = book.price_levels.get_mut(&p) {
                let matched_qty = match_at_price_level(
                    price_level,
                    &mut remaining_order_qty,
                    &mut self.order_loc,
                );
                if matched_qty != 0 {
                    fill_result.filled_orders.push((matched_qty, p));
                }
                if remaining_order_qty == 0 {
                    break;
                }
            }
        }
        fill_result.remaining_qty = remaining_order_qty;
        fill_result.status = if remaining_order_qty == 0 {
            OrderStatus::Filled
        } else {
            match order.order_type {
                OrderType::Market => {
                    if remaining_order_qty == order.qty {
                        OrderStatus::Cancelled
                    } else {
                        OrderStatus::PartiallyFilledMarket
                    }
                }
                OrderType::Limit(_) => {
                    let id = self.create_new_order(&order)?;
                    if remaining_order_qty == order.qty {
                        OrderStatus::Open(id)
                    } else {
                        OrderStatus::PartiallyFilled(id)
                    }
                }
            }
        };
        Ok(fill_result)
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
