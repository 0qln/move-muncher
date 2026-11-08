#![allow(unused_labels)]
#![allow(non_camel_case_types)]

use engine::core::{
    bitboard::Bitboard,
    color::{Color, colors},
    coordinates::{Square, castling as castling_utils, files, pawn_utils},
    move_iter::{
        bishop, king, knight, pawn, rook,
        sliding_piece::{SlidingAttacks, magics},
    },
    piece::{Piece, piece_type},
    position::PieceInfo,
};
use ghost_cell::{GhostCell, GhostToken};
use std::marker::PhantomData;

pub fn init() {
    magics::init();
}

pub trait Turn {
    type Next: Turn;
    type Prev: Turn;

    const COLOR: Color;

    type Castling_TakeK<C: Castling>: Castling;
    type Castling_TakeQ<C: Castling>: Castling;
}

pub mod turns {
    use super::*;

    pub struct White;
    pub struct Black;

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

/// KING_WHITE=0b1000, QUEEN_WHITE=0b0100, KING_BLACK=0b0010, QUEEN_BLACK=0b0001.
#[allow(non_camel_case_types)]
pub trait Castling {
    type Take_1000: Castling;
    type Take_0100: Castling;
    type Take_0010: Castling;
    type Take_0001: Castling;

    const RIGHTS: u8;

    type Take_Q<T: Turn>: Castling;
    type Take_K<T: Turn>: Castling;

    /// The next castling state after a move M is played.
    type Next<M: Move, T: Turn>: Castling;
}

pub mod castling {
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
                    type Next<M: Move, T: Turn> = <M::Flag as MoveFlag>::ApplyCastling<Self, T>;
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
pub mod ep_squares {
    pub struct None;
    pub struct Some;
}
impl EpSquare for ep_squares::None {
    type Next<M: Move> = <M::Flag as MoveFlag>::ApplyEp;
}
impl EpSquare for ep_squares::Some {
    type Next<M: Move> = <M::Flag as MoveFlag>::ApplyEp;
}

pub trait Move: Copy {
    type Flag: MoveFlag;

    fn from(&self) -> Square;
    fn to(&self) -> Square;
}

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
    castling: u8,
    history: Vec<Undo>,
}

struct Undo {
    captured: Option<(Square, Piece)>,
    prev_ep: Option<Square>,
    prev_castling: u8,
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

    fn begin_move(&mut self, captured: Option<(Square, Piece)>, from: Square, us: Color) {
        let prev_ep = self.ep_square;
        let prev_castling = self.castling;
        self.ep_square = None;

        // A king or rook moving off its home square forfeits that side's
        // castling right, as does capturing an enemy rook on its home square.
        let rank = castling_utils::castling_rank(us);
        if from == Square::from((files::E, rank)) {
            self.castling &= !(0b1100u8 >> (us.v() * 2));
        } else if from == Square::from((files::A, rank)) {
            self.castling &= !(0b0100u8 >> (us.v() * 2));
        } else if from == Square::from((files::H, rank)) {
            self.castling &= !(0b1000u8 >> (us.v() * 2));
        }
        if let Some((cap_sq, _)) = captured {
            let them = !us;
            let their_rank = castling_utils::castling_rank(them);
            if cap_sq == Square::from((files::A, their_rank)) {
                self.castling &= !(0b0100u8 >> (them.v() * 2));
            } else if cap_sq == Square::from((files::H, their_rank)) {
                self.castling &= !(0b1000u8 >> (them.v() * 2));
            }
        }

        self.history.push(Undo { captured, prev_ep, prev_castling });
    }

    fn end_move(&mut self) -> Option<(Square, Piece)> {
        let undo = self.history.pop().unwrap();
        self.ep_square = undo.prev_ep;
        self.castling = undo.prev_castling;
        undo.captured
    }

    /// Build a board from the piece-placement field of a FEN (turn, castling and
    /// en-passant are supplied as type parameters on `Pos`, not stored here).
    pub fn from_fen(piece_placement: &str) -> Self {
        let mut board = Board::default();
        let mut sq: i32 = 56; // a8
        for c in piece_placement.chars() {
            match c {
                '/' => sq -= 16,
                '1'..='8' => sq += c as i32 - '0' as i32,
                'p' | 'n' | 'b' | 'r' | 'q' | 'k' | 'P' | 'N' | 'B' | 'R' | 'Q' | 'K' => {
                    let color = if c.is_ascii_uppercase() { colors::WHITE } else { colors::BLACK };
                    let pt = match c.to_ascii_lowercase() {
                        'p' => piece_type::PAWN,
                        'n' => piece_type::KNIGHT,
                        'b' => piece_type::BISHOP,
                        'r' => piece_type::ROOK,
                        'q' => piece_type::QUEEN,
                        _ => piece_type::KING,
                    };
                    // Safety: `sq` is kept within 0..64.
                    board
                        .pieces
                        .put_piece(unsafe { Square::from_v(sq as u8) }, Piece::from((color, pt)), &mut ());
                    sq += 1;
                }
                ' ' => break,
                _ => {}
            }
        }
        board
    }

