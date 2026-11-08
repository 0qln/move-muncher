#![allow(unused_labels)]
#![allow(non_camel_case_types)]

use engine::core::{
    bitboard::Bitboard,
    color::{colors, Color},
    coordinates::{castling as castling_utils, files, pawn_utils, Square},
    move_iter::{bishop, king, knight, pawn, rook, sliding_piece::SlidingAttacks},
    piece::{piece_type, Piece},
    position::PieceInfo,
};
use ghost_cell::{GhostCell, GhostToken};
use std::marker::PhantomData;

pub trait Turn {
    type Next: Turn;
    type Prev: Turn;

    /// The runtime color this turn represents.
    const COLOR: Color;

    type Castling_TakeK<C: Castling>: Castling;
    type Castling_TakeQ<C: Castling>: Castling;
}

mod turns {
    use super::*;

    struct White;
    struct Black;

    impl Turn for White {
        type Next = Black;
        type Prev = Black;
        const COLOR: Color = colors::WHITE;

        type Castling_TakeK<C: Castling> = C::Take_1000;
        type Castling_TakeQ<C: Castling> = C::Take_0100;
    }

    impl Turn for Black {
        type Next = White;
        type Prev = White;
        const COLOR: Color = colors::BLACK;

        type Castling_TakeK<C: Castling> = C::Take_0010;
        type Castling_TakeQ<C: Castling> = C::Take_0001;
    }
}

#[allow(non_camel_case_types)]
pub trait Castling {
    /// The next castling state without KING_WHITE.
    type Take_1000: Castling;

    /// The next castling state without QUEEN_WHITE.
    type Take_0100: Castling;

    /// The next castling state without KING_BLACK.
    type Take_0010: Castling;

    /// The next castling state without QUEEN_BLACK.
    type Take_0001: Castling;

    /// The runtime castling-rights bitmask:
    /// KING_WHITE=0b1000, QUEEN_WHITE=0b0100, KING_BLACK=0b0010, QUEEN_BLACK=0b0001.
    const RIGHTS: u8;

    type Take_Q<T: Turn>: Castling;
    type Take_K<T: Turn>: Castling;

    /// The next castling state after a move whose flag is `M::Flag` is played.
    type Next<M: Move, T: Turn>: Castling;
}

mod castling {
    use super::*;

    macro_rules! impl_castling_states {
        ($( ($st:ident, $t1000:ident, $t0100:ident, $t0010:ident, $t0001:ident, $rights:literal) ),* $(,)?) => {
            $(
                pub struct $st;
                impl Castling for $st {
                    type Take_1000 = $t1000;
                    type Take_0100 = $t0100;
                    type Take_0010 = $t0010;
                    type Take_0001 = $t0001;
                    const RIGHTS: u8 = $rights;
                    type Take_K<T: Turn> = <T as Turn>::Castling_TakeK<Self>;
                    type Take_Q<T: Turn> = <T as Turn>::Castling_TakeQ<Self>;
                    type Next<M: Move, T: Turn> = <M::Flag as MoveFlag>::Apply<Self, T>;
                }
            )*
        };
    }

    impl_castling_states! {
        (_0000, _0000, _0000, _0000, _0000, 0b0000),
        (_0001, _0001, _0001, _0001, _0000, 0b0001),
        (_0010, _0010, _0010, _0000, _0010, 0b0010),
        (_0011, _0011, _0011, _0001, _0010, 0b0011),
        (_0100, _0100, _0000, _0100, _0100, 0b0100),
        (_0101, _0101, _0001, _0101, _0100, 0b0101),
        (_0110, _0110, _0010, _0100, _0110, 0b0110),
        (_0111, _0111, _0011, _0101, _0110, 0b0111),
        (_1000, _0000, _1000, _1000, _1000, 0b1000),
        (_1001, _0001, _1001, _1001, _1000, 0b1001),
        (_1010, _0010, _1010, _1000, _1010, 0b1010),
        (_1011, _0011, _1011, _1001, _1010, 0b1011),
        (_1100, _0100, _1000, _1100, _1100, 0b1100),
        (_1101, _0101, _1001, _1101, _1100, 0b1101),
        (_1110, _0110, _1010, _1100, _1110, 0b1110),
        (_1111, _0111, _1011, _1101, _1110, 0b1111),
    }
}

