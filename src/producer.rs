use std::{
    fs::File,
    io::{ErrorKind, Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use crossterm::{
    terminal,
};
use futures::{SinkExt, StreamExt};
use tokio::{
    sync::mpsc,
    time::{Instant, interval},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

cfg_if::cfg_if! {
	if #[cfg(windows)] {
		use tokio::net::windows::named_pipe::NamedPipeServer;
		use std::os::windows::io::RawHandle;
		use std::os::windows::io::AsRawHandle;
		use windows_sys::Win32::{
			Foundation::{ERROR_BROKEN_PIPE, ERROR_HANDLE_EOF, HANDLE},
			Storage::FileSystem::{FILE_TYPE_CHAR, FILE_TYPE_DISK, FILE_TYPE_PIPE, GetFileType},
			System::Console::GetConsoleMode,
			System::Threading::{GetExitCodeProcess, INFINITE, TerminateProcess, WaitForSingleObject},
		};
	} else {
		use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
		use tokio::task;
		use std::io::IsTerminal;
	}
}

#[allow(unused_imports)]
use crate::cxxrt;
use crate::{
    cli::RunArgs,
    protocol::{InputKey, ProducerCapabilities, ProducerExitPayload, ProducerHelloPayload, TerminalInput, WireMessage},
    server_config,
    terminal::{diff_snapshots, snapshot_from_parser},
    util::{derive_ws_url, generate_key},
};

type InputTx = mpsc::UnboundedSender<Vec<u8>>;

struct BackendParts {
    pid: u32,
    input_tx: InputTx,
    chunk_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    exit_rx: mpsc::UnboundedReceiver<i32>,
    resizer: Resizer,
    terminator: Option<Terminator>,
}

enum Resizer {
    #[cfg(not(windows))]
    Portable(Arc<Mutex<Box<dyn MasterPty + Send>>>),
    #[cfg(windows)]
    Windows(cxxrt::ProcessResult),
}

enum Terminator {
    #[cfg(windows)]
    Windows(Arc<WindowsSessionInner>),
}

pub async fn run(args: RunArgs) -> Result<()> {
    let resolved = ResolvedRunArgs::load(args)?;
    let ws_url = derive_ws_url(&resolved.server, "ws/producer")?;
    let session_key = generate_key("psk");
    let local_mode = LocalMode::detect();
    let (initial_cols, initial_rows) = local_mode.initial_size(resolved.cols, resolved.rows);
    let cursor_state = Arc::new(Mutex::new((0_u16, 0_u16)));
    let local_echo = local_mode.output_enabled;
    let responder_enabled = !local_mode.passthrough_replies;
    let sampler = PtySampler::from_env().map(Arc::new);
    let BackendParts {
        pid,
        input_tx,
        mut chunk_rx,
        mut exit_rx,
        resizer,
        terminator,
    } = spawn_backend(
        &resolved,
        initial_cols,
        initial_rows,
        cursor_state.clone(),
        local_echo,
        responder_enabled,
        sampler,
    )?;

    let (local_tx, mut local_rx) = mpsc::unbounded_channel::<LocalCommand>();
    let _local_keepalive = local_tx.clone();
    let local_terminal = LocalTerminal::start(local_mode, local_tx)?;

    let (stream, _) = connect_async(&ws_url).await?;
    let (mut ws_write, mut ws_read) = stream.split();
    let hello = ProducerHelloPayload {
        master_key: resolved.master_key.clone(),
        production_group_key: resolved.group_key.clone(),
        producer_session_key: session_key,
        producer_name: resolved
            .name
            .clone()
            .unwrap_or_else(|| resolved.command.first().cloned().unwrap_or_else(|| "shell".into())),
        command: resolved.command.clone(),
        platform: std::env::consts::OS.to_string(),
        pid,
        cols: initial_cols,
        rows: initial_rows,
        cwd: resolved.cwd.as_ref().map(|cwd| cwd.display().to_string()),
        capabilities: ProducerCapabilities {
            resize: true,
            signals: false,
        },
    };
    send_ws(
        &mut ws_write,
        &WireMessage::ProducerHello {
            version: crate::protocol::PROTOCOL_VERSION,
            payload: hello,
        },
    )
    .await?;

    let mut producer_id = String::new();
    let mut parser = vt100::Parser::new(initial_rows, initial_cols, 0);
    let mut revision = 0_u64;
    let mut last_sent_snapshot = None;
    let mut streaming = false;
    let mut process_exited = false;
    let mut heartbeat = interval(Duration::from_secs(15));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_activity = Instant::now();

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if last_activity.elapsed() >= Duration::from_secs(10) {
                    send_ws(&mut ws_write, &WireMessage::ProducerPing {
                        version: crate::protocol::PROTOCOL_VERSION,
                    }).await?;
                }
            }
            Some(local_command) = local_rx.recv() => {
                match local_command {
                    LocalCommand::Input(bytes) => {
                        write_raw(&input_tx, bytes)?;
                    }
                }
            }
            Some(chunk) = chunk_rx.recv() => {
                parser.process(&chunk);
                let (cursor_row, cursor_col) = parser.screen().cursor_position();
                *cursor_state.lock().expect("cursor state poisoned") = (cursor_row, cursor_col);
                revision += 1;
                last_activity = Instant::now();
                if streaming && !producer_id.is_empty() {
                    let snapshot = snapshot_from_parser(&parser, revision, None);
                    match &last_sent_snapshot {
                        None => {
                            send_ws(&mut ws_write, &WireMessage::term_snapshot(producer_id.clone(), snapshot.clone())).await?;
                        }
                        Some(previous) => {
                            if let Some(delta) = diff_snapshots(previous, &snapshot) {
                                send_ws(&mut ws_write, &WireMessage::term_delta(producer_id.clone(), delta)).await?;
                            }
                        }
                    }
                    last_sent_snapshot = Some(snapshot);
                }
            }
            Some(status) = exit_rx.recv() => {
                process_exited = true;
                let exit_status = Some(status);
                revision += 1;
                let final_snapshot = snapshot_from_parser(&parser, revision, exit_status);
                send_ws(&mut ws_write, &WireMessage::ProducerExit {
                    version: crate::protocol::PROTOCOL_VERSION,
                    payload: ProducerExitPayload {
                        producer_id: producer_id.clone(),
                        exit_status,
                        snapshot: Some(final_snapshot),
                        reason: "process exited".into(),
                    },
                }).await?;
                info!("producer process exited");
                break;
            }
            Some(message) = ws_read.next() => {
                match parse_ws(message?)? {
                    WireMessage::ProducerWelcome { payload, .. } => {
                        producer_id = payload.producer_id;
                    }
                    WireMessage::StartData { .. } => {
                        streaming = true;
                        revision += 1;
                        let snapshot = snapshot_from_parser(&parser, revision, None);
                        send_ws(&mut ws_write, &WireMessage::term_snapshot(producer_id.clone(), snapshot.clone())).await?;
                        last_sent_snapshot = Some(snapshot);
                    }
                    WireMessage::StopData { .. } => {
                        streaming = false;
                    }
                    WireMessage::InputData { payload, .. } => {
                        write_input(&input_tx, payload.input)?;
                    }
                    WireMessage::Resize { payload, .. } => {
                        resizer.apply(&mut parser, payload.cols, payload.rows).await?;
                    }
                    WireMessage::ServerKick { payload, .. } => {
                        warn!("producer was kicked: {}", payload.message);
                        break;
                    }
                    WireMessage::ConsumerError { payload, .. } => {
                        warn!("server error: {}", payload.message);
                    }
                    other => warn!("unexpected server message: {:?}", other),
                }
            }
            else => break,
        }
    }

    drop(local_terminal);
    if !process_exited {
        if let Some(terminator) = terminator {
            terminate_backend(terminator);
        }
    }
    Ok(())
}

