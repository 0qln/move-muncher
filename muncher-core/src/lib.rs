#![allow(unused_labels)]
#![allow(non_camel_case_types)]

use engine::core::coordinates::Square;
use ghost_cell::{GhostCell, GhostToken};
use std::marker::PhantomData;

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

pub trait Move: Copy {
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

pub struct Pos<'id, Turn, Castling, EpSquare> {
    _turn: PhantomData<Turn>,
    _castling: PhantomData<Castling>,
    _ep_square: PhantomData<EpSquare>,

    // todo: replace with lean implementation
    board: &'id GhostCell<'id, engine::core::position::Position>,
}

impl<T, C, Ep> Clone for Pos<'_, T, C, Ep> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, C, Ep> Copy for Pos<'_, T, C, Ep> {}

impl<'id, T: Turn, C: Castling, Ep: EpSquare> Pos<'id, T, C, Ep> {
    fn make_move<M: Move>(
        &self,
        m: M,
        token: &mut GhostToken<'id>,
    ) -> Pos<'id, T::Next, C::Next<M, T>, Ep::Next<M>> {
        // TODO: bridge the type-level `M` to a concrete `engine::core::move::Move`,
        // then `self.board.borrow_mut(token).make_move(engine_move, &mut ());`
        todo!()
    }

    fn unmake_move<M: Move>(&self, m: M, token: &mut GhostToken<'id>) {
        // TODO: `self.board.borrow_mut(token).unmake_move(engine_move, &mut ());`
        todo!()
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
    todo!()
}