pub trait EpSquare {
    type Next<M: Move>: EpSquare;
}
mod ep_squares {
    use super::*;

    struct None;
    struct Some(Square);
}

pub trait Move: Copy {
    type Flag: MoveFlag;

    fn from(&self) -> Square;
    fn to(&self) -> Square;
}

/// A move value carrying its from/to squares, with the flag encoded at the
/// type level so transitions like `Castling::Next<M, T>` see it statically.
pub struct MoveValue<F: MoveFlag> {
    pub from: Square,
    pub to: Square,
    _flag: PhantomData<F>,
}

impl<F: MoveFlag> Clone for MoveValue<F> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<F: MoveFlag> Copy for MoveValue<F> {}
impl<F: MoveFlag> Move for MoveValue<F> {
    type Flag = F;
    fn from(&self) -> Square {
        self.from
    }
    fn to(&self) -> Square {
        self.to
    }
}

/// The board: piece placement (`PieceInfo`) plus the runtime en-passant square
/// and an undo stack for captures. Turn, castling and en-passant availability
/// live in the type parameters of `Pos`; only the *square* is runtime.
pub struct Board {
    pieces: PieceInfo,
    ep_square: Option<Square>,
    history: Vec<Undo>,
}

struct Undo {
    captured: Option<(Square, Piece)>,
    prev_ep: Option<Square>,
}

impl Board {
    fn move_piece(&mut self, from: Square, to: Square) {
        self.pieces.move_piece(from, to, &mut ());
    }
    fn remove_piece(&mut self, sq: Square) -> Piece {
        self.pieces.remove_piece(sq, &mut ())
    }
    fn put_piece(&mut self, sq: Square, piece: Piece) {
        self.pieces.put_piece(sq, piece, &mut ());
    }

    /// Push an undo record (clearing the en-passant square) for the move about
    /// to be applied.
    fn begin_move(&mut self, captured: Option<(Square, Piece)>) {
        let prev_ep = self.ep_square;
        self.ep_square = None;
        self.history.push(Undo { captured, prev_ep });
    }

    /// Pop the undo record, restoring the en-passant square, and return the
    /// captured piece (if any) to be re-placed.
    fn end_move(&mut self) -> Option<(Square, Piece)> {
        let undo = self.history.pop().unwrap();
        self.ep_square = undo.prev_ep;
        undo.captured
    }
}

pub trait MoveFlag {
    /// The castling state after a move carrying this flag is played.
    type Apply<C: Castling, T: Turn>: Castling;

    fn make(board: &mut Board, from: Square, to: Square, us: Color);
    fn unmake(board: &mut Board, from: Square, to: Square, us: Color);
}

mod move_flags {
    use super::*;

    pub struct Quiet;
    pub struct DoublePawnPush;
    pub struct PromotionKnight;
    pub struct PromotionBishop;
    pub struct PromotionRook;
    pub struct PromotionQueen;
    pub struct CapturePromotionKnight;
    pub struct CapturePromotionBishop;
    pub struct CapturePromotionRook;
    pub struct CapturePromotionQueen;
    pub struct KingCastle;
    pub struct QueenCastle;
    pub struct Capture;
    pub struct EnPassant;

    impl MoveFlag for Quiet {
        type Apply<C: Castling, T: Turn> = C;
        fn make(board: &mut Board, from: Square, to: Square, _us: Color) {
            board.begin_move(None);
            board.move_piece(from, to);
        }
        fn unmake(board: &mut Board, from: Square, to: Square, _us: Color) {
            board.move_piece(to, from);
            board.end_move();
        }
    }