fn spawn_backend(
    resolved: &ResolvedRunArgs,
    cols: u16,
    rows: u16,
    cursor_state: Arc<Mutex<(u16, u16)>>,
    local_echo: bool,
    responder_enabled: bool,
    sampler: Option<Arc<PtySampler>>,
) -> Result<BackendParts> {
    #[cfg(windows)]
    {
        return spawn_backend_windows(resolved, cols, rows, cursor_state, local_echo, responder_enabled, sampler);
    }

    #[cfg(not(windows))]
    {
        spawn_backend_portable(resolved, cols, rows, cursor_state, local_echo, responder_enabled, sampler)
    }
}

#[cfg(not(windows))]
fn spawn_backend_portable(
    resolved: &ResolvedRunArgs,
    cols: u16,
    rows: u16,
    cursor_state: Arc<Mutex<(u16, u16)>>,
    local_echo: bool,
    responder_enabled: bool,
    sampler: Option<Arc<PtySampler>>,
) -> Result<BackendParts> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let mut child = pair
        .slave
        .spawn_command(command_builder(resolved))
        .context("failed to spawn command in pty")?;
    let pid = child.process_id().unwrap_or_default();
    drop(pair.slave);

    let master = Arc::new(Mutex::new(pair.master));
    let mut raw_writer = {
        let guard = master.lock().expect("pty master poisoned");
        guard.take_writer().context("failed to open pty writer")?
    };
    let mut reader = {
        let guard = master.lock().expect("pty master poisoned");
        guard
            .try_clone_reader()
            .context("failed to clone pty reader")?
    };
    let (chunk_tx, chunk_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (exit_tx, exit_rx) = mpsc::unbounded_channel::<i32>();
    let reader_cursor_state = cursor_state;
    let reply_input_tx = input_tx.clone();
    std::thread::spawn(move || {
        while let Some(bytes) = input_rx.blocking_recv() {
            if raw_writer.write_all(&bytes).is_err() || raw_writer.flush().is_err() {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        let mut responder = TerminalResponder::default();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if let Some(sampler) = &sampler {
                        sampler.record_chunk(&buffer[..read]);
                    }
                    if local_echo {
                        let _ = std::io::stdout().write_all(&buffer[..read]);
                        let _ = std::io::stdout().flush();
                    }
                    if responder_enabled {
                        let (cursor_row, cursor_col) =
                            *reader_cursor_state.lock().expect("cursor state poisoned");
                        for reply in responder.replies_for_chunk(&buffer[..read], cursor_row, cursor_col) {
                            let _ = reply_input_tx.send(reply);
                        }
                    }
                    if chunk_tx.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::TimedOut
                    ) =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    std::thread::spawn(move || {
        if let Ok(status) = child.wait() {
            let _ = exit_tx.send(status.exit_code() as i32);
        }
    });

    Ok(BackendParts {
        pid,
        input_tx,
        chunk_rx,
        exit_rx,
        resizer: Resizer::Portable(master),
        terminator: None,
    })
}

#[cfg(windows)]
fn spawn_backend_windows(
    resolved: &ResolvedRunArgs,
    cols: u16,
    rows: u16,
    cursor_state: Arc<Mutex<(u16, u16)>>,
    local_echo: bool,
    responder_enabled: bool,
    sampler: Option<Arc<PtySampler>>,
) -> Result<BackendParts> {
    let cwd = resolved
        .cwd
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let cmdline = windows_cmdline(&resolved.command);
    let mut process = cxxrt::run_process_in_pty(&cmdline, cols, rows, &cwd);
    if process.code != 0 {
        anyhow::bail!("run_process_in_pty_ex failed with {}", process.code);
    }
    let input_handle = process.input_writer as RawHandle;
    let output_handle = process.output_reader as RawHandle;
    process.input_writer = 0;
    process.output_reader = 0;

    let session = Arc::new(WindowsSessionInner { process });
    let input_pipe = Arc::new(unsafe { NamedPipeServer::from_raw_handle(input_handle) }
        .context("failed to adopt input named pipe handle")?);
    let output_pipe = Arc::new(unsafe { NamedPipeServer::from_raw_handle(output_handle) }
        .context("failed to adopt output named pipe handle")?);
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (chunk_tx, chunk_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (exit_tx, exit_rx) = mpsc::unbounded_channel::<i32>();
    let reader_cursor_state = cursor_state;
    let reply_input_tx = input_tx.clone();
    let input_pipe_writer = input_pipe.clone();
    tokio::spawn(async move {
        while let Some(bytes) = input_rx.recv().await {
            if let Err(_error) = write_pipe_bytes(input_pipe_writer.as_ref(), &bytes).await {
                break;
            }
        }
    });
    tokio::spawn(async move {
        let mut responder = TerminalResponder::default();
        loop {
            match read_pipe_chunk(output_pipe.as_ref()).await {
                Ok(Some(chunk)) => {
                    if let Some(sampler) = &sampler {
                        sampler.record_chunk(&chunk);
                    }
                    if local_echo {
                        let _ = std::io::stdout().write_all(&chunk);
                        let _ = std::io::stdout().flush();
                    }
                    if responder_enabled {
                        let (cursor_row, cursor_col) =
                            *reader_cursor_state.lock().expect("cursor state poisoned");
                        for reply in responder.replies_for_chunk(&chunk, cursor_row, cursor_col) {
                            let _ = reply_input_tx.send(reply);
                        }
                    }
                    if chunk_tx.send(chunk).is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(_error) => {
                    break;
                }
            }
        }
    });

    let waiter_session = session.clone();
    std::thread::spawn(move || {
        unsafe {
            WaitForSingleObject(waiter_session.process.hprocess as HANDLE, INFINITE);
        }
        let mut code = 1_u32;
        unsafe {
            GetExitCodeProcess(waiter_session.process.hprocess as HANDLE, &mut code);
        }
        let _ = exit_tx.send(code as i32);
    });

    Ok(BackendParts {
        pid: session.process.pid,
        input_tx,
        chunk_rx,
        exit_rx,
        resizer: Resizer::Windows(session.process.clone()),
        terminator: Some(Terminator::Windows(session)),
    })
}

#[cfg(not(windows))]
fn command_builder(args: &ResolvedRunArgs) -> CommandBuilder {
    let argv = args.command.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    let mut builder = CommandBuilder::from_argv(argv);
    if let Some(cwd) = &args.cwd {
        builder.cwd(cwd);
    } else if let Ok(cwd) = std::env::current_dir() {
        builder.cwd(cwd);
    }
    builder
}

#[cfg(windows)]
fn windows_cmdline(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_windows_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(windows)]
fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.chars().any(|ch| matches!(ch, ' ' | '\t' | '"')) {
        return arg.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for ch in arg.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                if backslashes > 0 {
                    quoted.push_str(&"\\".repeat(backslashes));
                    backslashes = 0;
                }
                quoted.push(ch);
            }
        }
    }
    if backslashes > 0 {
        quoted.push_str(&"\\".repeat(backslashes * 2));
    }
    quoted.push('"');
    quoted
}

struct ResolvedRunArgs {
    server: String,
    master_key: String,
    group_key: String,
    name: Option<String>,
    cols: u16,
    rows: u16,
    cwd: Option<std::path::PathBuf>,
    command: Vec<String>,
}

#[derive(Clone, Copy)]
struct LocalMode {
    console_input: bool,
    output_enabled: bool,
    passthrough_replies: bool,
}

impl LocalMode {
    fn detect() -> Self {
        #[cfg(windows)]
        {
            let stdin_kind = stdio_kind(std::io::stdin().as_raw_handle() as isize);
            let stdout_kind = stdio_kind(std::io::stdout().as_raw_handle() as isize);
            let console_input = matches!(stdin_kind, StdIoKind::Console)
                && matches!(stdout_kind, StdIoKind::Console);
            let passthrough_replies = matches!(stdin_kind, StdIoKind::Console | StdIoKind::Pipe)
                && matches!(stdout_kind, StdIoKind::Console | StdIoKind::Pipe);
            return Self {
                console_input,
                output_enabled: true,
                passthrough_replies,
            };
        }

        #[cfg(not(windows))]
        {
            let console_input = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
            Self {
                console_input,
                output_enabled: true,
                passthrough_replies: console_input,
            }
        }
    }

    fn initial_size(self, fallback_cols: u16, fallback_rows: u16) -> (u16, u16) {
        if self.console_input {
            if let Ok((cols, rows)) = terminal::size() {
                return (cols, rows);
            }
        }
        (fallback_cols, fallback_rows)
    }
}

enum LocalCommand {
    Input(Vec<u8>),
}

struct LocalTerminal {
    stop: Arc<AtomicBool>,
    raw_mode: bool,
}

struct PtySampler {
    file: Mutex<File>,
}

impl PtySampler {
    fn from_env() -> Option<Self> {
        let enabled = std::env::var("HURRYVC_PTY_SAMPLE").ok()?;
        if enabled != "1" {
            return None;
        }
        let path = std::env::var("HURRYVC_PTY_SAMPLE_PATH")
            .ok()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                crate::util::config_dir()
                    .unwrap_or_else(|_| std::env::temp_dir())
                    .join("pty-sample.log")
            });
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut file = File::create(&path).ok()?;
        let _ = writeln!(file, "# hurryvc pty sample");
        let _ = writeln!(file, "# path: {}", path.display());
        Some(Self {
            file: Mutex::new(file),
        })
    }

    fn record_chunk(&self, chunk: &[u8]) {
        let escaped = escape_bytes(chunk);
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "[{}] {}", chunk.len(), escaped);
        }
    }
}

