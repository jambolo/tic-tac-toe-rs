//! # ttt-core — a UI-agnostic Tic-Tac-Toe engine
//!
//! This crate is the **compute core** of the application template: it knows the
//! rules of Tic-Tac-Toe and how to pick an optimal move, and it knows *nothing*
//! about Tauri, the webview, or any UI. That separation is deliberate — it lets
//! the engine be unit-tested, benchmarked, or reused headless without a GUI.
//!
//! The artificial intelligence is provided entirely by the [`game_player`] crate
//! (a git submodule): we implement its [`State`], [`StaticEvaluator`], and
//! [`minimax::ResponseGenerator`] traits for Tic-Tac-Toe, then call
//! [`minimax::search`] to obtain the best move.
//!
//! ## Player convention
//!
//! `game-player` fixes Alice as the maximizing player and Bob as the minimizing
//! player, and its evaluator is *always* scored from Alice's perspective. We map
//! **Alice → X** and **Bob → O**. The human may play either mark; the computer
//! simply calls [`best_move`] whenever it is its turn, and the search's internal
//! max/min logic handles whichever side is to move.

use game_player::minimax::{self, ResponseGenerator};
use game_player::{PlayerId, State, StaticEvaluator};

/// Number of cells on the board.
pub const BOARD_SIZE: usize = 9;

/// Full-game search depth. A Tic-Tac-Toe game is at most 9 plies, so searching
/// to depth 9 explores the whole tree and yields perfect play.
pub const MAX_DEPTH: u32 = 9;

/// A player's mark. Alice (the maximizer) plays `X`; Bob (the minimizer) plays `O`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    X,
    O,
}

impl Mark {
    /// The `game-player` player id this mark corresponds to.
    pub fn player(self) -> PlayerId {
        match self {
            Mark::X => PlayerId::Alice,
            Mark::O => PlayerId::Bob,
        }
    }

    /// The mark for a given `game-player` player id.
    pub fn from_player(p: PlayerId) -> Self {
        match p {
            PlayerId::Alice => Mark::X,
            PlayerId::Bob => Mark::O,
        }
    }

    /// The opposing mark.
    pub fn other(self) -> Self {
        match self {
            Mark::X => Mark::O,
            Mark::O => Mark::X,
        }
    }
}

/// The outcome of a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    InProgress,
    Win(Mark),
    Draw,
}

/// A move: place the current player's mark at `cell` (0..=8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub cell: usize,
}

/// Why a human move was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    /// The game has already ended.
    GameOver,
    /// The cell index was outside `0..9`.
    OutOfBounds,
    /// The target cell is already occupied.
    Occupied,
}

impl core::fmt::Display for MoveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            MoveError::GameOver => "the game is already over",
            MoveError::OutOfBounds => "cell index out of bounds",
            MoveError::Occupied => "that cell is already taken",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for MoveError {}

/// The eight winning lines (rows, columns, diagonals).
const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

/// An immutable Tic-Tac-Toe position: the nine cells plus whose turn it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Board {
    cells: [Option<Mark>; BOARD_SIZE],
    turn: PlayerId,
}

impl Board {
    /// A fresh, empty board on which `first` moves next.
    pub fn new(first: Mark) -> Self {
        Board {
            cells: [None; BOARD_SIZE],
            turn: first.player(),
        }
    }

    /// A copy of the nine cells.
    pub fn cells(&self) -> [Option<Mark>; BOARD_SIZE] {
        self.cells
    }

    /// The mark of the player to move.
    pub fn turn_mark(&self) -> Mark {
        Mark::from_player(self.turn)
    }

    /// The winning mark, if any line is completed.
    pub fn winner(&self) -> Option<Mark> {
        for line in LINES {
            if let Some(m) = self.cells[line[0]]
                && self.cells[line[1]] == Some(m) && self.cells[line[2]] == Some(m) {
                    return Some(m);
                }
        }
        None
    }

    /// Whether every cell is filled.
    pub fn is_full(&self) -> bool {
        self.cells.iter().all(Option::is_some)
    }

    /// The current outcome.
    pub fn status(&self) -> Status {
        if let Some(m) = self.winner() {
            Status::Win(m)
        } else if self.is_full() {
            Status::Draw
        } else {
            Status::InProgress
        }
    }

