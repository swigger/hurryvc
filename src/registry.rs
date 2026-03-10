use std::collections::{HashMap, HashSet};

use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::protocol::{
    ConsumerHelloPayload, ProducerCapabilities, ProducerHelloPayload, SessionSummary,
    TerminalInput, TerminalSnapshot, WireMessage,
};

pub type PeerTx = mpsc::UnboundedSender<WireMessage>;

#[derive(Default)]
pub struct Registry {
    producers: HashMap<String, ProducerRecord>,
    producer_sessions: HashMap<(String, String), String>,
    consumers: HashMap<String, ConsumerRecord>,
    consumer_sessions: HashMap<(String, String), String>,
}

pub struct RegisterProducerResult {
    pub producer_id: String,
    pub messages: Vec<(PeerTx, WireMessage)>,
}

pub struct RegisterConsumerResult {
    pub consumer_id: String,
    pub messages: Vec<(PeerTx, WireMessage)>,
}

struct ProducerRecord {
    producer_id: String,
    group_key: String,
    session_key: String,
    producer_name: String,
    command: Vec<String>,
    platform: String,
    pid: u32,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    capabilities: ProducerCapabilities,
    tx: PeerTx,
    close: Option<oneshot::Sender<()>>,
    snapshot: Option<TerminalSnapshot>,
    subscribers: HashSet<String>,
    streaming: bool,
}

struct ConsumerRecord {
    group_key: String,
    session_key: String,
    client_info: Option<String>,
    subscribed_to: Option<String>,
    tx: PeerTx,
    close: Option<oneshot::Sender<()>>,
}

impl Registry {
    pub fn register_producer(
        &mut self,
        hello: ProducerHelloPayload,
        tx: PeerTx,
        close: oneshot::Sender<()>,
    ) -> RegisterProducerResult {
        let mut messages = Vec::new();
        let session_key = (
            hello.production_group_key.clone(),
            hello.producer_session_key.clone(),
        );
        let producer_id = if let Some(existing_id) = self.producer_sessions.get(&session_key).cloned()
        {
            if let Some(existing) = self.producers.get_mut(&existing_id) {
                if let Some(old_close) = existing.close.take() {
                    let _ = old_close.send(());
                }
                messages.push((
                    existing.tx.clone(),
                    WireMessage::server_kick("replaced by a new producer connection"),
                ));
                existing.producer_name = hello.producer_name.clone();
                existing.command = hello.command.clone();
                existing.platform = hello.platform.clone();
                existing.pid = hello.pid;
                existing.cwd = hello.cwd.clone();
                existing.cols = hello.cols;
                existing.rows = hello.rows;
                existing.capabilities = hello.capabilities.clone();
                existing.tx = tx.clone();
                existing.close = Some(close);
                if !existing.subscribers.is_empty() {
                    existing.streaming = true;
                    messages.push((tx.clone(), WireMessage::start_data(existing_id.clone())));
                    if let Some(snapshot) = existing.snapshot.clone() {
                        messages.push((
                            tx.clone(),
                            WireMessage::term_snapshot(existing_id.clone(), snapshot),
                        ));
                    }
                }
            }
            existing_id
        } else {
            let producer_id = format!("prd-{}", Uuid::new_v4().simple());
            self.producer_sessions
                .insert(session_key, producer_id.clone());
            self.producers.insert(
                producer_id.clone(),
                ProducerRecord {
                    producer_id: producer_id.clone(),
                    group_key: hello.production_group_key.clone(),
                    session_key: hello.producer_session_key.clone(),
                    producer_name: hello.producer_name.clone(),
                    command: hello.command.clone(),
                    platform: hello.platform.clone(),
                    pid: hello.pid,
                    cwd: hello.cwd.clone(),
                    cols: hello.cols,
                    rows: hello.rows,
                    capabilities: hello.capabilities,
                    tx,
                    close: Some(close),
                    snapshot: None,
                    subscribers: HashSet::new(),
                    streaming: false,
                },
            );
            producer_id
        };
        messages.extend(self.session_list_messages(&hello.production_group_key));
        RegisterProducerResult {
            producer_id,
            messages,
        }
    }

