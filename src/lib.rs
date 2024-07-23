type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

use rand::Rng;
use std::collections::{BTreeMap, HashMap, VecDeque};

#[derive(Debug)]
pub enum Side {
    Bid,
    Ask,
}

pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Default)]
pub enum OrderStatus {
    #[default]
    Uninitialized,
    Open,
    Filled,
    PartiallyFilled,
    //Canceled,
}

#[derive(Debug)]
pub struct FillResult {
    // Orders filled (qty, price)
    filled_orders: Vec<(u64, u64)>,
    remaining_qty: u64,
    pub status: OrderStatus,
}

impl Default for FillResult {
    fn default() -> Self {
        FillResult {
            filled_orders: Vec::new(),
            remaining_qty: u64::MAX,
            status: OrderStatus::default(), // This requires OrderStatus to implement Default
        }
    }
}

impl FillResult {
    pub fn avr_fill_price(&self) -> f32 {
        let mut total = 0;
        let mut qty = 0;
        for (q, p) in &self.filled_orders {
            total += q * p;
            qty += q;
        }
        total as f32 / qty as f32
    }
}

#[derive(Debug)]
struct Order {
    id: u64,
    qty: u64,
}

#[derive(Debug)]
struct PriceLevel {
    orders: VecDeque<Order>,
}

#[derive(Debug)]
struct HalfBook {
    s: Side,
    // Price & Index of price Level
    price_map: BTreeMap<u64, usize>,
    price_levels: Vec<PriceLevel>,
}

impl HalfBook {
    pub fn new(s: Side) -> HalfBook {
        HalfBook {
            s,
            price_map: BTreeMap::new(),
            price_levels: Vec::with_capacity(10_000),
        }
    }

    pub fn get_total_qty(&self, price: u64) -> Result<u64> {
        Ok(self.price_map.get(&price).map_or_else(
            || Err("Price not found"),
            |i| {
                self.price_levels.get(*i).map_or_else(
                    || Err("Price level not found"),
                    |pl| Ok(pl.orders.iter().fold(0, |acc, o| acc + o.qty)),
                )
            },
        )?)
    }
}

#[derive(Debug)]
pub struct OrderBook {
    symbol: String,
    pub best_bid_price: u64,
    pub best_ask_price: u64,
    bids: HalfBook,
    asks: HalfBook,
    // For fast order lookup / cancel OrderId -> (Side, PriceLevelIndex)
    order_loc: HashMap<u64, (Side, usize)>,
}

