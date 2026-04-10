use leptos::*;
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Empty,
    X,
    O,
}

#[component]
fn App() -> impl IntoView {
    let (board, set_board) = create_signal([Cell::Empty; 9]);
    let (turn, set_turn) = create_signal(Cell::X);
    let (x_wins, set_x_wins) = create_signal(0u32);
    let (o_wins, set_o_wins) = create_signal(0u32);
    let (draws, set_draws) = create_signal(0u32);
    let (game_over, set_game_over) = create_signal(false);
    let (status, set_status) = create_signal("X's turn".to_string());

    let check_winner = move |b: [Cell; 9]| -> Option<Cell> {
        let wins = [[0,1,2],[3,4,5],[6,7,8],[0,3,6],[1,4,7],[2,5,8],[0,4,8],[2,4,6]];
        for w in wins {
            if b[w[0]] != Cell::Empty && b[w[0]] == b[w[1]] && b[w[1]] == b[w[2]] {
                return Some(b[w[0]]);
            }
        }
        None
    };

    let play = move |idx: usize| {
        if game_over.get() { return; }
        let mut b = board.get();
        if b[idx] != Cell::Empty { return; }
        b[idx] = turn.get();
        set_board.set(b);

        if let Some(winner) = check_winner(b) {
            set_game_over.set(true);
            match winner {
                Cell::X => {
                    set_x_wins.update(|w| *w += 1);
                    set_status.set("X wins!".into());
                }
                Cell::O => {
                    set_o_wins.update(|w| *w += 1);
                    set_status.set("O wins!".into());
                }
                _ => {}
            }
        } else if b.iter().all(|c| *c != Cell::Empty) {
            set_game_over.set(true);
            set_draws.update(|d| *d += 1);
            set_status.set("Draw!".into());
        } else {
            let next = if turn.get() == Cell::X { Cell::O } else { Cell::X };
            set_turn.set(next);
            set_status.set(if next == Cell::X { "X's turn".into() } else { "O's turn".into() });
        }
    };

    let reset = move |_| {
        set_board.set([Cell::Empty; 9]);
        set_turn.set(Cell::X);
        set_game_over.set(false);
        set_status.set("X's turn".into());
    };

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a1a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center; padding: 20px;">
            <h1 style="color: #7c4dff; margin-bottom: 16px;">"Tic Tac Toe"</h1>

            <div style="display: flex; gap: 20px; margin-bottom: 16px;">
                <div style="text-align: center; background: #16213e; padding: 10px 20px; border-radius: 8px;">
                    <div style="font-size: 20px; font-weight: bold; color: #e94560;">{x_wins}</div>
                    <div style="font-size: 11px; color: #888;">"X WINS"</div>
                </div>
                <div style="text-align: center; background: #16213e; padding: 10px 20px; border-radius: 8px;">
                    <div style="font-size: 20px; font-weight: bold; color: #888;">{draws}</div>
                    <div style="font-size: 11px; color: #888;">"DRAWS"</div>
                </div>
                <div style="text-align: center; background: #16213e; padding: 10px 20px; border-radius: 8px;">
                    <div style="font-size: 20px; font-weight: bold; color: #4fc3f7;">{o_wins}</div>
                    <div style="font-size: 11px; color: #888;">"O WINS"</div>
                </div>
            </div>

            <div style="font-size: 18px; margin-bottom: 16px; min-height: 28px;">
                {status}
            </div>

            <div style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; \
                        max-width: 300px; width: 100%; margin-bottom: 20px;">
                {(0..9).map(|i| {
                    view! {
                        <button
                            on:click=move |_| play(i)
                            style=move || {
                                let cell = board.get()[i];
                                let color = match cell {
                                    Cell::X => "color: #e94560;",
                                    Cell::O => "color: #4fc3f7;",
                                    Cell::Empty => "color: transparent;",
                                };
                                format!(
                                    "aspect-ratio: 1; background: #16213e; border: none; border-radius: 12px; \
                                     font-size: 40px; font-weight: bold; cursor: pointer; {} \
                                     transition: all 0.2s;", color
                                )
                            }
                        >
                            {move || match board.get()[i] {
                                Cell::X => "X",
                                Cell::O => "O",
                                Cell::Empty => "\u{00A0}",
                            }}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>

            <button
                on:click=reset
                style="padding: 12px 30px; background: #7c4dff; color: #fff; border: none; \
                       border-radius: 8px; cursor: pointer; font-size: 16px;"
            >"New Game"</button>
        </div>
    }
}

#[wasm_bindgen]
pub fn init(selector: &str) {
    console_error_panic_hook::set_once();
    let document = web_sys::window().unwrap().document().unwrap();
    let el = document.query_selector(selector).unwrap().unwrap();
    mount_to(el.unchecked_into(), App);
}
