use leptos::*;
use wasm_bindgen::prelude::*;

#[derive(Clone, Debug)]
struct PollOption {
    id: u32,
    text: String,
    votes: u32,
}

#[component]
fn App() -> impl IntoView {
    let (options, set_options) = create_signal(vec![
        PollOption { id: 1, text: "Rust".into(), votes: 0 },
        PollOption { id: 2, text: "Python".into(), votes: 0 },
        PollOption { id: 3, text: "TypeScript".into(), votes: 0 },
        PollOption { id: 4, text: "Go".into(), votes: 0 },
    ]);
    let (question, set_question) = create_signal("What is your favorite programming language?".to_string());
    let (voted, set_voted) = create_signal(false);
    let (new_option, set_new_option) = create_signal(String::new());
    let (next_id, set_next_id) = create_signal(5u32);

    let total_votes = move || options.get().iter().map(|o| o.votes).sum::<u32>();

    let vote = move |id: u32| {
        if voted.get() { return; }
        set_options.update(|opts| {
            if let Some(opt) = opts.iter_mut().find(|o| o.id == id) {
                opt.votes += 1;
            }
        });
        set_voted.set(true);
    };

    let add_option = move |_| {
        let text = new_option.get().trim().to_string();
        if !text.is_empty() {
            let id = next_id.get();
            set_next_id.set(id + 1);
            set_options.update(|opts| opts.push(PollOption { id, text, votes: 0 }));
            set_new_option.set(String::new());
        }
    };

    let reset = move |_| {
        set_options.update(|opts| {
            for opt in opts.iter_mut() {
                opt.votes = 0;
            }
        });
        set_voted.set(false);
    };

    let options_list = move || options.get();

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a1a; color: #e0e0e0; \
                     min-height: 100vh; padding: 40px 20px; display: flex; flex-direction: column; \
                     align-items: center;">
            <h1 style="color: #7c4dff; margin-bottom: 8px;">"Poll"</h1>
            <h2 style="color: #ccc; font-size: 18px; margin-bottom: 24px; text-align: center;">
                {question}
            </h2>

            <div style="width: 100%; max-width: 500px;">
                <For
                    each=options_list
                    key=|opt| opt.id
                    children=move |opt: PollOption| {
                        let id = opt.id;
                        let text = opt.text.clone();
                        let votes = opt.votes;
                        let pct = move || {
                            let total = total_votes();
                            if total > 0 { (votes * 100) / total } else { 0 }
                        };
                        view! {
                            <div
                                on:click=move |_| vote(id)
                                style=move || format!(
                                    "position: relative; padding: 16px 20px; margin-bottom: 8px; \
                                     border-radius: 8px; cursor: {}; overflow: hidden; \
                                     border: 2px solid {}; background: #111;",
                                    if voted.get() { "default" } else { "pointer" },
                                    if voted.get() { "#333" } else { "#7c4dff" }
                                )
                            >
                                // Background bar
                                {move || if voted.get() {
                                    view! {
                                        <div style=move || format!(
                                            "position: absolute; left: 0; top: 0; bottom: 0; \
                                             width: {}%; background: #1a1040; transition: width 0.5s;",
                                            pct()
                                        )></div>
                                    }.into_view()
                                } else {
                                    view! { <span></span> }.into_view()
                                }}
                                <div style="position: relative; display: flex; justify-content: space-between; \
                                            align-items: center;">
                                    <span style="font-size: 16px;">{text}</span>
                                    {move || if voted.get() {
                                        view! {
                                            <span style="font-size: 14px; color: #7c4dff; font-weight: bold;">
                                                {format!("{} votes ({}%)", votes, pct())}
                                            </span>
                                        }.into_view()
                                    } else {
                                        view! { <span></span> }.into_view()
                                    }}
                                </div>
                            </div>
                        }
                    }
                />

                <div style="display: flex; gap: 8px; margin-top: 16px;">
                    <input
                        type="text"
                        placeholder="Add option..."
                        prop:value=new_option
                        on:input=move |ev| set_new_option.set(event_target_value(&ev))
                        style="flex: 1; padding: 10px; background: #111; border: 1px solid #333; \
                               color: #e0e0e0; border-radius: 6px; font-size: 14px;"
                    />
                    <button
                        on:click=add_option
                        style="padding: 10px 16px; background: #7c4dff; color: #fff; border: none; \
                               border-radius: 6px; cursor: pointer; font-size: 14px;"
                    >"Add"</button>
                </div>

                <div style="display: flex; justify-content: space-between; margin-top: 16px; \
                            font-size: 14px; color: #888;">
                    <span>{move || format!("{} total votes", total_votes())}</span>
                    <button
                        on:click=reset
                        style="background: none; border: none; color: #f85149; cursor: pointer; \
                               font-size: 14px;"
                    >"Reset"</button>
                </div>
            </div>
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
