mod apify;
mod cli;
mod config;
mod models;
mod tiktok;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();

    if let Err(error) = cli.run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
