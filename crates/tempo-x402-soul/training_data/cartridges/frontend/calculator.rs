use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (display, set_display) = create_signal("0".to_string());
    let (first_operand, set_first_operand) = create_signal(Option::<f64>::None);
    let (operator, set_operator) = create_signal(Option::<String>::None);
    let (reset_on_next, set_reset_on_next) = create_signal(false);

    let press_digit = move |d: &'static str| {
        move |_| {
            if reset_on_next.get() {
                set_display.set(d.to_string());
                set_reset_on_next.set(false);
            } else if display.get() == "0" && d != "." {
                set_display.set(d.to_string());
            } else if d == "." && display.get().contains('.') {
                // ignore duplicate decimal
            } else {
                set_display.update(|s| s.push_str(d));
            }
        }
    };

    let calculate = move |a: f64, op: &str, b: f64| -> f64 {
        match op {
            "+" => a + b,
            "-" => a - b,
            "\u{00d7}" => a * b,
            "\u{00f7}" => if b != 0.0 { a / b } else { f64::NAN },
            _ => b,
        }
    };

    let press_operator = move |op: String| {
        move |_| {
            let current: f64 = display.get().parse().unwrap_or(0.0);
            if let (Some(first), Some(prev_op)) = (first_operand.get(), operator.get()) {
                let result = calculate(first, &prev_op, current);
                let s = if result == result.floor() && result.abs() < 1e15 {
                    format!("{}", result as i64)
                } else {
                    format!("{:.8}", result).trim_end_matches('0').trim_end_matches('.').to_string()
                };
                set_display.set(s);
                set_first_operand.set(Some(result));
            } else {
                set_first_operand.set(Some(current));
            }
            set_operator.set(Some(op.clone()));
            set_reset_on_next.set(true);
        }
    };

    let press_equals = move |_| {
        let current: f64 = display.get().parse().unwrap_or(0.0);
        if let (Some(first), Some(op)) = (first_operand.get(), operator.get()) {
            let result = calculate(first, &op, current);
            let s = if result.is_nan() {
                "Error".to_string()
            } else if result == result.floor() && result.abs() < 1e15 {
                format!("{}", result as i64)
            } else {
                format!("{:.8}", result).trim_end_matches('0').trim_end_matches('.').to_string()
            };
            set_display.set(s);
            set_first_operand.set(None);
            set_operator.set(None);
            set_reset_on_next.set(true);
        }
    };

    let press_clear = move |_| {
        set_display.set("0".to_string());
        set_first_operand.set(None);
        set_operator.set(None);
        set_reset_on_next.set(false);
    };

    let press_negate = move |_| {
        let val: f64 = display.get().parse().unwrap_or(0.0);
        if val != 0.0 {
            let negated = -val;
            let s = if negated == negated.floor() && negated.abs() < 1e15 {
                format!("{}", negated as i64)
            } else {
                format!("{}", negated)
            };
            set_display.set(s);
        }
    };

    let press_percent = move |_| {
        let val: f64 = display.get().parse().unwrap_or(0.0);
        let result = val / 100.0;
        let s = format!("{:.8}", result).trim_end_matches('0').trim_end_matches('.').to_string();
        set_display.set(s);
    };

    let btn = "width: 60px; height: 60px; font-size: 20px; border: 1px solid #222; \
               background: #1a1a2e; color: #e0e0e0; cursor: pointer; border-radius: 8px;";
    let op_btn = "width: 60px; height: 60px; font-size: 20px; border: 1px solid #222; \
                  background: #c792ea; color: #0a0a0a; cursor: pointer; border-radius: 8px; font-weight: bold;";
    let fn_btn = "width: 60px; height: 60px; font-size: 16px; border: 1px solid #222; \
                  background: #333; color: #e0e0e0; cursor: pointer; border-radius: 8px;";

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center;">
            <div style="background: #111; border-radius: 16px; padding: 20px; \
                        border: 1px solid #222; width: 280px;">
                <div style="background: #0a0a0a; border-radius: 8px; padding: 16px; \
                            margin-bottom: 16px; text-align: right; min-height: 40px; \
                            font-size: 32px; color: #7fdbca; font-variant-numeric: tabular-nums; \
                            overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                    {display}
                </div>
                <div style="display: grid; grid-template-columns: repeat(4, 60px); gap: 8px; \
                            justify-content: center;">
                    <button style=fn_btn on:click=press_clear>"AC"</button>
                    <button style=fn_btn on:click=press_negate>"+/-"</button>
                    <button style=fn_btn on:click=press_percent>"%"</button>
                    <button style=op_btn on:click=press_operator("\u{00f7}".to_string())>"\u{00f7}"</button>

                    <button style=btn on:click=press_digit("7")>"7"</button>
                    <button style=btn on:click=press_digit("8")>"8"</button>
                    <button style=btn on:click=press_digit("9")>"9"</button>
                    <button style=op_btn on:click=press_operator("\u{00d7}".to_string())>"\u{00d7}"</button>

                    <button style=btn on:click=press_digit("4")>"4"</button>
                    <button style=btn on:click=press_digit("5")>"5"</button>
                    <button style=btn on:click=press_digit("6")>"6"</button>
                    <button style=op_btn on:click=press_operator("-".to_string())>"-"</button>

                    <button style=btn on:click=press_digit("1")>"1"</button>
                    <button style=btn on:click=press_digit("2")>"2"</button>
                    <button style=btn on:click=press_digit("3")>"3"</button>
                    <button style=op_btn on:click=press_operator("+".to_string())>"+"</button>

                    <button style="width: 128px; height: 60px; font-size: 20px; border: 1px solid #222; \
                                   background: #1a1a2e; color: #e0e0e0; cursor: pointer; border-radius: 8px; \
                                   grid-column: span 2;"
                            on:click=press_digit("0")>"0"</button>
                    <button style=btn on:click=press_digit(".")>"."</button>
                    <button style=op_btn on:click=press_equals>"="</button>
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