    pub fn register_consumer(
        &mut self,
        hello: ConsumerHelloPayload,
        tx: PeerTx,
        close: oneshot::Sender<()>,
    ) -> RegisterConsumerResult {
        let mut messages = Vec::new();
        let session_key = (
            hello.production_group_key.clone(),
            hello.consumer_session_key.clone(),
        );
        let consumer_id = if let Some(existing_id) = self.consumer_sessions.get(&session_key).cloned()
        {
            if let Some(existing) = self.consumers.get_mut(&existing_id) {
                if let Some(old_close) = existing.close.take() {
                    let _ = old_close.send(());
                }
                messages.push((
                    existing.tx.clone(),
                    WireMessage::server_kick("replaced by a new consumer connection"),
                ));
                existing.client_info = hello.client_info.clone();
                existing.tx = tx.clone();
                existing.close = Some(close);
                let subscribed_to = existing.subscribed_to.clone();
                messages.push((tx.clone(), WireMessage::consumer_welcome(existing_id.clone())));
                messages.push((
                    tx.clone(),
                    WireMessage::session_list(self.sessions_for_group(&hello.production_group_key)),
                ));
                if let Some(producer_id) = subscribed_to {
                    if let Some(producer) = self.producers.get(&producer_id) {
                        if let Some(snapshot) = producer.snapshot.clone() {
                            messages.push((
                                tx.clone(),
                                WireMessage::term_snapshot(producer_id, snapshot),
                            ));
                        }
                    } else if let Some(existing) = self.consumers.get_mut(&existing_id) {
                        existing.subscribed_to = None;
                    }
                }
            }
            existing_id
        } else {
            let consumer_id = format!("csm-{}", Uuid::new_v4().simple());
            self.consumer_sessions
                .insert(session_key, consumer_id.clone());
            self.consumers.insert(
                consumer_id.clone(),
                ConsumerRecord {
                    group_key: hello.production_group_key.clone(),
                    session_key: hello.consumer_session_key.clone(),
                    client_info: hello.client_info.clone(),
                    subscribed_to: None,
                    tx: tx.clone(),
                    close: Some(close),
                },
            );
            messages.push((tx.clone(), WireMessage::consumer_welcome(consumer_id.clone())));
            messages.push((
                tx,
                WireMessage::session_list(self.sessions_for_group(&hello.production_group_key)),
            ));
            consumer_id
        };
        RegisterConsumerResult {
            consumer_id,
            messages,
        }
    }

    pub fn update_snapshot(
        &mut self,
        producer_id: &str,
        snapshot: TerminalSnapshot,
    ) -> Vec<(PeerTx, WireMessage)> {
        let mut recipients = Vec::new();
        if let Some(producer) = self.producers.get_mut(producer_id) {
            producer.cols = snapshot.cols;
            producer.rows = snapshot.rows;
            producer.snapshot = Some(snapshot.clone());
            for consumer_id in &producer.subscribers {
                if let Some(consumer) = self.consumers.get(consumer_id) {
                    recipients.push((
                        consumer.tx.clone(),
                        WireMessage::term_snapshot(producer_id.to_string(), snapshot.clone()),
                    ));
                }
            }
        }
        recipients
    }

    pub fn update_delta(
        &mut self,
        producer_id: &str,
        delta: crate::protocol::TerminalDelta,
    ) -> Vec<(PeerTx, WireMessage)> {
        let mut recipients = Vec::new();
        if let Some(producer) = self.producers.get_mut(producer_id) {
            producer.cols = delta.cols;
            producer.rows = delta.rows;
            match producer.snapshot.as_mut() {
                Some(snapshot) => snapshot.apply_delta(&delta),
                None => {
                    producer.snapshot = Some(TerminalSnapshot {
                        revision: delta.revision,
                        cols: delta.cols,
                        rows: delta.rows,
                        cursor_row: delta.cursor_row,
                        cursor_col: delta.cursor_col,
                        cursor_visible: delta.cursor_visible,
                        title: delta.title.clone(),
                        lines: vec![],
                        exit_status: delta.exit_status,
                    });
                    if let Some(snapshot) = producer.snapshot.as_mut() {
                        snapshot.apply_delta(&delta);
                    }
                }
            }
            for consumer_id in &producer.subscribers {
                if let Some(consumer) = self.consumers.get(consumer_id) {
                    recipients.push((
                        consumer.tx.clone(),
                        WireMessage::term_delta(producer_id.to_string(), delta.clone()),
                    ));
                }
            }
        }
        recipients
    }

