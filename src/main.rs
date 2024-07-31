// use orderbooklib::{OrderBook, OrderStatus, Side};
// use rand::Rng;
// fn main() {
//     println!("Creating new Orderbook");
//     let mut ob = OrderBook::new("BTC".to_string());
//     let mut rng = rand::thread_rng();
//     for _ in 1..1000 {
//         ob.add_limit_order(Side::Bid, rng.gen_range(1..5000), rng.gen_range(1..=500))
//             .unwrap();
//     }
//     //dbgp!("{:#?}", ob);
//     println!("Done adding orders, Starting to fill");

//     for _ in 1..10 {
//         for _ in 1..100 {
//             let fr = ob
//                 .add_limit_order(Side::Ask, rng.gen_range(1..5000), rng.gen_range(1..=500))
//                 .unwrap();
//             println!("{:#?}", fr);
//             // if matches! {fr.status, OrderStatus::Filled} {
//             //     println!("{:#?}, avg_fill_price {}", fr, fr.avr_fill_price());
//             // }
//         }
//     }
//     println!("Done!");
//     println!("{:#?}", ob);
//     ob.update_best_bid_and_ask();
//     println!("Best Bid Price: {}", ob.best_bid_price);
//     println!("Best Ask Price: {}", ob.best_ask_price);

//     ob.get_best_bid_price().unwrap();
// }

// ... other existing imports and modules ...

use orderbooklib::App;

fn main() -> std::io::Result<()> {
    let mut app = App::new();
    app.run()
}
// fn main() {
//     println!("Hello, world!");
//     let mut engine = orderbooklib::MatchingEngine::new();
//     let pair = orderbooklib::TradingPair::new("BTC".to_string(), "USD".to_string());
//     engine.add_new_market(pair);
// }
