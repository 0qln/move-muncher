use clap::Parser;
use muncher_core::count_positions;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    depth: u8,
}

fn main() {
    let args = Args::parse();

    let positions = count_positions(args.depth);
}