fn escape_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 4);
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\r' => out.push_str("\\r"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => {
                let _ = std::fmt::Write::write_fmt(&mut out, format_args!("\\x{byte:02X}"));
            }
        }
    }
    out
}

impl LocalTerminal {
    fn start(mode: LocalMode, tx: mpsc::UnboundedSender<LocalCommand>) -> Result<Option<Self>> {
        if !mode.console_input && !mode.passthrough_replies {
            return Ok(None);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let raw_mode = if mode.console_input {
            terminal::enable_raw_mode().context("failed to enable raw mode")?;
            true
        } else {
            false
        };
        let thread_stop = stop.clone();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut locked = stdin.lock();
            let mut buffer = [0_u8; 1024];
            while !thread_stop.load(Ordering::Relaxed) {
                match locked.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        if tx.send(LocalCommand::Input(buffer[..read].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            ErrorKind::WouldBlock | ErrorKind::Interrupted | ErrorKind::TimedOut
                        ) =>
                    {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Some(Self {
            stop,
            raw_mode,
        }))
    }
}

impl Drop for LocalTerminal {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if self.raw_mode {
            let _ = terminal::disable_raw_mode();
        }
    }
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum StdIoKind {
    Console,
    Pipe,
    Other,
}

#[cfg(windows)]
fn stdio_kind(handle: isize) -> StdIoKind {
    let mut mode = 0_u32;
    // SAFETY: handle is a process std handle obtained from AsRawHandle.
    if unsafe { GetConsoleMode(handle as _, &mut mode) } != 0 {
        return StdIoKind::Console;
    }
    // SAFETY: handle is a process std handle obtained from AsRawHandle.
    match unsafe { GetFileType(handle as _) } {
        FILE_TYPE_PIPE => StdIoKind::Pipe,
        FILE_TYPE_CHAR | FILE_TYPE_DISK => StdIoKind::Other,
        _ => StdIoKind::Other,
    }
}

impl ResolvedRunArgs {
    fn load(args: RunArgs) -> Result<Self> {
        let run_config = crate::run_config::load_or_create()?;
        let fallback_master_key = if Self::has_explicit_master_key(args.master_key.as_deref()) {
            None
        } else {
            Some(server_config::load_existing()?.master_key)
        };
        Ok(Self::from_sources(args, run_config, fallback_master_key))
    }

    fn has_explicit_master_key(master_key: Option<&str>) -> bool {
        matches!(master_key, Some(value) if !value.trim().is_empty())
    }

    fn from_sources(
        args: RunArgs,
        run_config: crate::run_config::RunConfig,
        fallback_master_key: Option<String>,
    ) -> Self {
        let master_key = match args.master_key {
            Some(master_key) if !master_key.trim().is_empty() => master_key,
            _ => fallback_master_key
                .expect("fallback master key should be loaded when CLI master key is missing"),
        };
        let server = args
            .server
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(run_config.server);
        Self {
            server,
            master_key,
            group_key: run_config.group_key,
            name: args.name,
            cols: args.cols,
            rows: args.rows,
            cwd: args.cwd,
            command: args.command,
        }
    }
}

impl Resizer {
    async fn apply(&self, parser: &mut vt100::Parser, cols: u16, rows: u16) -> Result<()> {
        match self {
            #[cfg(not(windows))]
            Resizer::Portable(master) => {
                resize_portable(master.clone(), parser, cols, rows).await?;
            }
            #[cfg(windows)]
            Resizer::Windows(process) => {
                parser.screen_mut().set_size(rows, cols);
                let result = cxxrt::resize_process_in_pty(process, cols, rows);
                if result != 0 {
                    anyhow::bail!("resize_process_in_pty failed with {result}");
                }
            }
        }
        Ok(())
    }
}

fn terminate_backend(terminator: Terminator) {
    match terminator {
        #[cfg(windows)]
        Terminator::Windows(session) => session.terminate(),
    }
}

#[cfg(windows)]
struct WindowsSessionInner {
    process: cxxrt::ProcessResult,
}

#[cfg(windows)]
impl WindowsSessionInner {
    fn terminate(&self) {
        unsafe {
            let _ = TerminateProcess(self.process.hprocess as HANDLE, 0);
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsSessionInner {
    fn drop(&mut self) {
        cxxrt::destory_process(&self.process);
    }
}

#[cfg(windows)]
async fn write_pipe_bytes(pipe: &NamedPipeServer, mut data: &[u8]) -> std::io::Result<()> {
    while !data.is_empty() {
        pipe.writable().await?;
        match pipe.try_write(data) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "zero-byte named pipe write",
                ));
            }
            Ok(written) => data = &data[written..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(windows)]
async fn read_pipe_chunk(pipe: &NamedPipeServer) -> std::io::Result<Option<Vec<u8>>> {
    let mut buffer = vec![0_u8; 4096];
    loop {
        pipe.readable().await?;
        match pipe.try_read(&mut buffer) {
            Ok(0) => return Ok(None),
            Ok(read) => {
                buffer.truncate(read);
                return Ok(Some(buffer));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(error) => {
                if matches!(
                    error.raw_os_error(),
                    Some(code) if code == ERROR_BROKEN_PIPE as i32 || code == ERROR_HANDLE_EOF as i32
                ) {
                    return Ok(None);
                }
                return Err(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Mutex};

    use anyhow::Result;
    use uuid::Uuid;

    use super::{ResolvedRunArgs, TerminalResponder, escape_bytes};
    use crate::{cli::RunArgs, run_config::RunConfig};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_home_dir() -> PathBuf {
        std::env::temp_dir().join(format!("hurryvc-producer-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn resolved_run_args_falls_back_to_run_and_server_configs() {
        let resolved = ResolvedRunArgs::from_sources(
            RunArgs {
                server: None,
                master_key: None,
                name: Some("demo".into()),
                cols: 100,
                rows: 30,
                cwd: None,
                command: vec!["pwsh".into()],
            },
            RunConfig {
                server: "127.0.0.1:6600".into(),
                group_key: "p-demo".into(),
            },
            Some("master-demo".into()),
        );

        assert_eq!(resolved.server, "127.0.0.1:6600");
        assert_eq!(resolved.group_key, "p-demo");
        assert_eq!(resolved.master_key, "master-demo");
    }

    #[test]
    fn resolved_run_args_prefers_cli_server_and_master_key() {
        let resolved = ResolvedRunArgs::from_sources(
            RunArgs {
                server: Some("ws://example.com/base".into()),
                master_key: Some("master-cli".into()),
                name: None,
                cols: 120,
                rows: 40,
                cwd: None,
                command: vec!["cmd".into()],
            },
            RunConfig {
                server: "127.0.0.1:6600".into(),
                group_key: "p-demo".into(),
            },
            Some("master-fallback".into()),
        );

        assert_eq!(resolved.server, "ws://example.com/base");
        assert_eq!(resolved.master_key, "master-cli");
    }

    #[test]
    fn resolved_run_args_load_skips_missing_server_config_when_cli_master_key_present() -> Result<()> {
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let home = temp_home_dir();
        fs::create_dir_all(&home)?;
        let previous_home = std::env::var_os("HOME");

        // SAFETY: tests serialize process-wide environment mutations with ENV_LOCK.
        unsafe {
            std::env::set_var("HOME", &home);
        }

        let result = ResolvedRunArgs::load(RunArgs {
            server: Some("https://example.com/hurryvc".into()),
            master_key: Some("master-example".into()),
            name: None,
            cols: 120,
            rows: 40,
            cwd: None,
            command: vec!["pwsh".into()],
        });

        match previous_home {
            Some(value) => {
                // SAFETY: tests serialize process-wide environment mutations with ENV_LOCK.
                unsafe {
                    std::env::set_var("HOME", value);
                }
            }
            None => {
                // SAFETY: tests serialize process-wide environment mutations with ENV_LOCK.
                unsafe {
                    std::env::remove_var("HOME");
                }
            }
        }
        let _ = fs::remove_dir_all(&home);

        let resolved = result?;
        assert_eq!(resolved.server, "https://example.com/hurryvc");
        assert_eq!(resolved.master_key, "master-example");
        Ok(())
    }

    #[test]
    fn terminal_responder_replies_to_cursor_position_query() {
        let mut responder = TerminalResponder::default();
        let replies = responder.replies_for_chunk(b"\x1b[6n", 2, 4);
        assert_eq!(replies, vec![b"\x1b[3;5R".to_vec()]);
    }

    #[test]
    fn terminal_responder_handles_split_query_across_chunks() {
        let mut responder = TerminalResponder::default();
        assert!(responder.replies_for_chunk(b"\x1b[", 0, 0).is_empty());
        let replies = responder.replies_for_chunk(b"6n", 0, 0);
        assert_eq!(replies, vec![b"\x1b[1;1R".to_vec()]);
    }

    #[test]
    fn escape_bytes_preserves_ansi_sequences() {
        let escaped = escape_bytes(b"\x1b[93mdir\x1b[38;2;68;68;68m .\\zlib.lib");
        assert_eq!(escaped, "\\x1B[93mdir\\x1B[38;2;68;68;68m .\\\\zlib.lib");
    }
}

fn write_input(input_tx: &InputTx, input: TerminalInput) -> Result<()> {
    let data = encode_input(input);
    write_raw(input_tx, data)
}

fn write_raw(input_tx: &InputTx, data: Vec<u8>) -> Result<()> {
    input_tx
        .send(data)
        .map_err(|_| anyhow!("pty input channel closed"))
}

#[derive(Default)]
struct TerminalResponder {
    pending: Vec<u8>,
}

impl TerminalResponder {
    fn replies_for_chunk(&mut self, chunk: &[u8], cursor_row: u16, cursor_col: u16) -> Vec<Vec<u8>> {
        let pattern = b"\x1b[6n";
        let mut combined = self.pending.clone();
        combined.extend_from_slice(chunk);

        let mut replies = Vec::new();
        let mut index = 0;
        while index + pattern.len() <= combined.len() {
            if &combined[index..index + pattern.len()] == pattern {
                replies.push(format!("\x1b[{};{}R", cursor_row + 1, cursor_col + 1).into_bytes());
                index += pattern.len();
            } else {
                index += 1;
            }
        }

        self.pending.clear();
        let max_suffix = pattern.len() - 1;
        for len in (1..=max_suffix.min(combined.len())).rev() {
            if combined[combined.len() - len..] == pattern[..len] {
                self.pending.extend_from_slice(&combined[combined.len() - len..]);
                break;
            }
        }
        replies
    }
}

#[cfg(not(windows))]
async fn resize_portable(
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    parser: &mut vt100::Parser,
    cols: u16,
    rows: u16,
) -> Result<()> {
    parser.screen_mut().set_size(rows, cols);
    task::spawn_blocking(move || {
        master
            .lock()
            .expect("pty master poisoned")
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        Result::<(), anyhow::Error>::Ok(())
    })
    .await??;
    Ok(())
}

fn encode_input(input: TerminalInput) -> Vec<u8> {
    match input {
        TerminalInput::Text { data } => data.into_bytes(),
        TerminalInput::Key { key } => match key {
            InputKey::Enter => b"\r".to_vec(),
            InputKey::Tab => b"\t".to_vec(),
            InputKey::Backspace => vec![0x08],
            InputKey::Escape => vec![0x1b],
            InputKey::ArrowUp => b"\x1b[A".to_vec(),
            InputKey::ArrowDown => b"\x1b[B".to_vec(),
            InputKey::ArrowRight => b"\x1b[C".to_vec(),
            InputKey::ArrowLeft => b"\x1b[D".to_vec(),
            InputKey::CtrlC => vec![0x03],
            InputKey::CtrlD => vec![0x04],
        },
    }
}

async fn send_ws(
    writer: &mut futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >,
    message: &WireMessage,
) -> Result<()> {
    writer
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

fn parse_ws(message: Message) -> Result<WireMessage> {
    match message {
        Message::Text(text) => Ok(serde_json::from_str(text.as_ref())?),
        Message::Binary(data) => Ok(serde_json::from_slice(&data)?),
        Message::Close(_) => Err(anyhow!("server closed websocket")),
        Message::Ping(_) | Message::Pong(_) => Err(anyhow!("control frame")),
        Message::Frame(_) => Err(anyhow!("raw frame")),
    }
}
