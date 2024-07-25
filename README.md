# Rust Orderbook

## Project Description

rust-orderbook is a another one of my projects to help further of multiple concepts, including rust and implementing and order book. This project aims to provide myself a good foundation for building trading systems, financial simulations, or cryptocurrency exchanges.

## Key Features

- Efficient limit order book implementation using a sparse vector data structure
- Support for both market and limit orders
- Fast order matching algorithm
- Separate bid and ask books for optimized performance
- Quick order lookup and cancellation
- Depth view of the order book
- Best price and liquidity information

## Project Structure

The project is organized into several modules:

- `orderbook`: Contains the core order book implementation
  - `price_levels.rs`: Implements the `SparseVec` data structure for efficient price level management
  - `order_book.rs`: Implements the main `OrderBook` and `HalfBook` structures
- `engine.rs`: Implements the `MatchingEngine` for managing multiple order books
- `errors.rs`: Defines custom error types for the project
- `main.rs`: Entry point for the binary crate
- `lib.rs`: Exposes the library interface

## TODO List

- [x] Implement OrderBook
- [ ] Implement a way to visually display the order book, showing level-by-level information and total order amounts at each level
- [ ] Add concurrency support for parallel order processing
- [ ] Implement persistence for order book state
- [ ] Develop an event system to emit notifications for significant events (e.g., trades, order book updates)
- [ ] Expand supported order types (e.g., IOC, FOK, stop orders)
- [ ] Implement time-based orders with a mechanism to expire old orders
- [ ] Create a market data feed
- [ ] Improve visualization of the order book
- [ ] Expand the test suite with more unit tests and property-based tests using libraries like proptest
- [ ] Add a configuration system to allow easy adjustment of parameters (e.g., max order size, tick size)
- [ ] Profile the code and optimize critical paths, possibly using SIMD instructions for matching logic

## License

This project is licensed under the MIT License - see the LICENSE file for details.
