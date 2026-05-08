mod adapter;
mod cli;
mod command;
mod io;

fn main() {
    std::process::exit(cli::run());
}