    /// All legal moves. Returns an empty vector exactly when the position is
    /// terminal — the contract `game-player`'s search relies on.
    pub fn legal_moves(&self) -> Vec<Move> {
        if self.status() != Status::InProgress {
            return Vec::new();
        }
        (0..BOARD_SIZE)
            .filter(|&i| self.cells[i].is_none())
            .map(|cell| Move { cell })
            .collect()
    }

    /// Number of filled cells, as an `f32` for use in evaluation.
    fn filled(&self) -> f32 {
        self.cells.iter().filter(|c| c.is_some()).count() as f32
    }
}

impl State for Board {
    type Action = Move;

    fn fingerprint(&self) -> u64 {
        // Two bits per cell (0 = empty, 1 = X, 2 = O), plus a turn bit up top.
        let mut fp: u64 = 0;
        for (i, c) in self.cells.iter().enumerate() {
            let v: u64 = match c {
                None => 0,
                Some(Mark::X) => 1,
                Some(Mark::O) => 2,
            };
            fp |= v << (i * 2);
        }
        fp |= (self.turn as u64) << 62;
        fp
    }

    fn whose_turn(&self) -> PlayerId {
        self.turn
    }

    fn is_terminal(&self) -> bool {
        self.status() != Status::InProgress
    }

    fn apply(&self, action: &Self::Action) -> Self {
        debug_assert!(self.cells[action.cell].is_none(), "apply() on an occupied cell");
        let mut cells = self.cells;
        cells[action.cell] = Some(self.turn_mark());
        Board {
            cells,
            turn: self.turn.other(),
        }
    }
}

/// Static evaluator, scored from Alice's (X's) perspective.
///
/// A win is worth ±`WIN`, adjusted by how many cells were filled to reach it, so
/// the search prefers to **win as quickly as possible** and **lose as slowly as
/// possible**. Non-terminal positions score `0`; because the search reaches full
/// depth, those interior values are always backed up from real terminals.
pub struct Evaluator;

const WIN: f32 = 100.0;

impl StaticEvaluator for Evaluator {
    type State = Board;

    fn evaluate(&self, state: &Board) -> f32 {
        match state.winner() {
            Some(Mark::X) => self.alice_wins_value() - state.filled(),
            Some(Mark::O) => self.bob_wins_value() + state.filled(),
            None => 0.0, // draw or (interior) ongoing position
        }
    }

    fn alice_wins_value(&self) -> f32 {
        WIN
    }

    fn bob_wins_value(&self) -> f32 {
        -WIN
    }
}

/// Move generator. Delegates to [`Board::legal_moves`], which already honours the
/// "empty result iff terminal" policy the search requires.
pub struct Generator;

impl ResponseGenerator for Generator {
    type State = Board;

    fn generate(&self, state: &Board, _depth: u32) -> Vec<Move> {
        state.legal_moves()
    }
}

/// Compute the optimal move for the player to move, via `game-player`'s
/// alpha-beta minimax search. Returns `None` only if the position is terminal.
pub fn best_move(board: &Board) -> Option<Move> {
    minimax::search(&Evaluator, &Generator, board, MAX_DEPTH)
}

/// A convenience facade tying a board to the human's chosen mark. The Tauri shell
/// uses this as the authoritative game object; the rules live here, not in the UI.
#[derive(Debug, Clone)]
pub struct Game {
    pub board: Board,
    pub human: Mark,
}

impl Game {
    /// Start a game. The human plays `human`; `human_starts` decides who moves first.
    pub fn new(human: Mark, human_starts: bool) -> Self {
        let first = if human_starts { human } else { human.other() };
        Game {
            board: Board::new(first),
            human,
        }
    }

    /// The current outcome.
    pub fn status(&self) -> Status {
        self.board.status()
    }

    /// Whether it is the human's turn.
    pub fn is_human_turn(&self) -> bool {
        self.board.turn_mark() == self.human
    }

    /// Apply a move for the player to move, validating legality.
    pub fn play(&mut self, cell: usize) -> Result<(), MoveError> {
        if self.board.status() != Status::InProgress {
            return Err(MoveError::GameOver);
        }
        if cell >= BOARD_SIZE {
            return Err(MoveError::OutOfBounds);
        }
        if self.board.cells()[cell].is_some() {
            return Err(MoveError::Occupied);
        }
        self.board = self.board.apply(&Move { cell });
        Ok(())
    }

