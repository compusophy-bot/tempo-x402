use leptos::*;
use wasm_bindgen::prelude::*;

#[derive(Clone, Debug)]
struct TodoItem {
    id: u32,
    text: String,
    completed: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Active,
    Completed,
}

#[component]
fn App() -> impl IntoView {
    let (todos, set_todos) = create_signal(Vec::<TodoItem>::new());
    let (input, set_input) = create_signal(String::new());
    let (next_id, set_next_id) = create_signal(1u32);
    let (filter, set_filter) = create_signal(Filter::All);

    let add_todo = move |_| {
        let text = input.get().trim().to_string();
        if !text.is_empty() {
            let id = next_id.get();
            set_next_id.set(id + 1);
            set_todos.update(|t| t.push(TodoItem { id, text, completed: false }));
            set_input.set(String::new());
        }
    };

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" {
            let text = input.get().trim().to_string();
            if !text.is_empty() {
                let id = next_id.get();
                set_next_id.set(id + 1);
                set_todos.update(|t| t.push(TodoItem { id, text, completed: false }));
                set_input.set(String::new());
            }
        }
    };

    let toggle = move |id: u32| {
        set_todos.update(|t| {
            if let Some(item) = t.iter_mut().find(|i| i.id == id) {
                item.completed = !item.completed;
            }
        });
    };

    let delete = move |id: u32| {
        set_todos.update(|t| t.retain(|i| i.id != id));
    };

    let filtered_todos = move || {
        let f = filter.get();
        todos.get().into_iter().filter(|t| match f {
            Filter::All => true,
            Filter::Active => !t.completed,
            Filter::Completed => t.completed,
        }).collect::<Vec<_>>()
    };

    let active_count = move || todos.get().iter().filter(|t| !t.completed).count();

    let filter_btn = move |f: Filter, label: &'static str| {
        let is_active = move || filter.get() == f;
        view! {
            <button
                style=move || format!(
                    "padding: 6px 14px; border: 1px solid {}; background: {}; color: #e0e0e0; \
                     cursor: pointer; border-radius: 4px; font-size: 13px;",
                    if is_active() { "#7fdbca" } else { "#333" },
                    if is_active() { "#1a3a3a" } else { "#1a1a2e" }
                )
                on:click=move |_| set_filter.set(f)
            >
                {label}
            </button>
        }
    };

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; padding: 40px 20px; display: flex; flex-direction: column; \
                     align-items: center;">
            <h1 style="font-size: 28px; color: #7fdbca; margin-bottom: 20px;">"Todo List"</h1>
            <div style="width: 100%; max-width: 500px;">
                <div style="display: flex; gap: 8px; margin-bottom: 16px;">
                    <input
                        type="text"
                        placeholder="What needs to be done?"
                        style="flex: 1; padding: 10px 14px; background: #111; border: 1px solid #333; \
                               color: #e0e0e0; border-radius: 6px; font-size: 15px; outline: none;"
                        prop:value=input
                        on:input=move |ev| set_input.set(event_target_value(&ev))
                        on:keydown=on_keydown
                    />
                    <button
                        style="padding: 10px 20px; background: #1a3a3a; border: 1px solid #7fdbca; \
                               color: #7fdbca; cursor: pointer; border-radius: 6px; font-size: 15px;"
                        on:click=add_todo
                    >"Add"</button>
                </div>

                <div style="display: flex; gap: 8px; margin-bottom: 16px; align-items: center;">
                    {filter_btn(Filter::All, "All")}
                    {filter_btn(Filter::Active, "Active")}
                    {filter_btn(Filter::Completed, "Completed")}
                    <span style="margin-left: auto; color: #666; font-size: 13px;">
                        {move || format!("{} items left", active_count())}
                    </span>
                </div>

                <div style="display: flex; flex-direction: column; gap: 4px;">
                    <For
                        each=filtered_todos
                        key=|item| item.id
                        children=move |item: TodoItem| {
                            let id = item.id;
                            let completed = item.completed;
                            let text = item.text.clone();
                            view! {
                                <div style="display: flex; align-items: center; gap: 10px; \
                                            padding: 10px 14px; background: #111; border-radius: 6px; \
                                            border: 1px solid #1a1a2e;">
                                    <input
                                        type="checkbox"
                                        prop:checked=completed
                                        on:change=move |_| toggle(id)
                                        style="width: 18px; height: 18px; cursor: pointer;"
                                    />
                                    <span style=move || format!(
                                        "flex: 1; font-size: 15px; {}",
                                        if completed { "text-decoration: line-through; color: #555;" }
                                        else { "color: #e0e0e0;" }
                                    )>
                                        {text}
                                    </span>
                                    <button
                                        style="background: none; border: none; color: #e06c75; \
                                               cursor: pointer; font-size: 16px; padding: 4px 8px;"
                                        on:click=move |_| delete(id)
                                    >"\u{2715}"</button>
                                </div>
                            }
                        }
                    />
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