    impl MoveFlag for DoublePawnPush {
        type Apply<C: Castling, T: Turn> = C;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            board.begin_move(None);
            board.move_piece(from, to);
            let ep = pawn_utils::backward(Bitboard::from(to), pawn_utils::single_step(us)).lsb().unwrap();
            board.ep_square = Some(ep);
        }
        fn unmake(board: &mut Board, from: Square, to: Square, _us: Color) {
            board.move_piece(to, from);
            board.end_move();
        }
    }

    impl MoveFlag for Capture {
        type Apply<C: Castling, T: Turn> = C;
        fn make(board: &mut Board, from: Square, to: Square, _us: Color) {
            let captured = board.remove_piece(to);
            board.begin_move(Some((to, captured)));
            board.move_piece(from, to);
        }
        fn unmake(board: &mut Board, from: Square, to: Square, _us: Color) {
            board.move_piece(to, from);
            let (sq, cap) = board.end_move().unwrap();
            board.put_piece(sq, cap);
        }
    }

    impl MoveFlag for EnPassant {
        type Apply<C: Castling, T: Turn> = C;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            let cap_sq = pawn_utils::backward(Bitboard::from(to), pawn_utils::single_step(us)).lsb().unwrap();
            let captured = board.remove_piece(cap_sq);
            board.begin_move(Some((cap_sq, captured)));
            board.move_piece(from, to);
        }
        fn unmake(board: &mut Board, from: Square, to: Square, _us: Color) {
            board.move_piece(to, from);
            let (sq, cap) = board.end_move().unwrap();
            board.put_piece(sq, cap);
        }
    }

    macro_rules! impl_promo {
        ($flag:ident, $pt:path) => {
            impl MoveFlag for $flag {
                type Apply<C: Castling, T: Turn> = C;
                fn make(board: &mut Board, from: Square, to: Square, us: Color) {
                    board.begin_move(None);
                    board.remove_piece(from);
                    board.put_piece(to, Piece::from((us, $pt)));
                }
                fn unmake(board: &mut Board, from: Square, to: Square, us: Color) {
                    board.remove_piece(to);
                    board.put_piece(from, Piece::from((us, piece_type::PAWN)));
                    board.end_move();
                }
            }
        };
    }
    impl_promo!(PromotionKnight, piece_type::KNIGHT);
    impl_promo!(PromotionBishop, piece_type::BISHOP);
    impl_promo!(PromotionRook, piece_type::ROOK);
    impl_promo!(PromotionQueen, piece_type::QUEEN);

    macro_rules! impl_capture_promo {
        ($flag:ident, $pt:path) => {
            impl MoveFlag for $flag {
                type Apply<C: Castling, T: Turn> = C;
                fn make(board: &mut Board, from: Square, to: Square, us: Color) {
                    let captured = board.remove_piece(to);
                    board.begin_move(Some((to, captured)));
                    board.remove_piece(from);
                    board.put_piece(to, Piece::from((us, $pt)));
                }
                fn unmake(board: &mut Board, from: Square, to: Square, us: Color) {
                    board.remove_piece(to);
                    board.put_piece(from, Piece::from((us, piece_type::PAWN)));
                    let (sq, cap) = board.end_move().unwrap();
                    board.put_piece(sq, cap);
                }
            }
        };
    }
    impl_capture_promo!(CapturePromotionKnight, piece_type::KNIGHT);
    impl_capture_promo!(CapturePromotionBishop, piece_type::BISHOP);
    impl_capture_promo!(CapturePromotionRook, piece_type::ROOK);
    impl_capture_promo!(CapturePromotionQueen, piece_type::QUEEN);

    impl MoveFlag for KingCastle {
        type Apply<C: Castling, T: Turn> = <<C as Castling>::Take_K<T> as Castling>::Take_Q<T>;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            let rank = castling_utils::castling_rank(us);
            board.begin_move(None);
            board.move_piece(from, to);
            board.move_piece(Square::from((files::H, rank)), Square::from((files::F, rank)));
        }
        fn unmake(board: &mut Board, from: Square, to: Square, us: Color) {
            let rank = castling_utils::castling_rank(us);
            board.move_piece(to, from);
            board.move_piece(Square::from((files::F, rank)), Square::from((files::H, rank)));
            board.end_move();
        }
    }

    impl MoveFlag for QueenCastle {
        type Apply<C: Castling, T: Turn> = <<C as Castling>::Take_K<T> as Castling>::Take_Q<T>;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            let rank = castling_utils::castling_rank(us);
            board.begin_move(None);
            board.move_piece(from, to);
            board.move_piece(Square::from((files::A, rank)), Square::from((files::D, rank)));
        }
        fn unmake(board: &mut Board, from: Square, to: Square, us: Color) {
            let rank = castling_utils::castling_rank(us);
            board.move_piece(to, from);
            board.move_piece(Square::from((files::D, rank)), Square::from((files::A, rank)));
            board.end_move();
        }
    }
}

