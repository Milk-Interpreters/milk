use clap::Parser;
use meraki::compile::{compile, CompileArgs};

fn main() {
    let options = CompileArgs::parse();
    if let Err(e) = compile(options) {
        die(e);
    }
}

fn die(err: Box<dyn std::error::Error>) -> ! {
    eprintln!("{}", err);
    std::process::exit(1);
}
