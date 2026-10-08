use anyhow::Result;
use clap::{Parser, Subcommand};

mod cache;
mod download;
mod models;
mod r2;
mod util;
mod verify;
mod wasm;
mod wasm_bundle;

#[derive(Parser)]
#[command(name = "xtask", about = "Zanbergify automation tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Model management commands
    Models(models::ModelsCmd),
    /// WASM build and serve commands
    Wasm(wasm::WasmCmd),
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Models(cmd) => cmd.run(),
        Command::Wasm(cmd) => cmd.run(),
    }
}
