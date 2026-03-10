use std::process::ExitCode;

use clap::Parser;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt;

mod cli;
mod cxxrt;
mod producer;
mod protocol;
mod registry;
mod run_config;
mod server;
mod server_config;
mod terminal;
mod util;

fn main() -> ExitCode {
    let cpprt = cxxrt::cpp_main();
    if cpprt != 0 {
        return ExitCode::from(cpprt as u8);
    }

    let cli = cli::Cli::parse();
    let verbosity = match &cli.command {
        cli::Commands::Server(args) => args.verbose,
        _ => 0,
    };
    init_tracing(verbosity);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to build tokio runtime: {error:#}");
            return ExitCode::FAILURE;
        }
    };

    let result = runtime.block_on(async move {
        match cli.command {
            cli::Commands::Server(args) => server::run(args).await,
            cli::Commands::Run(args) => producer::run(args).await,
            cli::Commands::Keygen(args) => {
                let prefix = match args.kind {
                    cli::KeyKind::Master => "master",
                    cli::KeyKind::Group => "p",
                    cli::KeyKind::ProducerSession => "psk",
                    cli::KeyKind::ConsumerSession => "csk",
                };
                println!("{}", util::generate_key(prefix));
                Ok(())
            }
        }
    });

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => LevelFilter::INFO,
        1 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    };
    let _ = fmt().with_max_level(level).without_time().try_init();
}