pub struct Pos<'id, Turn, Castling, EpSquare> {
    _turn: PhantomData<Turn>,
    _castling: PhantomData<Castling>,
    _ep_square: PhantomData<EpSquare>,

    board: &'id GhostCell<'id, Board>,
}

impl<T, C, Ep> Clone for Pos<'_, T, C, Ep> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, C, Ep> Copy for Pos<'_, T, C, Ep> {}

impl<'id, T: Turn, C: Castling, Ep: EpSquare> Pos<'id, T, C, Ep> {
    fn make_move<M: Move>(&self, m: M, token: &mut GhostToken<'id>) -> Pos<'id, T::Next, C::Next<M, T>, Ep::Next<M>> {
        let board = self.board.borrow_mut(token);
        <M::Flag as MoveFlag>::make(board, m.from(), m.to(), T::COLOR);
        Pos {
            _turn: PhantomData,
            _castling: PhantomData,
            _ep_square: PhantomData,
            board: self.board,
        }
    }

    fn unmake_move<M: Move>(&self, m: M, token: &mut GhostToken<'id>) {
        let board = self.board.borrow_mut(token);
        // `T` is the side to move *after* the move was made, so the mover is
        // the previous side.
        <M::Flag as MoveFlag>::unmake(board, m.from(), m.to(), T::Prev::COLOR);
    }
}

trait MoveVisitor<'id, B> {
    fn invoke<M: Move>(&mut self, acc: B, m: M, token: &mut GhostToken<'id>) -> B;
}

pub fn perft<'id, T: Turn, C: Castling, Ep: EpSquare>(
    pos: Pos<'id, T, C, Ep>,
    token: &mut GhostToken<'id>,
    depth: u8,
) -> u64 {
    if depth == 0 {
        return 1;
    }

    struct Visitor<'id, T, C, Ep> {
        pos: Pos<'id, T, C, Ep>,
        depth: u8,
    }

    impl<'id, T: Turn, C: Castling, Ep: EpSquare> MoveVisitor<'id, u64> for Visitor<'id, T, C, Ep> {
        fn invoke<M: Move>(&mut self, count: u64, m: M, token: &mut GhostToken<'id>) -> u64 {
            let next = self.pos.make_move(m, token);
            let count = count + perft(next, token, self.depth - 1);
            next.unmake_move(m, token);
            count
        }
    }

    fold_moves(pos, 0, Visitor { pos, depth }, token)
}

