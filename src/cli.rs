use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "hurryvc", version, about = "Lightweight remote terminal handoff")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Server(ServerArgs),
    Run(RunArgs),
    Keygen(KeygenArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct ServerArgs {
    #[arg(long, default_value = "0.0.0.0:6600")]
    pub listen: String,
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Clone, clap::Args)]
pub struct RunArgs {
    #[arg(long, env = "HURRYVC_SERVER")]
    pub server: Option<String>,
    #[arg(long, env = "HURRYVC_MASTER_KEY")]
    pub master_key: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, default_value_t = 120)]
    pub cols: u16,
    #[arg(long, default_value_t = 40)]
    pub rows: u16,
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, clap::Args)]
pub struct KeygenArgs {
    #[arg(value_enum, default_value_t = KeyKind::Master)]
    pub kind: KeyKind,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KeyKind {
    Master,
    Group,
    ProducerSession,
    ConsumerSession,
}
