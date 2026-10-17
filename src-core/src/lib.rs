//! # ttt-core — a UI-agnostic Tic-Tac-Toe engine
//!
//! This crate is the **compute core** of the application template: it knows the rules of Tic-Tac-Toe and how to pick an optimal
//! move, and it knows *nothing* about Tauri, the webview, or any UI. That separation is deliberate — it lets the engine be
//! unit-tested, benchmarked, or reused headless without a GUI.
//!
//! The artificial intelligence is provided entirely by the [`game_player`] crate (a git dependency): we implement its [`State`],
//! [`StaticEvaluator`], and [`minimax::ResponseGenerator`] traits for Tic-Tac-Toe, then call [`minimax::search`] to obtain the best
//! move.
//!
//! ## Player convention
//!
//! `game-player` fixes Alice as the maximizing player and Bob as the minimizing player, and its evaluator is *always* scored from
//! Alice's perspective. We map **Alice → X** and **Bob → O**. The human may play either mark; the computer simply calls
//! [`best_move`] whenever it is its turn, and the search's internal max/min logic handles whichever side is to move.

use game_player::minimax::{self, ResponseGenerator};
use game_player::{PlayerId, State, StaticEvaluator};

/// Number of cells on the board.
pub const BOARD_SIZE: usize = 9;

