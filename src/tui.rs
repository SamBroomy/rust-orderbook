use std::io;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame, Terminal,
};
use rust_decimal::Decimal;

use crate::{OrderBook, OrderRequest, OrderResult, OrderType, Side, TradeExecution};

#[derive(Debug, PartialEq)]
enum InputMode {
    Normal,
    Price,
    Quantity,
}

#[derive(Debug)]
struct OrderHistoryEntry {
    time: String,
    side: Side,
    order_type: OrderType,
    price: u64,
    quantity: u64,
}

#[derive(Debug)]
pub struct App {
    order_book: OrderBook,
    current_side: Side,
    current_order_type: OrderType,
    input_price: String,
    input_quantity: String,
    order_history: Vec<OrderHistoryEntry>,
    status_message: String,
    input_mode: InputMode,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
impl App {
    pub fn new() -> Self {
        Self {
            order_book: OrderBook::default(),
            current_side: Side::Bid,
            current_order_type: OrderType::limit(0),
            input_price: String::new(),
            input_quantity: String::new(),
            order_history: Vec::new(),
            status_message: String::from("Welcome to the OrderBook TUI!"),
            input_mode: InputMode::Normal,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = self.run_app(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            println!("{:?}", err)
        }

        Ok(())
    }

    fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                match self.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('s') => self.toggle_side(),
                        KeyCode::Char('t') => self.cycle_order_type(),
                        KeyCode::Char('p') => self.input_mode = InputMode::Price,
                        KeyCode::Char('a') => self.input_mode = InputMode::Quantity,
                        KeyCode::Enter => self.place_order(),
                        _ => {}
                    },
                    InputMode::Price => self.handle_input(&key),
                    InputMode::Quantity => self.handle_input(&key),
                }
            }
        }
    }

    fn ui(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
            .split(f.area());

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(10),
                    Constraint::Min(5),
                    Constraint::Length(1),
                ]
                .as_ref(),
            )
            .split(chunks[0]);

        self.render_order_placement(f, left_chunks[0]);
        self.render_order_history(f, left_chunks[1]);
        self.render_status_bar(f, left_chunks[2]);

        self.render_order_book(f, chunks[1]);
    }

    fn render_order_placement(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area);

        let side = Paragraph::new(Span::raw(format!("Side: {:?}", self.current_side)))
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(side, chunks[0]);

        let order_type = Paragraph::new(Span::raw(format!("Type: {:}", self.current_order_type)))
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(order_type, chunks[1]);

        let quantity = Paragraph::new(Span::raw(format!("Quantity: {}", self.input_quantity)))
            .style(
                Style::default().fg(if self.input_mode == InputMode::Quantity {
                    Color::Green
                } else {
                    Color::White
                }),
            );
        f.render_widget(quantity, chunks[2]);

        match self.current_order_type {
            OrderType::Market => {
                let market = Paragraph::new(Span::raw("Market Order"))
                    .style(Style::default().fg(Color::Green));
                f.render_widget(market, chunks[3]);
            }
            OrderType::FOK(_)
            | OrderType::IOC(_)
            | OrderType::Limit(_)
            | OrderType::SystemLevel(_) => {
                let price = Paragraph::new(Span::raw(format!("Price: {}", self.input_price)))
                    .style(Style::default().fg(if self.input_mode == InputMode::Price {
                        Color::Green
                    } else {
                        Color::White
                    }));
                f.render_widget(price, chunks[3]);
            }
        }

        let place_order = Paragraph::new(Span::raw("Press Enter to Place Order"))
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(place_order, chunks[4]);
    }

    fn render_order_history(&self, f: &mut Frame, area: Rect) {
        let header_cells = ["Time", "Side", "Type", "Price", "Qty"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        let rows = self.order_history.iter().map(|entry| {
            let cells = vec![
                Cell::from(entry.time.clone()),
                Cell::from(format!("{:?}", entry.side)),
                Cell::from(format!("{:?}", entry.order_type)),
                Cell::from(entry.price.to_string()),
                Cell::from(entry.quantity.to_string()),
            ];
            Row::new(cells)
        });

        let table = Table::new(
            rows,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Order History"),
        )
        .widths([
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ]);

        f.render_widget(table, area);
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let status =
            Paragraph::new(Span::raw(&self.status_message)).style(Style::default().fg(Color::Cyan));
        f.render_widget(status, area);
    }

    fn render_order_book(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Percentage(45),
                Constraint::Percentage(10),
            ])
            .margin(1)
            .split(area);

        let bids = self.create_table("Bids", Side::Bid);
        f.render_widget(bids, chunks[1]);

        let asks = self.create_table("Asks", Side::Ask);
        f.render_widget(asks, chunks[0]);

        let spread = self
            .order_book
            .best_ask()
            .unwrap_or(Decimal::ZERO)
            .saturating_sub(self.order_book.best_bid().unwrap_or(Decimal::ZERO));
        let volume = self
            .order_book
            .asks
            .iter_prices()
            .chain(self.order_book.bids.iter_prices())
            .map(|p| {
                self.order_book
                    .asks
                    .get_total_qty(&p)
                    .unwrap_or(Decimal::ZERO)
                    + self
                        .order_book
                        .bids
                        .get_total_qty(&p)
                        .unwrap_or(Decimal::ZERO)
            })
            .sum::<Decimal>();

        let summary = Paragraph::new(Line::from(vec![
            Span::raw("Spread: "),
            Span::styled(spread.to_string(), Style::default().fg(Color::Yellow)),
            Span::raw(" | Volume: "),
            Span::styled(volume.to_string(), Style::default().fg(Color::Yellow)),
        ]));
        f.render_widget(summary, chunks[2]);
    }

    fn create_table<'a>(&self, title: &'a str, side: Side) -> Table<'a> {
        let headers = ["Price", "Quantity"];
        let mut rows = Vec::new();

        let book = match side {
            Side::Bid => &self.order_book.bids,
            Side::Ask => &self.order_book.asks,
        };

        for price in book.iter_prices() {
            if let Some(qty) = book.get_total_qty(&price) {
                rows.push(Row::new(vec![price.to_string(), qty.to_string()]));
            }
        }

        Table::new(
            rows,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .header(Row::new(headers).style(Style::default().fg(Color::Yellow)))
        .block(Block::default().title(title).borders(Borders::ALL))
        .widths([Constraint::Percentage(50), Constraint::Percentage(50)])
        .column_spacing(1)
    }

    fn toggle_side(&mut self) {
        self.current_side = match self.current_side {
            Side::Bid => Side::Ask,
            Side::Ask => Side::Bid,
        };
    }

    fn cycle_order_type(&mut self) {
        self.current_order_type = match self.current_order_type {
            OrderType::Limit(_) => OrderType::Market,
            OrderType::Market => OrderType::IOC(self.input_price.parse().unwrap_or(Decimal::ZERO)),
            OrderType::IOC(_) => OrderType::FOK(self.input_price.parse().unwrap_or(Decimal::ZERO)),
            OrderType::FOK(_) => {
                OrderType::Limit(self.input_price.parse().unwrap_or(Decimal::ZERO))
            }
            _ => OrderType::Limit(self.input_price.parse().unwrap_or(Decimal::ZERO)),
        };
    }

    fn handle_input(&mut self, key: &event::KeyEvent) {
        match key.code {
            KeyCode::Enter => self.input_mode = InputMode::Normal,
            KeyCode::Char(c) => {
                if c.is_ascii_digit() {
                    match self.input_mode {
                        InputMode::Price => self.input_price.push(c),
                        InputMode::Quantity => {
                            self.input_quantity.push(c);
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Backspace => match self.input_mode {
                InputMode::Price => {
                    self.input_price.pop();
                }
                InputMode::Quantity => {
                    self.input_quantity.pop();
                }
                _ => {}
            },
            KeyCode::Esc => self.input_mode = InputMode::Normal,
            _ => {}
        }
    }

    fn place_order(&mut self) {
        let price = self.input_price.parse().unwrap_or(0);
        let quantity = self.input_quantity.parse().unwrap_or(0);

        if quantity == 0 {
            self.status_message = String::from("Invalid quantity. Order not placed.");
            return;
        }

        self.current_order_type = match self.current_order_type {
            OrderType::Limit(_) => OrderType::limit(price),
            OrderType::Market => OrderType::Market,
            OrderType::IOC(_) => OrderType::ioc(price),
            OrderType::FOK(_) => OrderType::fok(price),
            OrderType::SystemLevel(_) => OrderType::system_level(price),
        };
        let order_type = self.current_order_type;

        let order = OrderRequest::new(self.current_side, quantity, order_type);
        let (result, executions) = self.order_book.add_order(order);

        self.update_order_history(price, quantity);
        self.update_status(result, executions);

        // Clear inputs
        self.input_price.clear();
        self.input_quantity.clear();
    }

    fn update_order_history(&mut self, price: u64, quantity: u64) {
        let entry = OrderHistoryEntry {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            side: self.current_side,
            order_type: self.current_order_type,
            price,
            quantity,
        };
        self.order_history.push(entry);
        if self.order_history.len() > 5 {
            self.order_history.remove(0);
        }
    }

    fn update_status(&mut self, result: OrderResult, executions: Vec<TradeExecution>) {
        if executions.is_empty() {
            self.status_message = format!("Order placed: {:?}", result.status);
        } else {
            let total_executed = executions.iter().map(|e| e.qty).sum::<Decimal>();
            self.status_message = format!("Order executed: {} units filled", total_executed);
        }
    }
}

// impl App {
//     pub fn new() -> Self {
//         App {
//             order_book: OrderBook::new(),
//             current_side: Side::Ask,
//             current_order_type: OrderType::Limit(0),
//             input_price: String::new(),
//             input_quantity: String::new(),
//             order_history: Vec::new(),
//             status_message: String::new(),
//         }
//     }

//     pub fn run(&mut self) -> io::Result<()> {
//         // Set up terminal
//         enable_raw_mode()?;
//         let mut stdout = io::stdout();
//         execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
//         let backend = CrosstermBackend::new(stdout);
//         let mut terminal = Terminal::new(backend)?;

//         // Main loop
//         loop {
//             terminal.draw(|f| self.ui(f))?;

//             if let Event::Key(key) = event::read()? {
//                 match key.code {
//                     KeyCode::Char('q') => break,
//                     KeyCode::Char('b') => {
//                         self.order_book.add_order(OrderRequest::new(
//                             Side::Bid,
//                             100,
//                             OrderType::Limit(100),
//                         ));
//                     }
//                     KeyCode::Char('s') => {
//                         self.order_book.add_order(OrderRequest::new(
//                             Side::Ask,
//                             100,
//                             OrderType::Limit(110),
//                         ));
//                     }
//                     _ => {}
//                 }
//             }
//         }

//         // Restore terminal
//         disable_raw_mode()?;
//         execute!(
//             terminal.backend_mut(),
//             LeaveAlternateScreen,
//             DisableMouseCapture
//         )?;
//         terminal.show_cursor()?;

//         Ok(())
//     }

//     fn ui(&self, f: &mut ratatui::Frame) {
//         let chunks = Layout::default()
//             .direction(Direction::Vertical)
//             .margin(1)
//             .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
//             .split(f.size());

//         let bids = self.create_table("Bids", Side::Bid);
//         f.render_widget(bids, chunks[0]);

//         let asks = self.create_table("Asks", Side::Ask);
//         f.render_widget(asks, chunks[1]);
//     }

//     fn create_table<'a>(&self, title: &'a str, side: Side) -> Table<'a> {
//         let headers = ["Price", "Quantity"];
//         let mut rows = Vec::new();

//         let book = match side {
//             Side::Bid => &self.order_book.bids,
//             Side::Ask => &self.order_book.asks,
//         };

//         for price in book.iter_prices() {
//             if let Some(qty) = book.get_total_qty(&price) {
//                 rows.push(Row::new(vec![price.to_string(), qty.to_string()]));
//             }
//         }

//         if let Side::Ask = side {
//             rows.reverse()
//         };

//         Table::new(rows, vec![Constraint::Percentage(100)])
//             .header(Row::new(headers).style(Style::default().fg(Color::Yellow)))
//             .block(Block::default().title(title).borders(Borders::ALL))
//             .widths([Constraint::Percentage(50), Constraint::Percentage(50)])
//             .column_spacing(1)
//     }
// }