impl OrderBook {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            best_bid_price: u64::MIN,
            best_ask_price: u64::MAX,
            bids: HalfBook::new(Side::Bid),
            asks: HalfBook::new(Side::Ask),
            order_loc: HashMap::with_capacity(50_000),
        }
    }

    pub fn cancel_order(&mut self, order_id: u64) -> Result<()> {
        let (s, i) = self.order_loc.get(&order_id).ok_or("Order not found")?;
        let pl = match s {
            Side::Bid => &mut self
                .bids
                .price_levels
                .get_mut(*i)
                .ok_or("Price level not found")?,
            Side::Ask => &mut self
                .asks
                .price_levels
                .get_mut(*i)
                .ok_or("Price level not found")?,
        };
        pl.orders.retain(|o| o.id != order_id);
        self.order_loc.remove(&order_id);
        Ok(())
    }

    fn create_new_limit_order(&mut self, side: Side, price: u64, qty: u64) -> Result<u64> {
        let mut rng = rand::thread_rng();
        let order_id = rng.gen::<u64>();
        let book = match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };
        let order = Order { id: order_id, qty };

        match book.price_map.get(&price) {
            Some(i) => {
                book.price_levels
                    .get_mut(*i)
                    .ok_or("Price level not found")?
                    .orders
                    .push_back(order);
                self.order_loc.insert(order_id, (side, *i));
            }
            None => {
                let i = book.price_levels.len();
                book.price_map.insert(price, i);
                book.price_levels.push(PriceLevel {
                    orders: VecDeque::from(vec![order]),
                });
                self.order_loc.insert(order_id, (side, i));
            }
        }
        Ok(order_id)
    }

    pub fn update_best_bid_and_ask(&mut self) {
        self.best_bid_price = self
            .bids
            .price_map
            .iter()
            .next_back()
            .map_or(u64::MIN, |(p, _)| *p);
        self.best_ask_price = self
            .asks
            .price_map
            .iter()
            .next()
            .map_or(u64::MAX, |(p, _)| *p);
    }

    // #[cfg(test)]
    // fn test_match_at_price_level() {
    //     let mut price_level = VecDeque::from(vec![Order { id: 1, qty: 10 }]);
    //     let mut incoming_order_qty = 5;
    //     let mut order_loc = HashMap::new();
    //     let total_qty = OrderBook::match_at_price_level(
    //         &mut price_level,
    //         &mut incoming_order_qty,
    //         &mut order_loc,
    //     );
    //     assert_eq!(total_qty, 5);
    //     assert_eq!(price_level.len(), 1);
    //     assert_eq!(price_level[0].qty, 5);
    //     assert_eq!(incoming_order_qty, 0);
    //     assert_eq!(order_loc.len(), 0);
    // }

    pub fn add_limit_order(&mut self, side: Side, price: u64, qty: u64) -> Result<FillResult> {
        fn match_at_price_level(
            price_level: &mut VecDeque<Order>,
            incoming_order_qty: &mut u64,
            order_loc: &mut HashMap<u64, (Side, usize)>,
        ) -> u64 {
            let mut total_qty = 0;
            while let Some(mut order) = price_level.pop_front() {
                if order.qty > *incoming_order_qty {
                    order.qty -= *incoming_order_qty;
                    total_qty += *incoming_order_qty;
                    price_level.push_front(order);
                    *incoming_order_qty = 0;
                    break;
                } else {
                    *incoming_order_qty -= order.qty;
                    total_qty += order.qty;
                    order_loc.remove(&order.id);
                }
            }
            total_qty
        }
        let mut remaining_order_qty = qty;
        let mut fill_result = FillResult::default();

        match side {
            Side::Bid => {
                let ask_book = &mut self.asks;
                let price_levels = &mut ask_book.price_levels;

                for (&order_price, &level_index) in
                    ask_book.price_map.iter().filter(|&(&p, _)| price >= p)
                {
                    let matched_qty = match_at_price_level(
                        &mut price_levels[level_index].orders,
                        &mut remaining_order_qty,
                        &mut self.order_loc,
                    );
                    if matched_qty != 0 {
                        fill_result.filled_orders.push((matched_qty, order_price));
                    }
                    if remaining_order_qty == 0 {
                        break;
                    }
                }
            }
            Side::Ask => {
                let bid_book = &mut self.bids;
                let price_levels = &mut bid_book.price_levels;

                for (&order_price, &level_index) in
                    bid_book.price_map.iter().filter(|&(&p, _)| price <= p)
                {
                    let matched_qty = match_at_price_level(
                        &mut price_levels[level_index].orders,
                        &mut remaining_order_qty,
                        &mut self.order_loc,
                    );
                    if matched_qty != 0 {
                        fill_result.filled_orders.push((matched_qty, order_price));
                    }
                    if remaining_order_qty == 0 {
                        break;
                    }
                }
            }
        }
        fill_result.remaining_qty = remaining_order_qty;
        fill_result.status = if remaining_order_qty != 0 {
            let _id = self.create_new_limit_order(side, price, qty)?;
            if remaining_order_qty == qty {
                OrderStatus::Open
            } else {
                OrderStatus::PartiallyFilled
            }
        } else {
            OrderStatus::Filled
        };
        self.update_best_bid_and_ask();
        Ok(fill_result)
    }

    pub fn get_best_bid_price(&self) -> Result<()> {
        let total_bid_qty = self.bids.get_total_qty(self.best_bid_price)?;
        let total_ask_qty = self.asks.get_total_qty(self.best_ask_price)?;

        println!(
            "Best Bid Price: {} Qty: {}",
            self.best_bid_price, total_bid_qty
        );
        println!(
            "Best Ask Price: {} Qty: {}",
            self.best_ask_price, total_ask_qty
        );
        println!(
            "Spread: {}",
            ((self.best_ask_price - self.best_bid_price) as f64 / self.best_ask_price as f64)
                as f32
        );
        Ok(())
    }
}
