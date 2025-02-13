use orderbooklib::{HalfBook, OrderBook, OrderRequest, OrderType, Side, TradeOrder};

fn halfbook() {
    let mut ask_book = HalfBook::new(Side::Ask);
    let order = TradeOrder::new(200);
    ask_book.add_order(200, order);
    let order = TradeOrder::new(300);
    ask_book.add_order(200, order);
    let order = TradeOrder::new(300);
    ask_book.add_order(300, order);
    let order = TradeOrder::new(100);
    let id = order.id;
    ask_book.add_order(100, order);
    println!("{:#?}", ask_book);
    println!("{:?}", ask_book.best_price());

    println!("{:?}", ask_book.remove_order(&100.into(), id));
    println!("{:?}", ask_book.remove_order(&100.into(), id));
    println!("{:#?}", ask_book);
    println!("{:?}", ask_book.best_price());
    println!("{:?}", ask_book.get_price_level(&200.into()));
    println!("{:?}", ask_book.get_price_level(&300.into()));
    println!("{:?}", ask_book.get_total_qty(&200.into()));
    ask_book.show_depth()
}

fn main() {
    halfbook();

    println!("------------------------");

    let mut orderbook = OrderBook::default();

    let order = OrderRequest::new(Side::Ask, 100, OrderType::limit(200));
    println!("{:?}", orderbook.add_order(order));
    let order = OrderRequest::new(Side::Ask, 200, OrderType::limit(200));
    println!("{:?}", orderbook.add_order(order));
    let order = OrderRequest::new(Side::Ask, 300, OrderType::limit(200));
    let (fill_result, executions) = orderbook.add_order(order);
    let order = OrderRequest::new(Side::Ask, 400, OrderType::limit(250));
    println!("{:?}", orderbook.add_order(order));
    println!("{:#?}", fill_result);
    println!("{:#?}", executions);
    let id = fill_result.get_id();
    println!("{:?}", id);

    println!("{:#?}", orderbook);
    println!("{:?}", orderbook.best_ask());
    println!("{:?}", orderbook.best_bid());
    println!("{:?}", orderbook.best_prices());
    orderbook.show_depth();

    let order = OrderRequest::new(Side::Bid, 100, OrderType::limit(200));
    println!("{:#?}", orderbook.add_order(order));
    let order = OrderRequest::new(Side::Bid, 200, OrderType::Market);
    println!("{:#?}", orderbook.add_order(order));
    let order = OrderRequest::new(Side::Bid, 300, OrderType::limit(100));
    println!("{:#?}", orderbook.add_order(order));
    let order = OrderRequest::new(Side::Bid, 50, OrderType::limit(300));
    println!("{:#?}", orderbook.add_order(order));
    let order = OrderRequest::new(Side::Bid, 400, OrderType::limit(50));
    println!("{:#?}", orderbook.add_order(order));

    println!("{:#?}", orderbook);
    println!("{:?}", orderbook.best_ask());
    println!("{:?}", orderbook.best_bid());
    println!("{:?}", orderbook.best_prices());
    orderbook.show_depth();
    println!("{:?}", orderbook.delete_order(id));
    println!("{:?}", orderbook.best_ask());
    println!("{:?}", orderbook.best_bid());
    println!("{:?}", orderbook.best_prices());
    orderbook.show_depth();
    orderbook.best_price_liq();
    println!("{:#?}", orderbook);

    println!("---------------------------------");

    let mut orderbook = OrderBook::default();

    // Add some initial orders
    let order = OrderRequest::new(Side::Ask, 100, OrderType::limit(200));
    println!("Limit Ask: {:?}", orderbook.add_order(order));

    let order = OrderRequest::new(Side::Ask, 10, OrderType::limit(200));
    println!("Limit Ask: {:?}", orderbook.add_order(order));
    let order = OrderRequest::new(Side::Ask, 10, OrderType::limit(200));
    println!("Limit Ask: {:?}", orderbook.add_order(order));
    let order = OrderRequest::new(Side::Ask, 10, OrderType::limit(200));
    println!("Limit Ask: {:?}", orderbook.add_order(order));

    let order = OrderRequest::new(Side::Bid, 50, OrderType::limit(190));
    println!("Limit Bid: {:?}", orderbook.add_order(order));

    // Test IOC order
    let order = OrderRequest::new(Side::Bid, 75, OrderType::ioc(200));
    println!("IOC Bid: {:?}", orderbook.add_order(order));

    // Test FOK order that should execute
    let order = OrderRequest::new(Side::Bid, 50, OrderType::fok(200));
    println!(
        "FOK Bid (should execute): {:#?}",
        orderbook.add_order(order)
    );

    // Test FOK order that should cancel
    let order = OrderRequest::new(Side::Bid, 100, OrderType::fok(200));
    println!("FOK Bid (should cancel): {:?}", orderbook.add_order(order));

    println!("Final Order Book:");
    orderbook.show_depth();
}