fn fold_moves<'id, T: Turn, C: Castling, Ep: EpSquare, B, V>(
    pos: Pos<'id, T, C, Ep>,
    init: B,
    mut visitor: V,
    token: &mut GhostToken<'id>,
) -> B
where
    V: MoveVisitor<'id, B>,
{
    struct Snapshot {
        occ: Bitboard,
        empty: Bitboard,
        them: Bitboard,
        pawns: Bitboard,
        knights: Bitboard,
        bishops: Bitboard,
        rooks: Bitboard,
        queens: Bitboard,
        king_sq: Square,
        ep_capture: Option<Square>,
    }

    let us = T::COLOR;

    // Snapshot the board (all `Copy`), then release the immutable borrow so the
    // visitor can take the token mutably below.
    let s = {
        let board = pos.board.borrow(&*token);
        let occ = board.pieces.get_occupancy();
        Snapshot {
            occ,
            empty: !occ,
            them: board.pieces.get_color_bb(!us),
            pawns: board.pieces.get_bitboard(piece_type::PAWN, us),
            knights: board.pieces.get_bitboard(piece_type::KNIGHT, us),
            bishops: board.pieces.get_bitboard(piece_type::BISHOP, us),
            rooks: board.pieces.get_bitboard(piece_type::ROOK, us),
            queens: board.pieces.get_bitboard(piece_type::QUEEN, us),
            king_sq: board.pieces.get_bitboard(piece_type::KING, us).lsb().unwrap(),
            ep_capture: board.ep_square,
        }
    };

    let mut acc = init;
    let step = pawn_utils::single_step(us);
    let promo_bb = Bitboard::from(pawn_utils::promo_rank(us));
    let start_bb = Bitboard::from(pawn_utils::start_rank(us));

    macro_rules! emit {
        ($flag:ty, $from:expr, $to:expr) => {
            acc = visitor.invoke(acc, MoveValue::<$flag> { from: $from, to: $to, _flag: PhantomData }, token);
        };
    }

    // Knights.
    let mut bb = s.knights;
    while let Some(from) = bb.pop_lsb() {
        let targets = knight::lookup_attacks(from);
        let mut q = targets & s.empty;
        while let Some(to) = q.pop_lsb() { emit!(move_flags::Quiet, from, to); }
        let mut c = targets & s.them;
        while let Some(to) = c.pop_lsb() { emit!(move_flags::Capture, from, to); }
    }

    // King (non-castling).
    {
        let targets = king::lookup_attacks(s.king_sq);
        let mut q = targets & s.empty;
        while let Some(to) = q.pop_lsb() { emit!(move_flags::Quiet, s.king_sq, to); }
        let mut c = targets & s.them;
        while let Some(to) = c.pop_lsb() { emit!(move_flags::Capture, s.king_sq, to); }
    }

    // Sliders: rook-like (rooks + queens).
    let mut bb = s.rooks | s.queens;
    while let Some(from) = bb.pop_lsb() {
        let targets = rook::Rook::lookup_attacks(from, s.occ);
        let mut q = targets & s.empty;
        while let Some(to) = q.pop_lsb() { emit!(move_flags::Quiet, from, to); }
        let mut c = targets & s.them;
        while let Some(to) = c.pop_lsb() { emit!(move_flags::Capture, from, to); }
    }

    // Sliders: bishop-like (bishops + queens).
    let mut bb = s.bishops | s.queens;
    while let Some(from) = bb.pop_lsb() {
        let targets = bishop::Bishop::lookup_attacks(from, s.occ);
        let mut q = targets & s.empty;
        while let Some(to) = q.pop_lsb() { emit!(move_flags::Quiet, from, to); }
        let mut c = targets & s.them;
        while let Some(to) = c.pop_lsb() { emit!(move_flags::Capture, from, to); }
    }

    // Pawns.
    let mut bb = s.pawns;
    while let Some(from) = bb.pop_lsb() {
        let from_bb = Bitboard::from(from);
        let promo = !(from_bb & promo_bb).is_empty();

        // Single push.
        let push = pawn_utils::forward(from_bb, step) & s.empty;
        if let Some(to) = push.lsb() {
            if promo {
                emit!(move_flags::PromotionKnight, from, to);
                emit!(move_flags::PromotionBishop, from, to);
                emit!(move_flags::PromotionRook, from, to);
                emit!(move_flags::PromotionQueen, from, to);
            } else {
                emit!(move_flags::Quiet, from, to);
            }
        }

        // Double push.
        if !(from_bb & start_bb).is_empty() {
            let dbl = pawn_utils::forward(push, step) & s.empty;
            if let Some(to) = dbl.lsb() {
                emit!(move_flags::DoublePawnPush, from, to);
            }
        }

        // Captures.
        let mut caps = pawn::lookup_attacks(from, us) & s.them;
        while let Some(to) = caps.pop_lsb() {
            if promo {
                emit!(move_flags::CapturePromotionKnight, from, to);
                emit!(move_flags::CapturePromotionBishop, from, to);
                emit!(move_flags::CapturePromotionRook, from, to);
                emit!(move_flags::CapturePromotionQueen, from, to);
            } else {
                emit!(move_flags::Capture, from, to);
            }
        }

        // En passant.
        if let Some(cap) = s.ep_capture {
            if !(pawn::lookup_attacks(from, us) & Bitboard::from(cap)).is_empty() {
                let tgt = pawn_utils::forward(Bitboard::from(cap), step).lsb().unwrap();
                emit!(move_flags::EnPassant, from, tgt);
            }
        }
    }

    // Castling.
    let rank = castling_utils::castling_rank(us);
    if C::RIGHTS & (0b1000u8 >> (us.v() * 2)) != 0 {
        let path = Bitboard::between(s.king_sq, Square::from((files::H, rank)));
        if (path & s.occ).is_empty() {
            emit!(move_flags::KingCastle, s.king_sq, Square::from((files::G, rank)));
        }
    }
    if C::RIGHTS & (0b0100u8 >> (us.v() * 2)) != 0 {
        let path = Bitboard::between(s.king_sq, Square::from((files::A, rank)));
        if (path & s.occ).is_empty() {
            emit!(move_flags::QueenCastle, s.king_sq, Square::from((files::C, rank)));
        }
    }

    acc
}
