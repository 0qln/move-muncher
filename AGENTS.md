# AGENTS.md

Guidance for AI agents working in this repository.

## What this is

`move-muncher` is a chess move generator optimized for raw perft speed. It pairs a
**type-level position-state machine** (turn, castling rights, en-passant
availability all live in `Pos`'s type parameters) with **bitboard move
generation**, and leans on **compile-time branch elimination** to drop whole
move classes when the position type says they're impossible.

Speed is the whole point. When in doubt, prefer the compile-time/type-level
solution over a runtime branch in the hot path.

## Layout

- `muncher/` — thin CLI (`cargo run -p muncher --release -- --depth N --fen "..."`).
- `muncher-core/` — the move generator (lib `muncher_core`), plus `benches/perft.rs`.
- `~/repos/nephrid` — the **reference engine**. `muncher-core` depends on its
  `engine` crate for primitives (`Bitboard`, `Square`, `PieceInfo`, magic
  bitboards, attack lookups) but has its own movegen + make/unmake. Treat nephrid
  as the correctness oracle and the benchmark reference.

## Core design

### Type-level position state

`Pos<'a, 'id, Turn, Castling, EpSquare>` — the side to move, castling rights and
en-passant availability are phantom type parameters, not runtime fields.

- `Turn` = `turns::White | turns::Black`
- `Castling` = 16 states (`castling::_0000 .. _1111`), one per rights bitmask
- `EpSquare` = `ep_squares::None | ep_squares::Some`

Only what the type can't encode is runtime. `Board` holds `pieces: PieceInfo`,
`ep_square: Option<Square>`, `castling: u8`, `history: Vec<Undo>`.

### Moves and make/unmake

A move is `MoveValue<F>` where `F: MoveFlag` is a ZST. Each flag
(`Quiet`, `Capture`, `DoublePawnPush`, `EnPassant`, `Promotion*`,
`CapturePromotion*`, `KingCastle`, `QueenCastle`) knows:

- how to `make`/`unmake` the board;
- how to transform the type-level state (`ApplyCastling<C, T>`, `ApplyEp`).

`Pos::make_move` returns `Pos<T::Next, C::Next<M, T>, Ep::Next<M>>`, so the type
state advances in lockstep with the move at zero runtime cost.

### Compile-time branching

Two gates rely on dead-code elimination to remove whole move classes:

- castling: `if n_checkers == 0 && C::has_color::<T>() { ... }` (a `const trait`
  method — `Castling` is `const trait`).
- en passant: `if Ep::AVAILABLE { ... }` (an associated const).

For these to be correct, `perft_fen_dispatch!` must map the runtime FEN fields
(side, castling mask, ep) onto the matching `Pos<...>` type. Keep the macro's
arms in sync with the type state it instantiates.

### Ghost cells

The board lives in a `GhostCell`; `Pos` holds `&GhostCell`. Single-owner
borrowing is checked statically — no `RefCell` overhead. `fold_moves` reads the
board's `Copy` state (~a dozen `u64` bitboards) into a `Snapshot` so it can drop
the borrow before the visitor takes the token mutably.

### Move generation

`fold_moves` is the single monolithic legal-move generator, generic over a
`MoveVisitor` accumulator (used by `perft`; the same shape serves move lists).
Checks, pins, evasions, castling and en passant are all handled inline. It is the
hot path — keep changes surgical.

## Invariants

- `Ep::AVAILABLE == true` iff `board.ep_square.is_some()`.
- `board.castling` (runtime `u8`) is the source of truth for *current* rights.
  The type-level `C` encodes the *initial* rights and goes stale when a king or
  rook makes a quiet/capture move (a `Quiet` flag can't say *which* piece
  moved). So `C::has_color::<T>()` is only a "could this color ever castle"
  gate, never a "may castle now" check — that's the runtime `u8`'s job.

## Lessons learned (do not reintroduce these)

- **Castling staleness.** Castling rights were once type-level-only; a quiet
  king/rook move couldn't clear them, so the engine generated illegal castles in
  "move away, return, castle" lines. Fixed with runtime `Board::castling`,
  cleared in `begin_move` and restored in `end_move`.
- **`cargo bench` runs lib/bin too.** By default it also benches the lib/bin
  targets with the *standard* test harness, which rejects criterion flags
  (`--save-baseline`). Both crates set `bench = false`
  (`[lib] bench = false`, `[[bin]] bench = false`) so only the criterion target
  runs.

## Toolchain

- Nightly Rust, edition 2024. Uses `#![feature(const_trait_impl)]` and
  `#![feature(const_cmp)]`; also `#![allow(unused_labels)]`,
  `#![allow(non_camel_case_types)]`.
- Nix shell (`flake.nix`) provides the dev environment.

## Testing & benchmarking

- **Tests** = perft against canonical node counts: the six standard positions
  (startpos, kiwipete, pos3–6) plus one with an en-passant square in the FEN.
  Run with `cargo test --release -p muncher_core` — depth-5/6 perft is slow in a
  debug build.
- **Benchmarks** = criterion over those positions: `cargo bench -p muncher_core`.
  Save/compare baselines with `--save-baseline <name>` / `--baseline <name>`.
- **Ground truth** = nephrid (`go perft N` UCI command, or its own perft tests).

## Style

- Prefer type-level state and macro dispatch over runtime branches in the hot
  path. When a type transition can't express a rule, backstop it with a runtime
  field and say so in a comment.
- Comment the *why* (compile-time elimination, ghost-cell single-ownership), not
  the *what*.
- Use `#[rustfmt::skip]` where formatting fights the code (e.g. the `Castling`
  table); `#[allow(...)]` for generated type names (`_0000`..`_1111`).
- Keep `fold_moves` surgical.
