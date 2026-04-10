use leptos::*;
use wasm_bindgen::prelude::*;

fn calculate_entropy(password: &str) -> f64 {
    let mut charset_size = 0u32;
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());
    if has_lower { charset_size += 26; }
    if has_upper { charset_size += 26; }
    if has_digit { charset_size += 10; }
    if has_special { charset_size += 32; }
    if charset_size == 0 { return 0.0; }
    let bits_per_char = (charset_size as f64).ln() / (2.0f64).ln();
    bits_per_char * password.len() as f64
}

#[component]
fn App() -> impl IntoView {
    let (password, set_password) = create_signal(String::new());
    let (show_password, set_show_password) = create_signal(false);

    let analysis = move || {
        let pw = password.get();
        let len = pw.len();
        let has_lower = pw.chars().any(|c| c.is_ascii_lowercase());
        let has_upper = pw.chars().any(|c| c.is_ascii_uppercase());
        let has_digit = pw.chars().any(|c| c.is_ascii_digit());
        let has_special = pw.chars().any(|c| !c.is_alphanumeric());
        let entropy = calculate_entropy(&pw);

        let unique_chars = {
            let mut chars: Vec<char> = pw.chars().collect();
            chars.sort();
            chars.dedup();
            chars.len()
        };
        let variety = if len > 0 { unique_chars as f64 / len as f64 } else { 0.0 };

        let mut score = 0u32;
        if len >= 8 { score += 1; }
        if len >= 12 { score += 1; }
        if has_lower { score += 1; }
        if has_upper { score += 1; }
        if has_digit { score += 1; }
        if has_special { score += 1; }
        if variety > 0.6 { score += 1; }
        if entropy > 50.0 { score += 1; }

        let (label, color) = if len == 0 {
            ("", "#333")
        } else if score <= 2 {
            ("Very Weak", "#e06c75")
        } else if score <= 3 {
            ("Weak", "#d19a66")
        } else if score <= 5 {
            ("Fair", "#e5c07b")
        } else if score <= 6 {
            ("Strong", "#98c379")
        } else {
            ("Very Strong", "#7fdbca")
        };

        (len, has_lower, has_upper, has_digit, has_special, entropy, score, label, color, variety)
    };

    let check_style = move |met: bool| {
        format!("color: {}; font-size: 14px;", if met { "#98c379" } else { "#555" })
    };

    let check_mark = move |met: bool| {
        if met { "\u{2713} " } else { "\u{2717} " }
    };

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center; gap: 24px; padding: 40px 20px;">
            <h1 style="font-size: 28px; color: #7fdbca; margin: 0;">"Password Strength"</h1>

            <div style="width: 100%; max-width: 400px; display: flex; flex-direction: column; \
                        gap: 16px;">
                <div style="position: relative;">
                    <input
                        type=move || if show_password.get() { "text" } else { "password" }
                        placeholder="Enter a password..."
                        style="width: 100%; padding: 14px; padding-right: 60px; background: #111; \
                               border: 1px solid #333; color: #e0e0e0; border-radius: 8px; \
                               font-size: 16px; outline: none;"
                        prop:value=password
                        on:input=move |ev| set_password.set(event_target_value(&ev))
                    />
                    <button
                        style="position: absolute; right: 8px; top: 50%; transform: translateY(-50%); \
                               background: none; border: none; color: #666; cursor: pointer; \
                               font-size: 13px; padding: 4px 8px;"
                        on:click=move |_| set_show_password.update(|s| *s = !*s)
                    >
                        {move || if show_password.get() { "Hide" } else { "Show" }}
                    </button>
                </div>

                // Strength bar
                <div style="width: 100%; height: 8px; background: #1a1a2e; border-radius: 4px; \
                            overflow: hidden;">
                    <div style=move || {
                        let a = analysis();
                        let pct = (a.6 as f64 / 8.0 * 100.0).min(100.0);
                        format!("height: 100%; width: {}%; background: {}; border-radius: 4px; \
                                 transition: width 0.3s, background 0.3s;", pct, a.8)
                    }></div>
                </div>

                <div style="text-align: center;">
                    <span style=move || format!("font-size: 18px; font-weight: bold; color: {};", analysis().8)>
                        {move || analysis().7}
                    </span>
                </div>

                // Requirements checklist
                <div style="background: #111; border-radius: 8px; padding: 16px; \
                            display: flex; flex-direction: column; gap: 8px;">
                    <div style=move || check_style(analysis().0 >= 8)>
                        {move || check_mark(analysis().0 >= 8)} "At least 8 characters"
                    </div>
                    <div style=move || check_style(analysis().0 >= 12)>
                        {move || check_mark(analysis().0 >= 12)} "At least 12 characters"
                    </div>
                    <div style=move || check_style(analysis().1)>
                        {move || check_mark(analysis().1)} "Lowercase letter (a-z)"
                    </div>
                    <div style=move || check_style(analysis().2)>
                        {move || check_mark(analysis().2)} "Uppercase letter (A-Z)"
                    </div>
                    <div style=move || check_style(analysis().3)>
                        {move || check_mark(analysis().3)} "Number (0-9)"
                    </div>
                    <div style=move || check_style(analysis().4)>
                        {move || check_mark(analysis().4)} "Special character (!@#...)"
                    </div>
                    <div style=move || check_style(analysis().9 > 0.6)>
                        {move || check_mark(analysis().9 > 0.6)} "Good character variety"
                    </div>
                </div>

                // Stats
                <div style="display: flex; justify-content: space-between; color: #555; \
                            font-size: 13px; padding: 0 4px;">
                    <span>{move || format!("Length: {}", analysis().0)}</span>
                    <span>{move || format!("Entropy: {:.0} bits", analysis().5)}</span>
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
