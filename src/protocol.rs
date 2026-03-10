use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireMessage {
    ProducerHello {
        version: u8,
        payload: ProducerHelloPayload,
    },
    ProducerWelcome {
        version: u8,
        payload: ProducerWelcomePayload,
    },
    ProducerPing {
        version: u8,
    },
    ProducerExit {
        version: u8,
        payload: ProducerExitPayload,
    },
    ConsumerHello {
        version: u8,
        payload: ConsumerHelloPayload,
    },
    ConsumerWelcome {
        version: u8,
        payload: ConsumerWelcomePayload,
    },
    ConsumerPing {
        version: u8,
    },
    SubscribeSession {
        version: u8,
        payload: SessionRefPayload,
    },
    UnsubscribeSession {
        version: u8,
        payload: SessionRefPayload,
    },
    ConsumerInput {
        version: u8,
        payload: ConsumerInputPayload,
    },
    StartData {
        version: u8,
        payload: SessionRefPayload,
    },
    StopData {
        version: u8,
        payload: SessionRefPayload,
    },
    InputData {
        version: u8,
        payload: InputDataPayload,
    },
    Resize {
        version: u8,
        payload: ResizePayload,
    },
    TermSnapshot {
        version: u8,
        payload: TermSnapshotPayload,
    },
    TermDelta {
        version: u8,
        payload: TermDeltaPayload,
    },
    SessionList {
        version: u8,
        payload: SessionListPayload,
    },
    SessionTerminated {
        version: u8,
        payload: SessionTerminatedPayload,
    },
    ConsumerError {
        version: u8,
        payload: ErrorPayload,
    },
    ServerKick {
        version: u8,
        payload: ErrorPayload,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProducerHelloPayload {
    pub master_key: String,
    pub production_group_key: String,
    pub producer_session_key: String,
    pub producer_name: String,
    pub command: Vec<String>,
    pub platform: String,
    pub pid: u32,
    pub cols: u16,
    pub rows: u16,
    pub cwd: Option<String>,
    pub capabilities: ProducerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProducerWelcomePayload {
    pub producer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProducerExitPayload {
    pub producer_id: String,
    pub exit_status: Option<i32>,
    pub snapshot: Option<TerminalSnapshot>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProducerCapabilities {
    pub resize: bool,
    pub signals: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumerHelloPayload {
    pub master_key: String,
    pub production_group_key: String,
    pub consumer_session_key: String,
    pub client_info: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumerWelcomePayload {
    pub consumer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRefPayload {
    pub producer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumerInputPayload {
    pub producer_id: String,
    pub input: TerminalInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputDataPayload {
    pub producer_id: String,
    pub input: TerminalInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminalInput {
    Text { data: String },
    Key { key: InputKey },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    CtrlC,
    CtrlD,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResizePayload {
    pub producer_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TermSnapshotPayload {
    pub producer_id: String,
    pub snapshot: TerminalSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TermDeltaPayload {
    pub producer_id: String,
    pub delta: TerminalDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionListPayload {
    pub sessions: Vec<SessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSummary {
    pub producer_id: String,
    pub producer_name: String,
    pub command: Vec<String>,
    pub platform: String,
    pub cols: u16,
    pub rows: u16,
    pub cwd: Option<String>,
    pub pid: u32,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionTerminatedPayload {
    pub producer_id: String,
    pub snapshot: Option<TerminalSnapshot>,
    pub exit_status: Option<i32>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorPayload {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalSnapshot {
    pub revision: u64,
    pub cols: u16,
    pub rows: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub title: Option<String>,
    pub lines: Vec<TerminalLine>,
    pub exit_status: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalDelta {
    pub revision: u64,
    pub cols: u16,
    pub rows: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub title: Option<String>,
    pub lines: Vec<TerminalLine>,
    pub exit_status: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalLine {
    pub index: u16,
    pub runs: Vec<TerminalRun>,
    pub wrapped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalRun {
    pub text: String,
    pub fg: Option<TerminalColor>,
    pub bg: Option<TerminalColor>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminalColor {
    Indexed { value: u8 },
    Rgb { r: u8, g: u8, b: u8 },
}

impl WireMessage {
    pub fn producer_welcome(producer_id: impl Into<String>) -> Self {
        Self::ProducerWelcome {
            version: PROTOCOL_VERSION,
            payload: ProducerWelcomePayload {
                producer_id: producer_id.into(),
            },
        }
    }

    pub fn consumer_welcome(consumer_id: impl Into<String>) -> Self {
        Self::ConsumerWelcome {
            version: PROTOCOL_VERSION,
            payload: ConsumerWelcomePayload {
                consumer_id: consumer_id.into(),
            },
        }
    }

    pub fn start_data(producer_id: impl Into<String>) -> Self {
        Self::StartData {
            version: PROTOCOL_VERSION,
            payload: SessionRefPayload {
                producer_id: producer_id.into(),
            },
        }
    }

    pub fn stop_data(producer_id: impl Into<String>) -> Self {
        Self::StopData {
            version: PROTOCOL_VERSION,
            payload: SessionRefPayload {
                producer_id: producer_id.into(),
            },
        }
    }

    pub fn input_data(producer_id: impl Into<String>, input: TerminalInput) -> Self {
        Self::InputData {
            version: PROTOCOL_VERSION,
            payload: InputDataPayload {
                producer_id: producer_id.into(),
                input,
            },
        }
    }

    pub fn term_snapshot(producer_id: impl Into<String>, snapshot: TerminalSnapshot) -> Self {
        Self::TermSnapshot {
            version: PROTOCOL_VERSION,
            payload: TermSnapshotPayload {
                producer_id: producer_id.into(),
                snapshot,
            },
        }
    }

    pub fn term_delta(producer_id: impl Into<String>, delta: TerminalDelta) -> Self {
        Self::TermDelta {
            version: PROTOCOL_VERSION,
            payload: TermDeltaPayload {
                producer_id: producer_id.into(),
                delta,
            },
        }
    }

    pub fn session_list(sessions: Vec<SessionSummary>) -> Self {
        Self::SessionList {
            version: PROTOCOL_VERSION,
            payload: SessionListPayload { sessions },
        }
    }

    pub fn session_terminated(
        producer_id: impl Into<String>,
        snapshot: Option<TerminalSnapshot>,
        exit_status: Option<i32>,
        reason: impl Into<String>,
    ) -> Self {
        Self::SessionTerminated {
            version: PROTOCOL_VERSION,
            payload: SessionTerminatedPayload {
                producer_id: producer_id.into(),
                snapshot,
                exit_status,
                reason: reason.into(),
            },
        }
    }

    pub fn consumer_error(message: impl Into<String>) -> Self {
        Self::ConsumerError {
            version: PROTOCOL_VERSION,
            payload: ErrorPayload {
                message: message.into(),
            },
        }
    }

    pub fn server_kick(message: impl Into<String>) -> Self {
        Self::ServerKick {
            version: PROTOCOL_VERSION,
            payload: ErrorPayload {
                message: message.into(),
            },
        }
    }
}

impl TerminalSnapshot {
    pub fn apply_delta(&mut self, delta: &TerminalDelta) {
        self.revision = delta.revision;
        self.cols = delta.cols;
        self.rows = delta.rows;
        self.cursor_row = delta.cursor_row;
        self.cursor_col = delta.cursor_col;
        self.cursor_visible = delta.cursor_visible;
        self.title = delta.title.clone();
        self.exit_status = delta.exit_status;
        let target_len = usize::from(delta.rows);
        if self.lines.len() < target_len {
            self.lines.extend((self.lines.len()..target_len).map(|idx| TerminalLine {
                index: idx as u16,
                runs: vec![],
                wrapped: false,
            }));
        } else if self.lines.len() > target_len {
            self.lines.truncate(target_len);
        }
        for line in &delta.lines {
            let idx = usize::from(line.index);
            if idx < self.lines.len() {
                self.lines[idx] = line.clone();
            }
        }
    }
}