    pub fn initial() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR")
    }

    pub fn to_fen(&self) -> String {
        let mut out = String::new();
        for rank in (0..8i32).rev() {
            let mut empty = 0;
            for file in 0..8i32 {
                let sq = unsafe { Square::from_v((rank * 8 + file) as u8) };
                let piece = self.pieces.get_piece(sq);
                if piece == Piece::default() {
                    empty += 1;
                } else {
                    if empty > 0 {
                        out.push_str(&empty.to_string());
                        empty = 0;
                    }
                    let c = match piece.piece_type() {
                        piece_type::PAWN => 'p',
                        piece_type::KNIGHT => 'n',
                        piece_type::BISHOP => 'b',
                        piece_type::ROOK => 'r',
                        piece_type::QUEEN => 'q',
                        piece_type::KING => 'k',
                        _ => '?',
                    };
                    out.push(if piece.color() == colors::WHITE { c.to_ascii_uppercase() } else { c });
                }
            }
            if empty > 0 {
                out.push_str(&empty.to_string());
            }
            if rank > 0 {
                out.push('/');
            }
        }
        out
    }
}

impl Default for Board {
    fn default() -> Self {
        Board {
            pieces: PieceInfo::default(),
            ep_square: None,
            castling: 0,
            history: Vec::new(),
        }
    }
}

pub trait MoveFlag {
    /// The castling state after a move carrying this flag is played.
    type ApplyCastling<C: Castling, T: Turn>: Castling;