    pub fn subscribe_consumer(
        &mut self,
        consumer_id: &str,
        producer_id: &str,
    ) -> Vec<(PeerTx, WireMessage)> {
        let mut messages = Vec::new();
        let Some(consumer_group) = self
            .consumers
            .get(consumer_id)
            .map(|consumer| consumer.group_key.clone())
        else {
            return messages;
        };
        let Some(producer_group) = self
            .producers
            .get(producer_id)
            .map(|producer| producer.group_key.clone())
        else {
            if let Some(consumer) = self.consumers.get(consumer_id) {
                messages.push((
                    consumer.tx.clone(),
                    WireMessage::consumer_error("session not found"),
                ));
            }
            return messages;
        };
        if consumer_group != producer_group {
            if let Some(consumer) = self.consumers.get(consumer_id) {
                messages.push((
                    consumer.tx.clone(),
                    WireMessage::consumer_error("session is not visible for this group"),
                ));
            }
            return messages;
        }
        let previous_producer = self
            .consumers
            .get(consumer_id)
            .and_then(|consumer| consumer.subscribed_to.clone());
        if let Some(previous_producer_id) = previous_producer {
            if previous_producer_id != producer_id {
                messages.extend(self.unsubscribe_consumer(consumer_id));
            }
        }
        if let Some(consumer) = self.consumers.get_mut(consumer_id) {
            consumer.subscribed_to = Some(producer_id.to_string());
        }
        if let Some(producer) = self.producers.get_mut(producer_id) {
            let was_empty = producer.subscribers.is_empty();
            producer.subscribers.insert(consumer_id.to_string());
            producer.streaming = true;
            if was_empty {
                messages.push((
                    producer.tx.clone(),
                    WireMessage::start_data(producer_id.to_string()),
                ));
            }
            if let Some(snapshot) = producer.snapshot.clone() {
                if let Some(consumer) = self.consumers.get(consumer_id) {
                    messages.push((
                        consumer.tx.clone(),
                        WireMessage::term_snapshot(producer_id.to_string(), snapshot),
                    ));
                }
            }
        }
        messages
    }

    pub fn unsubscribe_consumer(&mut self, consumer_id: &str) -> Vec<(PeerTx, WireMessage)> {
        let mut messages = Vec::new();
        let producer_id = self
            .consumers
            .get_mut(consumer_id)
            .and_then(|consumer| consumer.subscribed_to.take());
        if let Some(producer_id) = producer_id {
            if let Some(producer) = self.producers.get_mut(&producer_id) {
                producer.subscribers.remove(consumer_id);
                if producer.subscribers.is_empty() {
                    producer.streaming = false;
                    messages.push((
                        producer.tx.clone(),
                        WireMessage::stop_data(producer_id.clone()),
                    ));
                }
            }
        }
        messages
    }

    pub fn producer_input(
        &self,
        consumer_id: &str,
        producer_id: &str,
        input: TerminalInput,
    ) -> Vec<(PeerTx, WireMessage)> {
        let mut messages = Vec::new();
        let Some(consumer) = self.consumers.get(consumer_id) else {
            return messages;
        };
        let Some(producer) = self.producers.get(producer_id) else {
            messages.push((
                consumer.tx.clone(),
                WireMessage::consumer_error("session not found"),
            ));
            return messages;
        };
        if consumer.group_key != producer.group_key {
            messages.push((
                consumer.tx.clone(),
                WireMessage::consumer_error("session is not visible for this group"),
            ));
            return messages;
        }
        messages.push((
            producer.tx.clone(),
            WireMessage::input_data(producer_id.to_string(), input),
        ));
        messages
    }

    pub fn remove_consumer(&mut self, consumer_id: &str) -> Vec<(PeerTx, WireMessage)> {
        let messages = self.unsubscribe_consumer(consumer_id);
        if let Some(consumer) = self.consumers.remove(consumer_id) {
            self.consumer_sessions
                .remove(&(consumer.group_key, consumer.session_key));
        }
        messages
    }

    pub fn remove_producer(
        &mut self,
        producer_id: &str,
        snapshot: Option<TerminalSnapshot>,
        exit_status: Option<i32>,
        reason: impl Into<String>,
    ) -> Vec<(PeerTx, WireMessage)> {
        let reason = reason.into();
        let Some(producer) = self.producers.remove(producer_id) else {
            return Vec::new();
        };
        self.producer_sessions
            .remove(&(producer.group_key.clone(), producer.session_key.clone()));
        let final_snapshot = snapshot.or(producer.snapshot.clone());
        let mut messages = Vec::new();
        for consumer_id in &producer.subscribers {
            if let Some(consumer) = self.consumers.get_mut(consumer_id) {
                if consumer.subscribed_to.as_deref() == Some(producer_id) {
                    consumer.subscribed_to = None;
                    messages.push((
                        consumer.tx.clone(),
                        WireMessage::session_terminated(
                            producer_id.to_string(),
                            final_snapshot.clone(),
                            exit_status,
                            reason.clone(),
                        ),
                    ));
                }
            }
        }
        messages.extend(self.session_list_messages(&producer.group_key));
        messages
    }

