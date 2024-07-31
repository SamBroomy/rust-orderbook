use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{orderbook::TradeExecution, Side};

#[derive(Debug, Clone)]
pub enum Notification {
    OrderAdded {
        order_id: Uuid,
        price: u64,
        qty: u64,
        side: Side,
    },
    OrderRemoved {
        order_id: Uuid,
        price: u64,
        qty: u64,
        side: Side,
    },
    TradeExecuted(TradeExecution),
}

pub struct NotificationHandler {
    sender: Sender<(Uuid, Notification)>,
    receiver: Receiver<(Uuid, Notification)>,
    subscribers: HashMap<Uuid, Sender<Notification>>,
}

impl Default for NotificationHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationHandler {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();
        NotificationHandler {
            sender,
            receiver,
            subscribers: HashMap::new(),
        }
    }

    pub fn get_sender(&self) -> Sender<(Uuid, Notification)> {
        self.sender.clone()
    }

    pub fn subscribe(&mut self) -> (Uuid, Receiver<Notification>) {
        let id = Uuid::new_v4();
        let (sender, receiver) = unbounded();
        self.subscribers.insert(id, sender);
        (id, receiver)
    }

    pub fn unsubscribe(&mut self, id: Uuid) {
        self.subscribers.remove(&id);
    }

    pub fn run(&mut self) {
        while let Ok((subscriber_id, notification)) = self.receiver.recv() {
            if let Some(sender) = self.subscribers.get(&subscriber_id) {
                sender.send(notification).unwrap();
            }
        }
    }
}
