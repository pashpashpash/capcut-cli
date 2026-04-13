mod cli;
mod downloader;
mod ffmpeg;
mod library;
mod models;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();

    if let Err(error) = cli.run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
