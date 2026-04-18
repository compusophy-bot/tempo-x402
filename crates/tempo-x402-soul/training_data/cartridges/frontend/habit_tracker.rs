use leptos::*;
use wasm_bindgen::prelude::*;

#[derive(Clone, Debug)]
struct Habit {
    id: u32,
    name: String,
    streak: u32,
    done_today: bool,
}

#[component]
fn App() -> impl IntoView {
    let (habits, set_habits) = create_signal(Vec::<Habit>::new());
    let (input, set_input) = create_signal(String::new());
    let (next_id, set_next_id) = create_signal(1u32);

    let add_habit = move |_| {
        let name = input.get().trim().to_string();
        if !name.is_empty() {
            let id = next_id.get();
            set_next_id.set(id + 1);
            set_habits.update(|h| h.push(Habit { id, name, streak: 0, done_today: false }));
            set_input.set(String::new());
        }
    };

    let toggle = move |id: u32| {
        set_habits.update(|h| {
            if let Some(habit) = h.iter_mut().find(|x| x.id == id) {
                habit.done_today = !habit.done_today;
                if habit.done_today {
                    habit.streak += 1;
                } else if habit.streak > 0 {
                    habit.streak -= 1;
                }
            }
        });
    };

    let delete = move |id: u32| {
        set_habits.update(|h| h.retain(|x| x.id != id));
    };

    let total = move || habits.get().len();
    let completed = move || habits.get().iter().filter(|h| h.done_today).count();

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; padding: 40px 20px; display: flex; flex-direction: column; \
                     align-items: center;">
            <h1 style="color: #3fb950; margin-bottom: 8px;">"Habit Tracker"</h1>
            <p style="color: #888; margin-bottom: 24px; font-size: 14px;">
                {move || format!("{} / {} completed today", completed(), total())}
            </p>

            // Progress bar
            <div style="width: 100%; max-width: 500px; height: 8px; background: #161b22; \
                        border-radius: 4px; margin-bottom: 24px; overflow: hidden;">
                <div style=move || {
                    let pct = if total() > 0 { completed() * 100 / total() } else { 0 };
                    format!("height: 100%; width: {}%; background: #3fb950; border-radius: 4px; \
                             transition: width 0.3s;", pct)
                }></div>
            </div>

            <div style="width: 100%; max-width: 500px;">
                <div style="display: flex; gap: 8px; margin-bottom: 20px;">
                    <input
                        type="text"
                        placeholder="New habit..."
                        prop:value=input
                        on:input=move |ev| set_input.set(event_target_value(&ev))
                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                            if ev.key() == "Enter" {
                                let name = input.get().trim().to_string();
                                if !name.is_empty() {
                                    let id = next_id.get();
                                    set_next_id.set(id + 1);
                                    set_habits.update(|h| h.push(Habit { id, name, streak: 0, done_today: false }));
                                    set_input.set(String::new());
                                }
                            }
                        }
                        style="flex: 1; padding: 12px 16px; background: #161b22; border: 1px solid #30363d; \
                               color: #e0e0e0; border-radius: 8px; font-size: 15px; outline: none;"
                    />
                    <button
                        on:click=add_habit
                        style="padding: 12px 20px; background: #238636; color: #fff; border: none; \
                               border-radius: 8px; cursor: pointer; font-size: 15px;"
                    >"Add"</button>
                </div>

                <For
                    each=move || habits.get()
                    key=|h| h.id
                    children=move |habit: Habit| {
                        let id = habit.id;
                        let done = habit.done_today;
                        let name = habit.name.clone();
                        let streak = habit.streak;
                        view! {
                            <div style=move || format!(
                                "display: flex; align-items: center; gap: 12px; padding: 14px 16px; \
                                 background: {}; border-radius: 8px; margin-bottom: 6px; \
                                 border: 1px solid {};",
                                if done { "#0d2818" } else { "#161b22" },
                                if done { "#238636" } else { "#30363d" }
                            )>
                                <input
                                    type="checkbox"
                                    prop:checked=done
                                    on:change=move |_| toggle(id)
                                    style="width: 20px; height: 20px; cursor: pointer; accent-color: #3fb950;"
                                />
                                <span style=move || format!(
                                    "flex: 1; font-size: 15px; {}",
                                    if done { "text-decoration: line-through; color: #555;" } else { "" }
                                )>
                                    {name}
                                </span>
                                <span style="font-size: 13px; color: #f0883e; background: #2d1a0a; \
                                             padding: 2px 8px; border-radius: 10px;">
                                    {format!("{} day streak", streak)}
                                </span>
                                <button
                                    on:click=move |_| delete(id)
                                    style="background: none; border: none; color: #f85149; cursor: pointer; \
                                           font-size: 16px; padding: 4px 8px;"
                                >"\u{2715}"</button>
                            </div>
                        }
                    }
                />
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