    /// The en-passant state after this move.
    type ApplyEp: EpSquare;

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
        type ApplyCastling<C: Castling, T: Turn> = C;
        type ApplyEp = ep_squares::None;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            board.begin_move(None, from, us);
            board.move_piece(from, to);
        }
        fn unmake(board: &mut Board, from: Square, to: Square, _us: Color) {
            board.move_piece(to, from);
            board.end_move();
        }
    }

    impl MoveFlag for DoublePawnPush {
        type ApplyCastling<C: Castling, T: Turn> = C;
        type ApplyEp = ep_squares::Some;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            board.begin_move(None, from, us);
            board.move_piece(from, to);
            // The capture square is the doubled pawn's destination square.
            board.ep_square = Some(to);
        }
        fn unmake(board: &mut Board, from: Square, to: Square, _us: Color) {
            board.move_piece(to, from);
            board.end_move();
        }
    }

    impl MoveFlag for Capture {
        type ApplyCastling<C: Castling, T: Turn> = C;
        type ApplyEp = ep_squares::None;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            let captured = board.remove_piece(to);
            board.begin_move(Some((to, captured)), from, us);
            board.move_piece(from, to);
        }
        fn unmake(board: &mut Board, from: Square, to: Square, _us: Color) {
            board.move_piece(to, from);
            let (sq, cap) = board.end_move().unwrap();
            board.put_piece(sq, cap);
        }
    }

    impl MoveFlag for EnPassant {
        type ApplyCastling<C: Castling, T: Turn> = C;
        type ApplyEp = ep_squares::None;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            let cap_sq = pawn_utils::backward(Bitboard::from(to), pawn_utils::single_step(us)).lsb().unwrap();
            let captured = board.remove_piece(cap_sq);
            board.begin_move(Some((cap_sq, captured)), from, us);
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
                type ApplyCastling<C: Castling, T: Turn> = C;
                type ApplyEp = ep_squares::None;
                fn make(board: &mut Board, from: Square, to: Square, us: Color) {
                    board.begin_move(None, from, us);
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
                type ApplyCastling<C: Castling, T: Turn> = C;
                type ApplyEp = ep_squares::None;
                fn make(board: &mut Board, from: Square, to: Square, us: Color) {
                    let captured = board.remove_piece(to);
                    board.begin_move(Some((to, captured)), from, us);
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
        type ApplyCastling<C: Castling, T: Turn> = <<C as Castling>::Take_K<T> as Castling>::Take_Q<T>;
        type ApplyEp = ep_squares::None;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            let rank = castling_utils::castling_rank(us);
            board.begin_move(None, from, us);
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
        type ApplyCastling<C: Castling, T: Turn> = <<C as Castling>::Take_K<T> as Castling>::Take_Q<T>;
        type ApplyEp = ep_squares::None;
        fn make(board: &mut Board, from: Square, to: Square, us: Color) {
            let rank = castling_utils::castling_rank(us);
            board.begin_move(None, from, us);
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

pub struct Pos<'a, 'id, Turn, Castling, EpSquare> {
    _turn: PhantomData<Turn>,
    _castling: PhantomData<Castling>,
    _ep_square: PhantomData<EpSquare>,

    board: &'a GhostCell<'id, Board>,
}

impl<T, C, Ep> Clone for Pos<'_, '_, T, C, Ep> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, C, Ep> Copy for Pos<'_, '_, T, C, Ep> {}

impl<'a, 'id, T, C, Ep> Pos<'a, 'id, T, C, Ep> {
    pub fn new(board: &'a GhostCell<'id, Board>) -> Self {
        Pos {
            _turn: PhantomData,
            _castling: PhantomData,
            _ep_square: PhantomData,
            board,
        }
    }
}

impl<'a, 'id, T: Turn, C: Castling, Ep: EpSquare> Pos<'a, 'id, T, C, Ep> {
    fn make_move<M: Move>(&self, m: M, token: &mut GhostToken<'id>) -> Pos<'a, 'id, T::Next, C::Next<M, T>, Ep::Next<M>> {
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
        // `T` is the side to move after the move was made, so the mover is the previous side.
        <M::Flag as MoveFlag>::unmake(board, m.from(), m.to(), T::Prev::COLOR);
    }
}

trait MoveVisitor<'id, B> {
    fn invoke<M: Move>(&mut self, acc: B, m: M, token: &mut GhostToken<'id>) -> B;
}

pub fn perft<'a, 'id, T: Turn, C: Castling, Ep: EpSquare>(pos: Pos<'a, 'id, T, C, Ep>, token: &mut GhostToken<'id>, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }

    struct Counter<'a, 'id, T, C, Ep> {
        pos: Pos<'a, 'id, T, C, Ep>,
        depth: u8,
    }

    impl<'a, 'id, T: Turn, C: Castling, Ep: EpSquare> MoveVisitor<'id, u64> for Counter<'a, 'id, T, C, Ep> {
        fn invoke<M: Move>(&mut self, count: u64, m: M, token: &mut GhostToken<'id>) -> u64 {
            let next = self.pos.make_move(m, token);
            let count = count + perft(next, token, self.depth - 1);
            next.unmake_move(m, token);
            count
        }
    }

    fold_moves(pos, 0, Counter { pos, depth }, token)
}

/// Piece bitboards for `color`, in order PAWN, KNIGHT, BISHOP, ROOK, QUEEN, KING.
fn piece_array(pieces: &PieceInfo, color: Color) -> [Bitboard; 6] {
    [
        pieces.get_bitboard(piece_type::PAWN, color),
        pieces.get_bitboard(piece_type::KNIGHT, color),
        pieces.get_bitboard(piece_type::BISHOP, color),
        pieces.get_bitboard(piece_type::ROOK, color),
        pieces.get_bitboard(piece_type::QUEEN, color),
        pieces.get_bitboard(piece_type::KING, color),
    ]
}

/// Pieces of `by` that attack `to`, given the occupancy `occ`.
fn attackers_to(to: Square, occ: Bitboard, by: Color, p: &[Bitboard; 6]) -> Bitboard {
    (rook::Rook::lookup_attacks(to, occ) & (p[3] | p[4]))
        | (bishop::Bishop::lookup_attacks(to, occ) & (p[2] | p[4]))
        | (knight::lookup_attacks(to) & p[1])
        | (king::lookup_attacks(to) & p[5])
        | (pawn::lookup_attacks(to, !by) & p[0])
}

/// If `sq` is pinned against our king, restrict it to the pin ray; otherwise allow
/// everything.
fn pin_mask(sq: Square, blockers: Bitboard, king_sq: Square) -> Bitboard {
    if (Bitboard::from(sq) & blockers).is_empty() {
        Bitboard::full()
    } else {
        Bitboard::ray(sq, king_sq)
    }
}

fn fold_moves<'a, 'id, T: Turn, C: Castling, Ep: EpSquare, B, V>(
    pos: Pos<'a, 'id, T, C, Ep>,
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
        us_bb: Bitboard,
        them_bb: Bitboard,
        us: [Bitboard; 6],
        enemy: [Bitboard; 6],
        king_sq: Square,
        checkers: Bitboard,
        blockers: Bitboard,
        ep_capture: Option<Square>,
        castling: u8,
    }

    let us = T::COLOR;

    // Read the board's Copy state once. GhostCell gives single-owner access, so
    // we can't hold `&Board` while the visitor below takes the token mutably;
    // this copies ~a dozen `u64` bitboards, not the board itself.
    let s = {
        let board = pos.board.borrow(token);
        let occ = board.pieces.get_occupancy();
        let us_pieces = piece_array(&board.pieces, us);
        let enemy = piece_array(&board.pieces, !us);

        let king_sq = us_pieces[5]
            .lsb()
            .unwrap_or_else(|| panic!("no king for side to move ({us}); FEN: {}", board.to_fen()));
        let checkers = attackers_to(king_sq, occ, !us, &enemy);

        // X-ray blockers (pinned pieces).
        let x_ray = (rook::lookup_attacks_0_occ(king_sq) & (enemy[3] | enemy[4])) | (bishop::lookup_attacks_0_occ(king_sq) & (enemy[2] | enemy[4]));
        let mut blockers = Bitboard::empty();
        let mut xb = x_ray;
        while let Some(x) = xb.pop_lsb() {
            let between = Bitboard::between(x, king_sq);
            if (occ & between).pop_cnt() == 1 {
                blockers |= between;
            }
        }

        Snapshot {
            occ,
            empty: !occ,
            us_bb: board.pieces.get_color_bb(us),
            them_bb: board.pieces.get_color_bb(!us),
            us: us_pieces,
            enemy,
            king_sq,
            checkers,
            blockers,
            ep_capture: board.ep_square,
            castling: board.castling,
        }
    };

    let n_checkers = s.checkers.pop_cnt();

    // Check-resolution target masks: which squares may a quiet/capture move land on.
    let (quiet_mask, capture_mask) = match n_checkers {
        0 => (s.empty, s.them_bb),
        1 => {
            let checker = s.checkers.lsb().unwrap();
            (Bitboard::between(s.king_sq, checker), s.checkers)
        }
        _ => (Bitboard::empty(), Bitboard::empty()),
    };

    let mut acc = init;
    let step = pawn_utils::single_step(us);
    let promo_bb = Bitboard::from(pawn_utils::promo_rank(us));
    let start_bb = Bitboard::from(pawn_utils::start_rank(us));

    macro_rules! emit {
        ($flag:ty, $from:expr, $to:expr) => {
            acc = visitor.invoke(
                acc,
                MoveValue::<$flag> {
                    from: $from,
                    to: $to,
                    _flag: PhantomData,
                },
                token,
            );
        };
    }

    // Non-king pieces (no moves resolve a double check).
    if n_checkers <= 1 {
        // Knights.
        let knights = s.us[1];
        for from in knights {
            let mask = pin_mask(from, s.blockers, s.king_sq);
            let targets = knight::lookup_attacks(from) & mask;

            let quiets = targets & quiet_mask;
            for to in quiets {
                emit!(move_flags::Quiet, from, to);
            }

            let captures = targets & capture_mask;
            for to in captures {
                emit!(move_flags::Capture, from, to);
            }
        }

        // Rook-like sliders (rooks + queens).
        // TODO: deduplicate the pin_mask computation for the queens
        let r_n_q = s.us[3] | s.us[4];
        for from in r_n_q {
            let mask = pin_mask(from, s.blockers, s.king_sq);
            let targets = rook::Rook::lookup_attacks(from, s.occ) & mask;

            let quiets = targets & quiet_mask;
            for to in quiets {
                emit!(move_flags::Quiet, from, to);
            }

            let captures = targets & capture_mask;
            for to in captures {
                emit!(move_flags::Capture, from, to);
            }
        }

        // Bishop-like sliders (bishops + queens).
        let b_n_q = s.us[2] | s.us[4];
        for from in b_n_q {
            let mask = pin_mask(from, s.blockers, s.king_sq);
            let targets = bishop::Bishop::lookup_attacks(from, s.occ) & mask;

            let quiets = targets & quiet_mask;
            for to in quiets {
                emit!(move_flags::Quiet, from, to);
            }

            let captures = targets & capture_mask;
            for to in captures {
                emit!(move_flags::Capture, from, to);
            }
        }

        // Pawns.
        // TODO: move all pawns at once
        let pawns = s.us[0];
        for from in pawns {
            let from_bb = Bitboard::from(from);
            let mask = pin_mask(from, s.blockers, s.king_sq);
            let promo = !(from_bb & promo_bb).is_empty();

            // Single push.
            let push = pawn_utils::forward(from_bb, step) & quiet_mask & mask;
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

            // Double push: the middle square just needs to be empty; the
            // destination must satisfy the quiet/block mask (and not be pinned).
            if !(from_bb & start_bb).is_empty() {
                let middle = pawn_utils::forward(from_bb, step);
                if (middle & s.occ).is_empty() {
                    let dbl = pawn_utils::forward(middle, step) & quiet_mask & mask;
                    if let Some(to) = dbl.lsb() {
                        emit!(move_flags::DoublePawnPush, from, to);
                    }
                }
            }

            // Captures.
            let mut caps = pawn::lookup_attacks(from, us) & capture_mask & mask;
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

            // En passant (with the ep-pin legality check). `cap` is the capture
            // square (the enemy pawn); `tgt` is where our pawn lands.
            if let Some(cap) = s.ep_capture {
                let tgt = pawn_utils::forward(Bitboard::from(cap), step).lsb().unwrap();
                if !(pawn::lookup_attacks(from, us) & Bitboard::from(tgt)).is_empty() {
                    let legal = {
                        let board = pos.board.borrow_mut(token);
                        let captured = board.remove_piece(cap);
                        board.move_piece(from, tgt);
                        let occ_after = board.pieces.get_occupancy();
                        let enemy_after = piece_array(&board.pieces, !us);
                        let legal = attackers_to(s.king_sq, occ_after, !us, &enemy_after).is_empty();
                        board.move_piece(tgt, from);
                        board.put_piece(cap, captured);
                        legal
                    };
                    if legal {
                        emit!(move_flags::EnPassant, from, tgt);
                    }
                }
            }
        }
    }

    // King moves
    {
        let occ_no_king = s.occ & !Bitboard::from(s.king_sq);
        let targets = king::lookup_attacks(s.king_sq) & !s.us_bb;
        let quiets = targets & s.empty;
        for to in quiets {
            if attackers_to(to, occ_no_king, !us, &s.enemy).is_empty() {
                emit!(move_flags::Quiet, s.king_sq, to);
            }
        }
        let captures = targets & s.them_bb;
        for to in captures {
            if attackers_to(to, occ_no_king, !us, &s.enemy).is_empty() {
                emit!(move_flags::Capture, s.king_sq, to);
            }
        }
    }

    // Castling (only out of check, through unattacked, empty squares). Rights
    // are tracked at runtime in `Board::castling` and cleared when a king or
    // rook moves (or an enemy rook is captured on its home square); the home-
    // square checks below are a defensive extra.
    if n_checkers == 0 {
        let rank = castling_utils::castling_rank(us);
        let home_king = Square::from((files::E, rank));
        let occ_no_king = s.occ & !Bitboard::from(s.king_sq);
        if s.castling & (0b1000u8 >> (us.v() * 2)) != 0
            && s.king_sq == home_king
            && !(s.us[3] & Bitboard::from(Square::from((files::H, rank)))).is_empty()
        {
            let f = Square::from((files::F, rank));
            let g = Square::from((files::G, rank));
            let path = Bitboard::from(f) | Bitboard::from(g);
            if (path & s.occ).is_empty()
                && attackers_to(f, occ_no_king, !us, &s.enemy).is_empty()
                && attackers_to(g, occ_no_king, !us, &s.enemy).is_empty()
            {
                emit!(move_flags::KingCastle, s.king_sq, g);
            }
        }
        if s.castling & (0b0100u8 >> (us.v() * 2)) != 0
            && s.king_sq == home_king
            && !(s.us[3] & Bitboard::from(Square::from((files::A, rank)))).is_empty()
        {
            let d = Square::from((files::D, rank));
            let c = Square::from((files::C, rank));
            let b = Square::from((files::B, rank));
            let path = Bitboard::from(d) | Bitboard::from(c) | Bitboard::from(b);
            if (path & s.occ).is_empty()
                && attackers_to(d, occ_no_king, !us, &s.enemy).is_empty()
                && attackers_to(c, occ_no_king, !us, &s.enemy).is_empty()
            {
                emit!(move_flags::QueenCastle, s.king_sq, c);
            }
        }
    }

    acc
}

/// Runtime side to move, used only to parse a FEN; the type-level `Turn` carries
/// the actual side through `perft`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    White,
    Black,
}

fn parse_square(s: &str) -> Square {
    let b = s.as_bytes();
    let file = (b[0] - b'a') as u8;
    let rank = (b[1] - b'1') as u8;
    // Safety: `file` and `rank` are each in 0..8, so the index is in 0..64.
    unsafe { Square::from_v(rank * 8 + file) }
}

/// Parse a FEN into a board, the side to move, and the castling-rights bitmask.
pub fn parse_fen(fen: &str) -> (Board, Side, u8) {
    let mut board = Board::from_fen(fen); // parses piece placement, stops at the first space

    let mut fields = fen.split_whitespace();
    let _ = fields.next(); // pieces (already parsed by `Board::from_fen`)
    let turn = fields.next().unwrap_or("w");
    let castling = fields.next().unwrap_or("-");
    let ep = fields.next().unwrap_or("-");

    let side = if turn == "w" { Side::White } else { Side::Black };

    let mut mask = 0u8;
    for c in castling.chars() {
        mask |= match c {
            'K' => 0b1000,
            'Q' => 0b0100,
            'k' => 0b0010,
            'q' => 0b0001,
            _ => 0,
        };
    }

    if ep != "-" {
        // FEN records the ep *target*; we store the *capture* square, one step
        // beyond the target in the opponent's direction.
        let target = parse_square(ep);
        let opp_step = if side == Side::White {
            pawn_utils::single_step(colors::BLACK)
        } else {
            pawn_utils::single_step(colors::WHITE)
        };
        let capture = pawn_utils::forward(Bitboard::from(target), opp_step).lsb().unwrap();
        board.ep_square = Some(capture);
    }

    board.castling = mask;

    (board, side, mask)
}

macro_rules! perft_fen_dispatch {
    ($cell:expr, $token:expr, $depth:expr, $side:expr, $castling:expr) => {
        perft_fen_dispatch!(@arm $cell, $token, $depth, $side, $castling;
            (_0000, 0b0000), (_0001, 0b0001), (_0010, 0b0010), (_0011, 0b0011),
            (_0100, 0b0100), (_0101, 0b0101), (_0110, 0b0110), (_0111, 0b0111),
            (_1000, 0b1000), (_1001, 0b1001), (_1010, 0b1010), (_1011, 0b1011),
            (_1100, 0b1100), (_1101, 0b1101), (_1110, 0b1110), (_1111, 0b1111))
    };
    (@arm $cell:expr, $token:expr, $depth:expr, $side:expr, $castling:expr; $(($st:ident, $mask:literal)),* $(,)?) => {{
        match ($side, $castling) {
            $(
                (Side::White, $mask) => perft(Pos::<turns::White, castling::$st, ep_squares::None>::new($cell), $token, $depth),
                (Side::Black, $mask) => perft(Pos::<turns::Black, castling::$st, ep_squares::None>::new($cell), $token, $depth),
            )*
            _ => unreachable!("invalid castling mask"),
        }
    }};
}

/// Run perft from an arbitrary FEN string.
pub fn perft_fen(fen: &str, depth: u8) -> u64 {
    let (board, side, castling) = parse_fen(fen);
    GhostToken::new(|mut token| {
        let cell = GhostCell::new(board);
        perft_fen_dispatch!(&cell, &mut token, depth, side, castling)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_perft(fen: &str, depth: u8, expected: u64) {
        init();
        assert_eq!(
            perft_fen(fen, depth),
            expected,
            "perft({depth}) mismatch for {fen}"
        );
    }

    // The six standard perft positions (same set nephrid uses in
    // `engine/src/core/move_iter/test.rs`), with the canonical node counts.
    #[test]
    fn perft_start_position() {
        assert_perft("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 5, 4_865_609);
    }

    #[test]
    fn perft_kiwipete() {
        assert_perft("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 5, 193_690_690);
    }

    #[test]
    fn perft_position_3() {
        assert_perft("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 6, 11_030_083);
    }

    #[test]
    fn perft_position_4() {
        assert_perft("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1", 5, 15_833_292);
    }

    #[test]
    fn perft_position_5() {
        assert_perft("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", 5, 89_941_194);
    }

    #[test]
    fn perft_position_6() {
        assert_perft("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10", 5, 164_075_551);
    }
}
