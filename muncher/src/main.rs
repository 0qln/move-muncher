use clap::Parser;
use muncher_core::{init, perft_fen};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    depth: u8,

    /// FEN string (defaults to the starting position).
    #[arg(long)]
    fen: Option<String>,
}

fn main() {
    init();

    let args = Args::parse();

    let fen = args.fen.as_deref().unwrap_or("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    println!("{}", perft_fen(fen, args.depth));
}
