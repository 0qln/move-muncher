use clap::Parser;
use ghost_cell::{GhostCell, GhostToken};
use muncher_core::{castling::_1111, ep_squares::None as EpNone, init, turns::White, perft, Board, Pos};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    depth: u8,
}

fn main() {
    init();
    let args = Args::parse();

    let nodes = GhostToken::new(|mut token| {
        let cell = GhostCell::new(Board::initial());
        let pos = Pos::<White, _1111, EpNone>::new(&cell);
        perft(pos, &mut token, args.depth)
    });

    println!("{nodes}");
}