    /// The optimal move for the player to move.
    pub fn ai_best(&self) -> Option<Move> {
        best_move(&self.board)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a board from a compact string of `X`, `O`, `.`, with `turn` to move.
    fn board_from(s: &str, turn: Mark) -> Board {
        let cells: Vec<Option<Mark>> = s
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| match c {
                'X' => Some(Mark::X),
                'O' => Some(Mark::O),
                '.' => None,
                other => panic!("bad board char {other:?}"),
            })
            .collect();
        assert_eq!(cells.len(), BOARD_SIZE, "board must have 9 cells");
        let mut arr = [None; BOARD_SIZE];
        arr.copy_from_slice(&cells);
        Board {
            cells: arr,
            turn: turn.player(),
        }
    }

    #[test]
    fn detects_row_column_diagonal_wins() {
        assert_eq!(board_from("XXX ... ...", Mark::O).winner(), Some(Mark::X));
        assert_eq!(board_from("O.. O.. O..", Mark::X).winner(), Some(Mark::O));
        assert_eq!(board_from("X.. .X. ..X", Mark::O).winner(), Some(Mark::X));
        assert_eq!(board_from("..O .O. O..", Mark::X).winner(), Some(Mark::O));
        assert_eq!(board_from("XOX OXO OXO", Mark::X).winner(), None);
    }

    #[test]
    fn empty_board_has_a_best_move() {
        let b = Board::new(Mark::X);
        assert!(best_move(&b).is_some());
    }

    #[test]
    fn terminal_board_has_no_best_move() {
        let b = board_from("XXX OO. ...", Mark::O);
        assert_eq!(b.status(), Status::Win(Mark::X));
        assert_eq!(best_move(&b), None);
    }

    #[test]
    fn ai_takes_the_immediate_win() {
        // X to move; X can complete the top row at cell 2.
        let b = board_from("XX. OO. ...", Mark::X);
        assert_eq!(best_move(&b), Some(Move { cell: 2 }));
    }

    #[test]
    fn ai_blocks_the_immediate_loss() {
        // O to move; X threatens to complete the top row at cell 2, so O must block there.
        let b = board_from("XX. O.. ...", Mark::O);
        assert_eq!(best_move(&b), Some(Move { cell: 2 }));
    }

    /// Play an optimal-vs-optimal game from an empty board with `first` to move.
    fn optimal_selfplay(first: Mark) -> Status {
        let mut b = Board::new(first);
        while b.status() == Status::InProgress {
            let mv = best_move(&b).expect("non-terminal position must have a move");
            b = b.apply(&mv);
        }
        b.status()
    }

    #[test]
    fn optimal_vs_optimal_is_always_a_draw() {
        assert_eq!(optimal_selfplay(Mark::X), Status::Draw);
        assert_eq!(optimal_selfplay(Mark::O), Status::Draw);
    }

    /// Exhaustively explore *every* line of play the human could choose while the
    /// computer always replies with `best_move`, asserting the human never wins.
    fn ai_never_loses(board: Board, human: Mark) {
        match board.status() {
            Status::Win(w) => {
                assert_ne!(w, human, "the computer allowed the human ({human:?}) to win");
                return;
            }
            Status::Draw => return,
            Status::InProgress => {}
        }
        if board.turn_mark() == human {
            // Human may play any legal move; explore all of them.
            for mv in board.legal_moves() {
                ai_never_loses(board.apply(&mv), human);
            }
        } else {
            // Computer plays its single optimal move.
            let mv = best_move(&board).expect("non-terminal position must have a move");
            ai_never_loses(board.apply(&mv), human);
        }
    }

    #[test]
    fn computer_is_unbeatable_as_o_when_human_starts() {
        // Human is X and moves first; computer is O.
        ai_never_loses(Board::new(Mark::X), Mark::X);
    }

    #[test]
    fn computer_is_unbeatable_as_x_when_computer_starts() {
        // Human is O; computer is X and moves first.
        ai_never_loses(Board::new(Mark::X), Mark::O);
    }

    #[test]
    fn apply_is_immutable_and_flips_turn() {
        let b0 = Board::new(Mark::X);
        let b1 = b0.apply(&Move { cell: 4 });
        assert_eq!(b0.cells()[4], None, "original board must be unchanged");
        assert_eq!(b1.cells()[4], Some(Mark::X));
        assert_eq!(b1.turn_mark(), Mark::O);
        assert_ne!(b0.fingerprint(), b1.fingerprint());
    }
}