/// Full-game search depth. A Tic-Tac-Toe game is at most 9 plies, so searching to depth 9 explores the whole tree and yields
/// perfect play.
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
                && self.cells[line[1]] == Some(m)
                && self.cells[line[2]] == Some(m)
            {
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

    /// All legal moves. Returns an empty vector exactly when the position is terminal — the contract `game-player`'s search
    /// relies on.
    pub fn legal_moves(&self) -> Vec<Move> {
        if self.status() != Status::InProgress {
            return Vec::new();
        }
        (0..BOARD_SIZE)
            .filter(|&i| self.cells[i].is_none())
            .map(|cell| Move { cell })
            .collect()
    }

    /// Number of lines where `mark` has two cells and the third is empty —
    /// i.e. immediate winning threats for `mark`.
    fn threats(&self, mark: Mark) -> usize {
        LINES
            .iter()
            .filter(|line| {
                let own = line.iter().filter(|&&i| self.cells[i] == Some(mark)).count();
                let empty = line.iter().filter(|&&i| self.cells[i].is_none()).count();
                own == 2 && empty == 1
            })
            .count()
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
/// A completed win is worth exactly ±`WIN` — matching `alice_wins_value` / `bob_wins_value` so the search's win-recognition
/// cutoffs fire. Non-terminal positions are scored by immediate threats per side (a threat is a line with two own marks and an
/// empty cell): one threat is worth `WIN / 2`, two or more (a fork) `WIN - 1` — strictly below `WIN`, so only a real terminal
/// reads as a win. X's and O's threat scores are combined by subtraction.
///
/// When neither side has a threat, the position is scored on square occupancy instead: the centre is worth `WIN / 5` and each
/// corner `WIN / 10`, again combined by subtraction. The maximum positional score (centre plus all four corners, `3 * WIN / 5`)
/// stays below a single threat's `WIN / 2` differential only in mixed positions; more importantly it stays below `WIN`, so it can
/// never read as a win.
pub struct Evaluator;

const WIN: f32 = 100.0;

/// Threat score for one side: 0, `WIN / 2` for a single threat, `WIN - 1` for a fork.
fn threat_value(count: usize) -> f32 {
    match count {
        0 => 0.0,
        1 => WIN / 2.0,
        _ => WIN - 1.0,
    }
}

/// The four corner cells.
const CORNERS: [usize; 4] = [0, 2, 6, 8];

/// The centre cell.
const CENTER: usize = 4;

/// Positional score for one side: `WIN / 5` for holding the centre plus `WIN / 10` per corner held. Used only when neither side has
/// a threat.
fn position_value(board: &Board, mark: Mark) -> f32 {
    let cells = board.cells();
    let mut value = 0.0;
    if cells[CENTER] == Some(mark) {
        value += WIN / 5.0;
    }
    for corner in CORNERS {
        if cells[corner] == Some(mark) {
            value += WIN / 10.0;
        }
    }
    value
}

impl StaticEvaluator for Evaluator {
    type State = Board;

    fn evaluate(&self, state: &Board) -> f32 {
        match state.winner() {
            Some(Mark::X) => self.alice_wins_value(),
            Some(Mark::O) => self.bob_wins_value(),
            None if state.is_full() => 0.0, // draw
            None => {
                let x_threats = state.threats(Mark::X);
                let o_threats = state.threats(Mark::O);
                if x_threats == 0 && o_threats == 0 {
                    position_value(state, Mark::X) - position_value(state, Mark::O)
                } else {
                    threat_value(x_threats) - threat_value(o_threats)
                }
            }
        }
    }

    fn alice_wins_value(&self) -> f32 {
        WIN
    }

    fn bob_wins_value(&self) -> f32 {
        -WIN
    }
}

/// Move generator. Delegates to [`Board::legal_moves`], which already honours the "empty result iff terminal" policy the search
/// requires.
pub struct Generator;

impl ResponseGenerator for Generator {
    type State = Board;

    fn generate(&self, state: &Board, _depth: u32) -> Vec<Move> {
        state.legal_moves()
    }
}

/// Compute the optimal move for the player to move, via `game-player`'s alpha-beta minimax search. Returns `None` only if the
/// position is terminal.
pub fn best_move(board: &Board) -> Option<Move> {
    minimax::search(&Evaluator, &Generator, board, MAX_DEPTH)
}

/// A convenience facade tying a board to the human's chosen mark. The Tauri shell uses this as the authoritative game object; the
/// rules live here, not in the UI.
#[derive(Debug, Clone)]
pub struct Game {
    pub board: Board,
    pub human: Mark,
}

impl Game {
    /// Start a game. X always moves first; `human_starts` decides whether the human plays X (moving first) or O (moving second).
    pub fn new(human_starts: bool) -> Self {
        Game {
            board: Board::new(Mark::X),
            human: if human_starts { Mark::X } else { Mark::O },
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

    /// Exhaustively explore *every* line of play the human could choose while the computer always replies with `best_move`,
    /// asserting the human never wins.
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
    fn computer_going_first_beats_a_side_square_reply() {
        // The computer (X) opens optimally. O answers with a side square — a theoretically losing reply to a perfect opening —
        // then plays optimally for the rest of the game. X must still convert the win.
        const SIDES: [usize; 4] = [1, 3, 5, 7];
        let start = Board::new(Mark::X);
        let opening = best_move(&start).expect("empty board must have a move");
        let after_x = start.apply(&opening);

        for side in SIDES {
            if after_x.cells()[side].is_some() {
                continue; // X's opening took this square
            }
            let mut b = after_x.apply(&Move { cell: side });
            while b.status() == Status::InProgress {
                let mv = best_move(&b).expect("non-terminal position must have a move");
                b = b.apply(&mv);
            }
            assert_eq!(
                b.status(),
                Status::Win(Mark::X),
                "O opened with side square {side} and should have lost"
            );
        }
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

    #[test]
    fn mark_conversions_round_trip() {
        for m in [Mark::X, Mark::O] {
            assert_eq!(Mark::from_player(m.player()), m);
            assert_eq!(m.other().other(), m);
            assert_ne!(m.other(), m);
        }
        assert_eq!(Mark::X.player(), PlayerId::Alice);
        assert_eq!(Mark::O.player(), PlayerId::Bob);
    }

    #[test]
    fn full_board_without_a_winner_is_a_draw() {
        let b = board_from("XOX OXO OXO", Mark::X);
        assert!(b.is_full());
        assert_eq!(b.status(), Status::Draw);
    }

    #[test]
    fn legal_moves_are_empty_exactly_when_terminal() {
        // The contract game-player's search relies on.
        let won = board_from("XXX OO. ...", Mark::O);
        assert!(won.legal_moves().is_empty());

        let drawn = board_from("XOX OXO OXO", Mark::X);
        assert!(drawn.legal_moves().is_empty());

        let open = board_from("X.. .O. ...", Mark::X);
        let moves = open.legal_moves();
        assert_eq!(moves.len(), 7);
        assert!(moves.iter().all(|m| open.cells()[m.cell].is_none()));

        assert_eq!(Board::new(Mark::X).legal_moves().len(), BOARD_SIZE);
    }

    #[test]
    fn fingerprint_distinguishes_turn_on_identical_cells() {
        let x_to_move = board_from("X.. .O. ...", Mark::X);
        let o_to_move = board_from("X.. .O. ...", Mark::O);
        assert_ne!(x_to_move.fingerprint(), o_to_move.fingerprint());
    }

    #[test]
    fn evaluator_scores_terminal_wins_at_exactly_win_value() {
        // A completed win must equal alice_wins_value/bob_wins_value so the search's win-recognition cutoff (`is_winning_value`)
        // fires.
        let x_won = board_from("XXX OO. ...", Mark::O);
        assert_eq!(Evaluator.evaluate(&x_won), Evaluator.alice_wins_value());

        let o_won = board_from("OOO XX. ..X", Mark::X);
        assert_eq!(Evaluator.evaluate(&o_won), Evaluator.bob_wins_value());
    }

    #[test]
    fn evaluator_scores_single_threat_as_half_win() {
        // X has two in the top row with cell 2 open; O has no threat.
        let x_threat = board_from("XX. O.. ...", Mark::O);
        assert_eq!(Evaluator.evaluate(&x_threat), 50.0);

        // Mirror for O.
        let o_threat = board_from("OO. X.. ..X", Mark::X);
        assert_eq!(Evaluator.evaluate(&o_threat), -50.0);
    }

    #[test]
    fn evaluator_scores_fork_as_win_minus_one() {
        // X threatens the top row (cell 2) and the left column (cell 6): a fork. O's marks (cells 5 and 7) share no line, so O has
        // no threat.
        let x_fork = board_from("XX. X.O .O.", Mark::O);
        assert_eq!(x_fork.threats(Mark::X), 2);
        assert_eq!(Evaluator.evaluate(&x_fork), 99.0);

        // Mirror for O.
        let o_fork = board_from("OO. O.X .X.", Mark::X);
        assert_eq!(o_fork.threats(Mark::O), 2);
        assert_eq!(Evaluator.evaluate(&o_fork), -99.0);
    }

    #[test]
    fn evaluator_scores_center_and_corners_when_no_threat() {
        // Centre is worth WIN/5, a corner WIN/10, mirrored for O.
        assert_eq!(Evaluator.evaluate(&board_from("... .X. ...", Mark::O)), 20.0);
        assert_eq!(Evaluator.evaluate(&board_from("... .O. ...", Mark::X)), -20.0);
        assert_eq!(Evaluator.evaluate(&board_from("X.. ... ...", Mark::O)), 10.0);

        // X holds centre + a corner (30), O holds a corner (10); the shared diagonal is blocked, so no threats: 30 - 10 = 20.
        let mixed = board_from("X.. .X. ..O", Mark::O);
        assert_eq!(mixed.threats(Mark::X), 0);
        assert_eq!(Evaluator.evaluate(&mixed), 20.0);
    }

    #[test]
    fn evaluator_prefers_threats_over_position() {
        // X threatens the top row; O holds the centre. Threat scoring applies, positional scoring does not: 50, not 50 - 20.
        let b = board_from("XX. .O. ...", Mark::O);
        assert_eq!(b.threats(Mark::X), 1);
        assert_eq!(Evaluator.evaluate(&b), 50.0);
    }

    #[test]
    fn evaluator_combines_opposing_threats_by_subtraction() {
        // X threatens the top row, O threatens the middle row: 50 - 50 = 0.
        let both = board_from("XX. OO. .X.", Mark::X);
        assert_eq!(both.threats(Mark::X), 1);
        assert_eq!(both.threats(Mark::O), 1);
        assert_eq!(Evaluator.evaluate(&both), 0.0);
    }

    #[test]
    fn evaluator_scores_nonterminal_and_draw_as_zero() {
        assert_eq!(Evaluator.evaluate(&Board::new(Mark::X)), 0.0);
        assert_eq!(Evaluator.evaluate(&board_from("XOX OXO OXO", Mark::X)), 0.0);
    }

    #[test]
    fn game_new_always_opens_with_x() {
        // X always moves first; the human's mark follows from who starts.
        let g = Game::new(true);
        assert_eq!(g.board.turn_mark(), Mark::X);
        assert_eq!(g.human, Mark::X);
        assert!(g.is_human_turn());

        let g = Game::new(false);
        assert_eq!(g.board.turn_mark(), Mark::X);
        assert_eq!(g.human, Mark::O);
        assert!(!g.is_human_turn());
    }

    #[test]
    fn game_play_rejects_out_of_bounds() {
        let mut g = Game::new(true);
        assert_eq!(g.play(9), Err(MoveError::OutOfBounds));
        assert_eq!(g.play(usize::MAX), Err(MoveError::OutOfBounds));
    }

    #[test]
    fn game_play_rejects_occupied_cells() {
        let mut g = Game::new(true);
        g.play(4).unwrap();
        assert_eq!(g.play(4), Err(MoveError::Occupied));
    }

    #[test]
    fn game_play_rejects_moves_after_the_game_ends() {
        let mut g = Game::new(true);
        // X: 0, O: 3, X: 1, O: 4, X: 2 => X wins the top row.
        for cell in [0, 3, 1, 4, 2] {
            g.play(cell).unwrap();
        }
        assert_eq!(g.status(), Status::Win(Mark::X));
        assert_eq!(g.play(8), Err(MoveError::GameOver));
    }

    #[test]
    fn game_play_alternates_turns() {
        let mut g = Game::new(true);
        g.play(0).unwrap();
        assert_eq!(g.board.cells()[0], Some(Mark::X));
        assert!(!g.is_human_turn());
        g.play(1).unwrap();
        assert_eq!(g.board.cells()[1], Some(Mark::O));
        assert!(g.is_human_turn());
    }

    #[test]
    fn game_ai_best_matches_best_move() {
        let g = Game::new(true);
        assert_eq!(g.ai_best(), best_move(&g.board));
    }

    #[test]
    fn move_error_messages_are_stable() {
        assert_eq!(MoveError::GameOver.to_string(), "the game is already over");
        assert_eq!(MoveError::OutOfBounds.to_string(), "cell index out of bounds");
        assert_eq!(MoveError::Occupied.to_string(), "that cell is already taken");
    }
}