    pub fn sessions_for_group(&self, group_key: &str) -> Vec<SessionSummary> {
        let mut sessions = self
            .producers
            .values()
            .filter(|producer| producer.group_key == group_key)
            .map(|producer| SessionSummary {
                producer_id: producer.producer_id.clone(),
                producer_name: producer.producer_name.clone(),
                command: producer.command.clone(),
                platform: producer.platform.clone(),
                cols: producer.cols,
                rows: producer.rows,
                cwd: producer.cwd.clone(),
                pid: producer.pid,
                streaming: producer.streaming,
            })
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| left.producer_name.cmp(&right.producer_name));
        sessions
    }

    fn session_list_messages(&self, group_key: &str) -> Vec<(PeerTx, WireMessage)> {
        let payload = WireMessage::session_list(self.sessions_for_group(group_key));
        self.consumers
            .values()
            .filter(|consumer| consumer.group_key == group_key)
            .map(|consumer| (consumer.tx.clone(), payload.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::{mpsc, oneshot};

    use super::Registry;
    use crate::protocol::{
        ConsumerHelloPayload, ProducerCapabilities, ProducerHelloPayload, WireMessage,
    };

    fn producer_hello() -> ProducerHelloPayload {
        ProducerHelloPayload {
            master_key: "master-demo".into(),
            production_group_key: "p-demo".into(),
            producer_session_key: "psk-demo".into(),
            producer_name: "codex".into(),
            command: vec!["cmd".into()],
            platform: "windows".into(),
            pid: 42,
            cols: 120,
            rows: 40,
            cwd: Some("D:/work".into()),
            capabilities: ProducerCapabilities {
                resize: true,
                signals: false,
            },
        }
    }

    fn consumer_hello() -> ConsumerHelloPayload {
        ConsumerHelloPayload {
            master_key: "master-demo".into(),
            production_group_key: "p-demo".into(),
            consumer_session_key: "csk-demo".into(),
            client_info: Some("browser".into()),
        }
    }

    #[test]
    fn subscribe_sends_start_data_only_on_first_consumer() {
        let mut registry = Registry::default();
        let (producer_tx, _) = mpsc::unbounded_channel();
        let (producer_close, _) = oneshot::channel();
        let producer = registry.register_producer(producer_hello(), producer_tx, producer_close);

        let (consumer_tx_1, _) = mpsc::unbounded_channel();
        let (consumer_close_1, _) = oneshot::channel();
        let consumer_1 = registry.register_consumer(consumer_hello(), consumer_tx_1, consumer_close_1);

        let messages = registry.subscribe_consumer(&consumer_1.consumer_id, &producer.producer_id);
        assert!(messages.iter().any(|(_, message)| matches!(
            message,
            WireMessage::StartData { .. }
        )));

        let (consumer_tx_2, _) = mpsc::unbounded_channel();
        let (consumer_close_2, _) = oneshot::channel();
        let consumer_2 = registry.register_consumer(
            ConsumerHelloPayload {
                consumer_session_key: "csk-demo-2".into(),
                ..consumer_hello()
            },
            consumer_tx_2,
            consumer_close_2,
        );
        let messages = registry.subscribe_consumer(&consumer_2.consumer_id, &producer.producer_id);
        assert!(!messages.iter().any(|(_, message)| matches!(
            message,
            WireMessage::StartData { .. }
        )));
    }

    #[test]
    fn producer_exit_removes_session_but_keeps_terminal_for_live_consumer() {
        let mut registry = Registry::default();
        let (producer_tx, _) = mpsc::unbounded_channel();
        let (producer_close, _) = oneshot::channel();
        let producer = registry.register_producer(producer_hello(), producer_tx, producer_close);
        let (consumer_tx, _) = mpsc::unbounded_channel();
        let (consumer_close, _) = oneshot::channel();
        let consumer = registry.register_consumer(consumer_hello(), consumer_tx, consumer_close);
        registry.subscribe_consumer(&consumer.consumer_id, &producer.producer_id);

        let messages = registry.remove_producer(&producer.producer_id, None, Some(0), "process exited");
        assert!(messages.iter().any(|(_, message)| matches!(
            message,
            WireMessage::SessionTerminated { .. }
        )));
        assert!(registry.sessions_for_group("p-demo").is_empty());
    }
}
