// Usage: cargo run -p caiman-bridge --example convert -- <in.qcow2> <out.raw>
use caiman_bridge::Qcow2Reader;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: convert <in.qcow2> <out.raw>");
        std::process::exit(2);
    }
    let mut reader = Qcow2Reader::open(&args[1]).expect("open qcow2");
    println!("virtual size: {} bytes", reader.virtual_size());
    reader.convert_to_raw(&args[2]).expect("convert");
    println!("done -> {}", args[2]);
}
