use leptos::*;
use wasm_bindgen::prelude::*;

#[derive(Clone, Debug)]
struct Question {
    text: String,
    options: Vec<String>,
    correct: usize,
}

#[component]
fn App() -> impl IntoView {
    let questions = vec![
        Question { text: "What is the capital of France?".into(), options: vec!["London".into(), "Paris".into(), "Berlin".into(), "Madrid".into()], correct: 1 },
        Question { text: "What is 7 x 8?".into(), options: vec!["54".into(), "56".into(), "58".into(), "62".into()], correct: 1 },
        Question { text: "Which planet is closest to the Sun?".into(), options: vec!["Venus".into(), "Earth".into(), "Mercury".into(), "Mars".into()], correct: 2 },
        Question { text: "What gas do plants absorb?".into(), options: vec!["Oxygen".into(), "Nitrogen".into(), "Carbon Dioxide".into(), "Helium".into()], correct: 2 },
        Question { text: "How many continents are there?".into(), options: vec!["5".into(), "6".into(), "7".into(), "8".into()], correct: 2 },
    ];

    let (current, set_current) = create_signal(0usize);
    let (score, set_score) = create_signal(0u32);
    let (answered, set_answered) = create_signal(false);
    let (selected, set_selected) = create_signal(None::<usize>);
    let (finished, set_finished) = create_signal(false);
    let total = questions.len();
    let questions = store_value(questions);

    let answer = move |idx: usize| {
        if answered.get() { return; }
        set_selected.set(Some(idx));
        set_answered.set(true);
        let q = &questions.get_value()[current.get()];
        if idx == q.correct {
            set_score.update(|s| *s += 1);
        }
    };

    let next = move |_| {
        let next_q = current.get() + 1;
        if next_q >= total {
            set_finished.set(true);
        } else {
            set_current.set(next_q);
            set_answered.set(false);
            set_selected.set(None);
        }
    };

    let restart = move |_| {
        set_current.set(0);
        set_score.set(0);
        set_answered.set(false);
        set_selected.set(None);
        set_finished.set(false);
    };

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a1a; color: #e0e0e0; \
                     min-height: 100vh; padding: 40px 20px; display: flex; flex-direction: column; \
                     align-items: center;">
            <h1 style="color: #7c4dff; margin-bottom: 8px;">"Quiz Time"</h1>
            <div style="color: #888; margin-bottom: 24px; font-size: 14px;">
                {move || format!("Score: {} / {}", score.get(), total)}
            </div>

            {move || {
                if finished.get() {
                    let s = score.get();
                    let pct = (s as f64 / total as f64 * 100.0) as u32;
                    view! {
                        <div style="text-align: center; background: #16213e; padding: 40px; \
                                    border-radius: 16px; max-width: 500px; width: 100%;">
                            <div style="font-size: 48px; margin-bottom: 12px;">
                                {if pct >= 80 { "🎉" } else if pct >= 60 { "👍" } else { "📚" }}
                            </div>
                            <h2 style="color: #7c4dff; margin-bottom: 8px;">
                                {format!("{}%", pct)}
                            </h2>
                            <p style="color: #888; margin-bottom: 20px;">
                                {format!("You got {} out of {} correct!", s, total)}
                            </p>
                            <button
                                on:click=restart
                                style="padding: 12px 32px; background: #7c4dff; color: #fff; \
                                       border: none; border-radius: 8px; cursor: pointer; font-size: 16px;"
                            >"Try Again"</button>
                        </div>
                    }.into_view()
                } else {
                    let q = questions.get_value()[current.get()].clone();
                    let correct_idx = q.correct;
                    view! {
                        <div style="max-width: 500px; width: 100%;">
                            <div style="background: #16213e; padding: 24px; border-radius: 12px; \
                                        margin-bottom: 16px;">
                                <div style="font-size: 13px; color: #7c4dff; margin-bottom: 8px;">
                                    {move || format!("Question {} of {}", current.get() + 1, total)}
                                </div>
                                <div style="font-size: 20px; font-weight: 600;">
                                    {q.text.clone()}
                                </div>
                            </div>
                            <div style="display: flex; flex-direction: column; gap: 8px;">
                                {q.options.into_iter().enumerate().map(|(i, opt)| {
                                    let opt_clone = opt.clone();
                                    view! {
                                        <button
                                            on:click=move |_| answer(i)
                                            style=move || {
                                                let base = "width: 100%; padding: 14px 20px; border-radius: 8px; \
                                                           cursor: pointer; font-size: 16px; text-align: left; ";
                                                let sel = selected.get();
                                                let ans = answered.get();
                                                if ans && i == correct_idx {
                                                    format!("{}background: #1a3a1a; border: 2px solid #3fb950; color: #3fb950;", base)
                                                } else if ans && sel == Some(i) && i != correct_idx {
                                                    format!("{}background: #3a1a1a; border: 2px solid #f85149; color: #f85149;", base)
                                                } else {
                                                    format!("{}background: #16213e; border: 2px solid #333; color: #e0e0e0;", base)
                                                }
                                            }
                                        >
                                            {opt_clone}
                                        </button>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                            {move || {
                                if answered.get() {
                                    view! {
                                        <button
                                            on:click=next
                                            style="margin-top: 16px; padding: 12px 32px; background: #7c4dff; \
                                                   color: #fff; border: none; border-radius: 8px; cursor: pointer; \
                                                   font-size: 16px; width: 100%;"
                                        >"Next"</button>
                                    }.into_view()
                                } else {
                                    view! { <span></span> }.into_view()
                                }
                            }}
                        </div>
                    }.into_view()
                }
            }}
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
