#![allow(unused_labels)]

use std::{marker::PhantomData, mem::MaybeUninit};
    use engine::core::coordinates::Square;

pub trait Position {
    fn make_move<M: Move>(self, m: M);
    fn unmake_move<M: Move>(self, m: M);
}

pub trait Turn {
    type Next: Turn;

    /// `C` with this color's king-side castling right cleared.
    type TakeKing<C: Castling>: Castling;

    /// `C` with this color's queen-side castling right cleared.
    type TakeQueen<C: Castling>: Castling;
}
mod turns {
    use super::*;

    struct White;
    struct Black;

    impl Turn for White {
        type Next = Black;
        type TakeKing<C: Castling> = C::Take_1000;
        type TakeQueen<C: Castling> = C::Take_0100;
    }

    impl Turn for Black {
        type Next = White;
        type TakeKing<C: Castling> = C::Take_0010;
        type TakeQueen<C: Castling> = C::Take_0001;
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

    type Take_Q<T: Turn>: Castling;
    type Take_K<T: Turn>: Castling;

    /// The next castling state after a move whose flag is `M::Flag` is played.
    type Next<M: Move, T: Turn>: Castling;
}

mod castling {
    use super::*;

    macro_rules! impl_castling_states {
        ($( ($st:ident, $t1000:ident, $t0100:ident, $t0010:ident, $t0001:ident) ),* $(,)?) => {
            $(
                pub struct $st;
                impl Castling for $st {
                    type Take_1000 = $t1000;
                    type Take_0100 = $t0100;
                    type Take_0010 = $t0010;
                    type Take_0001 = $t0001;
                    type Take_K<T: Turn> = <T as Turn>::TakeKing<Self>;
                    type Take_Q<T: Turn> = <T as Turn>::TakeQueen<Self>;
                    type Next<M: Move, T: Turn> = <M::Flag as MoveFlag>::Apply<Self, T>;
                }
            )*
        };
    }

    impl_castling_states! {
        (_0000, _0000, _0000, _0000, _0000),
        (_0001, _0001, _0001, _0001, _0000),
        (_0010, _0010, _0010, _0000, _0010),
        (_0011, _0011, _0011, _0001, _0010),
        (_0100, _0100, _0000, _0100, _0100),
        (_0101, _0101, _0001, _0101, _0100),
        (_0110, _0110, _0010, _0100, _0110),
        (_0111, _0111, _0011, _0101, _0110),
        (_1000, _0000, _1000, _1000, _1000),
        (_1001, _0001, _1001, _1001, _1000),
        (_1010, _0010, _1010, _1000, _1010),
        (_1011, _0011, _1011, _1001, _1010),
        (_1100, _0100, _1000, _1100, _1100),
        (_1101, _0101, _1001, _1101, _1100),
        (_1110, _0110, _1010, _1100, _1110),
        (_1111, _0111, _1011, _1101, _1110),
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

pub trait Move {
    type Flag: MoveFlag;
}

pub trait MoveFlag {
    /// The castling state after a move carrying this flag is played.
    type Apply<C: Castling, T: Turn>: Castling;
}
mod move_flags {
    use super::*;

    struct Quiet;
    struct DoublePawnPush;
    struct PromotionKnight;
    struct PromotionBishop;
    struct PromotionRook;
    struct PromotionQueen;
    struct CapturePromotionKnight;
    struct CapturePromotionBishop;
    struct CapturePromotionRook;
    struct CapturePromotionQueen;
    struct KingCastle;
    struct QueenCastle;
    struct Capture;
    struct EnPassant;

    macro_rules! keep_castling {
        ($($flag:ident),* $(,)?) => {
            $(
                impl MoveFlag for $flag {
                    type Apply<C: Castling, T: Turn> = C;
                }
            )*
        };
    }

    keep_castling! {
        Quiet,
        DoublePawnPush,
        PromotionKnight,
        PromotionBishop,
        PromotionRook,
        PromotionQueen,
        CapturePromotionKnight,
        CapturePromotionBishop,
        CapturePromotionRook,
        CapturePromotionQueen,
        Capture,
        EnPassant,
    }

    impl MoveFlag for KingCastle {
        type Apply<C: Castling, T: Turn> = <<C as Castling>::Take_K<T> as Castling>::Take_Q<T>;
    }

    impl MoveFlag for QueenCastle {
        type Apply<C: Castling, T: Turn> = <<C as Castling>::Take_K<T> as Castling>::Take_Q<T>;
    }
}

pub struct Pos<Turn, Castling, EpSquare> {
    _turn: PhantomData<Turn>,
    _castling: PhantomData<Castling>,
    _ep_square: PhantomData<EpSquare>,

    runtime: engine::core::position::Position,
}

impl<T: Turn, C: Castling, Ep: EpSquare> Position for Pos<T, C, Ep> {
    fn make_move<M: Move>(mut self, m: M) -> Pos<T::Next, C::Next<M, T>, Ep::Next<M>> {
        todo!()
    }

    fn unmake_move<M: Move>(mut self, m: M) {
        todo!()
    }
}

pub fn count_positions(pos: impl Position, max_depth: u8) -> u64 {
    struct StackInfo {
        moves: [Move; 256],
    }

    let mut count = 1;
    let mut pos = pos;
    let mut stack = [const { MaybeUninit::<StackInfo>::uninit() }; 2 << 8];

    'depth: for depth in 0..max_depth {
        let si = unsafe { stack[depth].assume_init_mut() };
        fill_moves(&mut si.moves);

        'moves: for &m in &si.moves {
            let pos = pos.make_move<>(m);
            count += 1;
        }
    }

    count
}

fn fill_moves(moves: &mut [Move; 256]) {
    for m in moves.iter_mut() {
        *m = Move;
    }
}
