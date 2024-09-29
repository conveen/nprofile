use clap::Parser;

mod cli;
mod logging;

fn main() {
    cli::Cli::parse().run()
}
