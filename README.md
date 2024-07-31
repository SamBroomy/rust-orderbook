
# Rust Orderbook

## Project Description

rust-orderbook is a another one of my projects to help further of multiple concepts, including rust and implementing and order book. This project aims to provide myself a good foundation for building trading systems, financial simulations, or cryptocurrency exchanges.

## Key Features

- Efficient limit order book implementation using a sparse vector data structure
- Support for multiple order types: Market, Limit, IOC (Immediate or Cancel), and FOK (Fill or Kill)
- Fast order matching algorithm with price-time priority
- Separate bid and ask books for optimized performance
- Quick order lookup and cancellation
- Depth view of the order book
- Best price and liquidity information
- Terminal User Interface (TUI) for interactive order placement and book visualization (limited functionality)
- Matching engine supporting multiple trading pairs

## Project Structure

The project is organized into several modules:

- `orderbook`: Contains the core order book implementation
  - `price_levels.rs`: Implements the `SparseVec` data structure for efficient price level management
  - `orders.rs`: Defines order types, requests, and results
  - `book.rs`: Implements the main `OrderBook` and `HalfBook` structures
- `engine.rs`: Implements the `MatchingEngine` for managing multiple order books
- `errors.rs`: Defines custom error types for the project
- `notifications.rs`: Implements a notification system for order book events (in progress)
- `tui.rs`: Provides a Terminal User Interface for interacting with the order book
- `main.rs`: Entry point for the binary crate
- `lib.rs`: Exposes the library interface

## Getting Started

To run the project:

1. Clone the repository
2. Navigate to the project directory
3. Run `cargo run` to start the TUI application

For development:

- Use `just quick_dev` to run the quick development example with auto-reloading

## TODO List

- [x] Implement OrderBook
- [x] Implement a TUI to visually display and interact with the order book
- [x] Tests for the order book functionality
- [x] Add support for IOC and FOK order types
- [x] Implement a matching engine for multiple trading pairs
- [ ] Add concurrency support for parallel order processing
- [ ] Implement persistence for order book state
- [ ] Expand the event system to emit notifications for significant events
- [ ] Implement time-based orders with a mechanism to expire old orders
- [ ] Create a market data feed
- [ ] Add a configuration system to allow easy adjustment of parameters
- [ ] Profile the code and optimize critical paths

## License

This project is licensed under the MIT License - see the LICENSE file for details.
