use leptos::*;
use wasm_bindgen::prelude::*;

#[derive(Clone, Debug)]
struct Note {
    id: u32,
    title: String,
    content: String,
    color: String,
}

#[component]
fn App() -> impl IntoView {
    let (notes, set_notes) = create_signal(Vec::<Note>::new());
    let (editing, set_editing) = create_signal(None::<u32>);
    let (title, set_title) = create_signal(String::new());
    let (content, set_content) = create_signal(String::new());
    let (color, set_color) = create_signal("#1a1a2e".to_string());
    let (next_id, set_next_id) = create_signal(1u32);

    let colors = vec!["#1a1a2e", "#2d1810", "#1a2e1a", "#1a1a3e", "#2e1a2a", "#2e2a1a"];

    let save_note = move |_| {
        let t = title.get().trim().to_string();
        let c = content.get();
        let col = color.get();
        if t.is_empty() { return; }
        if let Some(id) = editing.get() {
            set_notes.update(|n| {
                if let Some(note) = n.iter_mut().find(|x| x.id == id) {
                    note.title = t;
                    note.content = c;
                    note.color = col;
                }
            });
            set_editing.set(None);
        } else {
            let id = next_id.get();
            set_next_id.set(id + 1);
            set_notes.update(|n| n.push(Note { id, title: t, content: c, color: col }));
        }
        set_title.set(String::new());
        set_content.set(String::new());
    };

    let edit_note = move |id: u32| {
        let n = notes.get();
        if let Some(note) = n.iter().find(|x| x.id == id) {
            set_title.set(note.title.clone());
            set_content.set(note.content.clone());
            set_color.set(note.color.clone());
            set_editing.set(Some(id));
        }
    };

    let delete_note = move |id: u32| {
        set_notes.update(|n| n.retain(|x| x.id != id));
        if editing.get() == Some(id) { set_editing.set(None); }
    };

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; padding: 30px 20px;">
            <h1 style="text-align: center; color: #ffd700; margin-bottom: 24px;">"Notepad"</h1>

            // Editor
            <div style="max-width: 600px; margin: 0 auto 24px; background: #111; \
                        padding: 16px; border-radius: 12px;">
                <input
                    type="text"
                    placeholder="Note title..."
                    prop:value=title
                    on:input=move |ev| set_title.set(event_target_value(&ev))
                    style="width: 100%; padding: 10px; background: #0a0a0a; border: 1px solid #333; \
                           color: #e0e0e0; border-radius: 6px; font-size: 16px; margin-bottom: 8px;"
                />
                <textarea
                    placeholder="Write your note..."
                    prop:value=content
                    on:input=move |ev| set_content.set(event_target_value(&ev))
                    style="width: 100%; height: 120px; padding: 10px; background: #0a0a0a; \
                           border: 1px solid #333; color: #e0e0e0; border-radius: 6px; \
                           font-size: 14px; resize: vertical; margin-bottom: 8px;"
                ></textarea>
                <div style="display: flex; gap: 8px; align-items: center;">
                    {colors.into_iter().map(|c| {
                        let c_owned = c.to_string();
                        let c_style = c.to_string();
                        view! {
                            <div
                                on:click=move |_| set_color.set(c_owned.clone())
                                style=move || format!(
                                    "width: 28px; height: 28px; border-radius: 50%; cursor: pointer; \
                                     background: {}; border: 2px solid {};",
                                    c_style,
                                    if color.get() == c_style { "#ffd700" } else { "#333" }
                                )
                            ></div>
                        }
                    }).collect::<Vec<_>>()}
                    <div style="flex: 1;"></div>
                    <button
                        on:click=save_note
                        style="padding: 10px 24px; background: #ffd700; color: #000; border: none; \
                               border-radius: 6px; cursor: pointer; font-size: 14px; font-weight: bold;"
                    >
                        {move || if editing.get().is_some() { "Update" } else { "Save" }}
                    </button>
                </div>
            </div>

            // Notes grid
            <div style="max-width: 800px; margin: 0 auto; display: grid; \
                        grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px;">
                <For
                    each=move || notes.get()
                    key=|n| n.id
                    children=move |note: Note| {
                        let id = note.id;
                        let title_text = note.title.clone();
                        let content_text = note.content.clone();
                        let bg = note.color.clone();
                        view! {
                            <div style=move || format!(
                                "background: {}; padding: 16px; border-radius: 10px; \
                                 border: 1px solid #333; cursor: pointer;", bg
                            )>
                                <div style="display: flex; justify-content: space-between; \
                                            margin-bottom: 8px;">
                                    <h3 style="font-size: 16px; color: #ffd700;">{title_text}</h3>
                                    <div style="display: flex; gap: 4px;">
                                        <button
                                            on:click=move |_| edit_note(id)
                                            style="background: none; border: none; color: #888; \
                                                   cursor: pointer; font-size: 14px;"
                                        >"\u{270E}"</button>
                                        <button
                                            on:click=move |_| delete_note(id)
                                            style="background: none; border: none; color: #f85149; \
                                                   cursor: pointer; font-size: 14px;"
                                        >"\u{2715}"</button>
                                    </div>
                                </div>
                                <p style="font-size: 13px; color: #ccc; line-height: 1.5; \
                                          white-space: pre-wrap; word-break: break-word;">
                                    {content_text}
                                </p>
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
