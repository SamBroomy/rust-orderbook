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

// use orderbooklib::App;

// fn main() -> std::io::Result<()> {
//     let mut app = App::new();
//     app.run()
// }

use orderbooklib::{OrderBook, OrderRequest, OrderStatus, OrderType, Side};
use polars::{frame::row::Row, prelude::*};
use std::time::Instant;
use tracing::debug;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new order book
    let mut order_book = OrderBook::default();

    let schema = Schema::from_iter(vec![
        Field::new("Time", DataType::Float64),
        Field::new("Event Type", DataType::Int32),
        Field::new("Order Id", DataType::Int64),
        Field::new("Size", DataType::Int64),
        Field::new("Price", DataType::Int64),
        Field::new("Direction", DataType::Int32),
    ]);

    // Read the LOBSTER data
    let df = LazyCsvReader::new(
        "data/LOBSTER_SampleFile_MSFT_2012-06-21_10/MSFT_2012-06-21_34200000_57600000_message_10.csv",
        //"data/LOBSTER_SampleFile_AMZN_2012-06-21_1/AMZN_2012-06-21_34200000_57600000_message_1.csv",
    )
    .with_schema(Some(schema.into()))
    .with_has_header(false)
    .finish()?
    // Map direction to Side enum
    .collect()?;

    println!("Starting simulation");
    println!("{}", df);
    // println!("{}", df.collect()?);

    fn extract_data_out_of_row(row: Row) -> (i32, String, Side, u64, OrderType) {
        let row = row.0;
        let event_type = match row[1] {
            AnyValue::Int32(event_type) => event_type,
            _ => panic!("Invalid event type"),
        };
        let id = match row[2] {
            AnyValue::Int64(id) => id.to_string(),
            _ => panic!("Invalid id"),
        };
        let side = match row[5] {
            AnyValue::Int32(direction) => match direction {
                -1 => Side::Ask,
                1 => Side::Bid,
                _ => panic!("Invalid direction"),
            },
            _ => panic!("Invalid direction"),
        };
        let qty = match row[3] {
            AnyValue::Int64(qty) => qty.try_into().unwrap(),
            _ => panic!("Invalid quantity"),
        };
        let order_type = OrderType::Limit(match row[4] {
            AnyValue::Int64(price) => price.try_into().unwrap(),
            _ => panic!("Invalid price"),
        });

        (event_type, id, side, qty, order_type)
    }

    // fn create_orders_from_df(df: &DataFrame) -> Vec<OrderRequest> {
    //     let mut orders = Vec::new();
    //     for i in 0..df.height() {
    //         if let Ok(row) = df.get_row(i) {
    //             let (event_type, id, side, qty, order_type) = extract_data_out_of_row(row);
    //             let order = OrderRequest::new_with_other_id(id, side, qty, order_type);
    //             orders.push(order);
    //         }
    //     }
    //     orders
    // }

    // let orders = create_orders_from_df(&df);

    println!("{}", df.height());
    let start_time = Instant::now();
    for i in 0..df.height() {
        if let Ok(row) = df.get_row(i) {
            let (event_type, id, side, qty, order_type) = extract_data_out_of_row(row);

            match event_type {
                1 | 4 | 5 => {
                    let order = OrderRequest::new_with_other_id(id, side, qty, order_type);
                    let (fill_result, trade_ex) = order_book.add_order(order);
                    if fill_result.status != OrderStatus::Open {
                        debug!("{:#?}", fill_result);
                        debug!("-----------------------------");
                        debug!("{:#?}", trade_ex);
                        debug!("#############################");
                    }
                }
                2 => {
                    let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, id.as_ref());
                    order_book.cancel_order(id, qty);
                }
                3 => {
                    let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, id.as_ref());
                    order_book.delete_order(id);
                }
                _ => continue,
            }

            // if i == 100 {
            //     break;
            // }
        }
    }
    println!("Elapsed time: {:?}", start_time.elapsed());
    println!("{:#?}", order_book.get_order_book_state());
    println!("Best ask: {:?}", order_book.best_ask());
    println!("Best bid: {:?}", order_book.best_bid());
    println!("Best prices: {:?}", order_book.best_prices());
    println!("Depth: {:?}", order_book.get_depth());
    println!("Volume: {:?}", order_book.get_total_volume());
    println!("Spread: {:?}", order_book.spread());

    Ok(())
}
