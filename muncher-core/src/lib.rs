#![allow(unused_labels)]
#![allow(non_camel_case_types)]

use engine::core::coordinates::Square;
use std::{cell::UnsafeCell, marker::PhantomData};

pub trait Turn {
    type Next: Turn;
    type Prev: Turn;

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

        type Castling_TakeK<C: Castling> = C::Take_1000;
        type Castling_TakeQ<C: Castling> = C::Take_0100;
    }

    impl Turn for Black {
        type Next = White;
        type Prev = White;

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
                    type Take_K<T: Turn> = <T as Turn>::Castling_TakeK<Self>;
                    type Take_Q<T: Turn> = <T as Turn>::Castling_TakeQ<Self>;
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

    // todo: replace with lean implementation
    runtime: engine::core::position::Position,
}

impl<T: Turn, C: Castling, Ep: EpSquare> Pos<T, C, Ep> {
    fn make_move<M: Move>(mut self, m: M) -> Pos<T::Next, C::Next<M, T>, Ep::Next<M>> {
        todo!()
    }

    fn unmake_move<M: Move>(mut self, m: M) {
        todo!()
    }
}

trait FoldMove<B> {
    /// Fold a single move. `m` is generic so the body can name it at its own
    /// type level (e.g. pass it to `Pos::make_move`).
    fn fold<M: Move>(&mut self, acc: B, m: M) -> B;
}

pub fn perft<T: Turn, C: Castling, Ep: EpSquare>(pos: &UnsafeCell<Pos<T, C, Ep>>, depth: u8) -> u64 {
    struct Visitor<'a, T, C, Ep> {
        pos: &'a UnsafeCell<Pos<T, C, Ep>>,
        depth: u8,
    }

    impl<T: Turn, C: Castling, Ep: EpSquare> FoldMove<u64> for Visitor<'_, T, C, Ep> {
        fn fold<M: Move>(&mut self, count: u64, m: M) -> u64 {
            let pos = unsafe { self.pos.into_inner() };
            let pos = pos.make_move(m);
            let count = count + perft(pos, self.depth - 1);
            pos.unmake_move(m);
            count
        }
    }

    fold_moves(unsafe { &*pos.get() }, 1, Visitor { pos, depth })
}

fn fold_moves<T: Turn, C: Castling, Ep: EpSquare, B, V>(pos: &Pos<T, C, Ep>, init: B, mut visitor: V) -> B
where
    V: FoldMove<B>,
{
    todo!()
}
