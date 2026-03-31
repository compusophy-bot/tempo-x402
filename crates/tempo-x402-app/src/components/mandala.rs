use crate::api;
use crate::WalletState;
use gloo_timers::callback::Interval;
use leptos::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::wallet_panel::WalletButtons;

/// Event from SSE stream, parsed into a usable struct.
#[derive(Clone, Debug)]
struct SoulEventMsg {
    code: String,
    _level: String,
    message: String,
    timestamp: i64,
}

/// Neural Mandala — event-driven intelligence visualization.
/// Every visual change corresponds to a real cognitive event.
#[component]
pub fn Mandala() -> impl IntoView {
    let (wallet, set_wallet) =
        expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();

    // ── Data signals (polled every 10s for state) ──
    let (soul, set_soul) = create_signal(None::<serde_json::Value>);
    let (info, set_info) = create_signal(None::<serde_json::Value>);
    let (system, set_system) = create_signal(None::<serde_json::Value>);
    let (panel_open, set_panel_open) = create_signal(false);
    let (clone_loading, set_clone_loading) = create_signal(false);
    let (clone_result, set_clone_result) = create_signal(None::<Result<String, String>>);

    // ── Event-driven signals (from SSE) ──
    let (events, set_events) = create_signal(Vec::<SoulEventMsg>::new());
    // Track last-fired timestamp per event code prefix for connection pulses
    let (pulses, set_pulses) = create_signal(std::collections::HashMap::<String, f64>::new());

    // ── History buffers for sparkline rings ──
    let (psi_history, set_psi_history) = create_signal(Vec::<f64>::new());
    let (fe_history, set_fe_history) = create_signal(Vec::<f64>::new());
    let (fitness_history, set_fitness_history) = create_signal(Vec::<f64>::new());

    // ── Fetch state data (polling, not events) ──
    let fetch_all = move || {
        spawn_local(async move {
            let base = api::gateway_base_url();
            if let Ok(resp) = gloo_net::http::Request::get(&format!("{}/instance/info", base))
                .send().await
            {
                if resp.ok() {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        set_info.set(Some(data));
                    }
                }
            }
            if let Ok(data) = api::fetch_soul_status().await {
                let psi = data.get("role").and_then(|r| r.get("psi")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let fe = data.get("free_energy").and_then(|f| f.get("F")).and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                let fit = data.get("fitness").and_then(|f| f.get("total")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                set_psi_history.update(|h| { h.push(psi); if h.len() > 60 { h.drain(..h.len()-60); } });
                set_fe_history.update(|h| { h.push(fe); if h.len() > 60 { h.drain(..h.len()-60); } });
                set_fitness_history.update(|h| { h.push(fit); if h.len() > 60 { h.drain(..h.len()-60); } });
                set_soul.set(Some(data));
            }
            if let Ok(resp) = gloo_net::http::Request::get(&format!("{}/soul/system", base))
                .send().await
            {
                if resp.ok() {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        set_system.set(Some(data));
                    }
                }
            }
        });
    };

    fetch_all();
    let interval = Interval::new(10_000, move || { fetch_all(); });
    on_cleanup(move || drop(interval));

    // ── SSE EventSource subscription ──
    {
        let base = api::gateway_base_url().to_string();
        spawn_local(async move {
            let url = format!("{}/soul/events/stream", base);
            let es = match web_sys::EventSource::new(&url) {
                Ok(es) => es,
                Err(_) => return,
            };

            let on_msg = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                let data_str = ev.data().as_string().unwrap_or_default();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data_str) {
                    let code = parsed.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let level = parsed.get("level").and_then(|v| v.as_str()).unwrap_or("info").to_string();
                    let message = parsed.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let timestamp = parsed.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);

                    if !code.is_empty() && code != "heartbeat" {
                        // Add to event log (cap at 30)
                        set_events.update(|evts| {
                            evts.push(SoulEventMsg { code: code.clone(), _level: level, message, timestamp });
                            if evts.len() > 30 { evts.drain(..evts.len() - 30); }
                        });

                        // Record pulse for connection visualization
                        let now = js_sys::Date::now();
                        set_pulses.update(|p| {
                            p.insert(code, now);
                        });
                    }
                }
            });

            es.add_event_listener_with_callback("soul_event", on_msg.as_ref().unchecked_ref()).ok();
            on_msg.forget(); // Leak closure — lives for page lifetime
        });
    }

    // Layout constants
    let cx = 400.0f64;
    let cy = 400.0f64;
    let r_inner = 100.0f64;
    let r_outer = 185.0f64;
    let r_colony = 280.0f64;
    let r_spark_psi = 225.0f64;
    let r_spark_fe = 238.0f64;
    let r_spark_fit = 251.0f64;

    // 9 cognitive systems with their associated event code prefixes
    let systems: [(& str, usize, &str); 9] = [
        ("BRAIN", 0, "brain"),
        ("CORTEX", 1, "cortex"),
        ("GENESIS", 2, "genesis"),
        ("HIVEMND", 3, "hivemind"),
        ("SYNTH", 4, "synthesis"),
        ("EVAL", 5, "evaluation"),
        ("AUTON", 6, "autonomy"),
        ("FREE-E", 7, "free_energy"),
        ("FEEDBACK", 8, "plan"),
    ];

    // 4 models with their event prefixes
    let models: [(&str, usize, &str); 4] = [
        ("brain", 0, "brain.trained"),
        ("xformer", 1, "transformer.trained"),
        ("quality", 2, "quality"),
        ("codegen", 3, "codegen.trained"),
    ];

    view! {
        <div class="mandala-container">
            <svg viewBox="0 0 800 800" class="mandala-svg" preserveAspectRatio="xMidYMid meet">
                <defs>
                    <filter id="glow-green">
                        <feGaussianBlur stdDeviation="4" result="blur"/>
                        <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
                    </filter>
                    <filter id="glow-pulse">
                        <feGaussianBlur stdDeviation="6" result="blur"/>
                        <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
                    </filter>
                    <radialGradient id="psi-grad" cx="50%" cy="50%" r="50%">
                        <stop offset="0%" stop-color="#00ff41" stop-opacity="0.6"/>
                        <stop offset="100%" stop-color="#00ff41" stop-opacity="0"/>
                    </radialGradient>
                </defs>

                // ── Background rings (subtle) ──
                {(0..8).map(|i| {
                    let r = (i as f64 + 1.0) * 40.0;
                    view! {
                        <circle cx=cx.to_string() cy=cy.to_string() r=r.to_string()
                            fill="none" stroke="#08081a" stroke-width="0.5"/>
                    }
                }).collect::<Vec<_>>()}

                // ── Connections: center ↔ models (event-driven brightness) ──
                {move || {
                    let p = pulses.get();
                    let now = js_sys::Date::now();
                    models.iter().map(|(_, i, evt_code)| {
                        let angle = (*i as f64) * std::f64::consts::TAU / 4.0 - std::f64::consts::FRAC_PI_2;
                        let mx = cx + r_inner * angle.cos();
                        let my = cy + r_inner * angle.sin();
                        let intensity = pulse_intensity(&p, evt_code, now);
                        let opacity = 0.08 + intensity * 0.7;
                        let width = 0.5 + intensity * 2.5;
                        let color = if intensity > 0.3 { "#00ff41" } else { "#00ff41" };
                        view! {
                            <line x1=cx.to_string() y1=cy.to_string()
                                x2=mx.to_string() y2=my.to_string()
                                stroke=color stroke-width=width.to_string()
                                stroke-opacity=opacity.to_string()
                                {..} />
                        }
                    }).collect::<Vec<_>>()
                }}

                // ── Connections: systems ↔ models (event-driven) ──
                {move || {
                    let p = pulses.get();
                    let now = js_sys::Date::now();
                    systems.iter().map(|(_, i, evt_prefix)| {
                        let angle = (*i as f64) * std::f64::consts::TAU / 9.0 - std::f64::consts::FRAC_PI_2;
                        let sx = cx + r_outer * angle.cos();
                        let sy = cy + r_outer * angle.sin();
                        let mi = *i % 4;
                        let m_angle = (mi as f64) * std::f64::consts::TAU / 4.0 - std::f64::consts::FRAC_PI_2;
                        let mx = cx + r_inner * m_angle.cos();
                        let my = cy + r_inner * m_angle.sin();
                        let intensity = pulse_intensity(&p, evt_prefix, now);
                        let opacity = 0.05 + intensity * 0.5;
                        let width = 0.3 + intensity * 1.5;
                        view! {
                            <line x1=sx.to_string() y1=sy.to_string()
                                x2=mx.to_string() y2=my.to_string()
                                stroke="#00e5ff" stroke-width=width.to_string()
                                stroke-opacity=opacity.to_string()
                                {..} />
                        }
                    }).collect::<Vec<_>>()
                }}

                // ── Sparkline rings (only render when enough data to look like curves, not polygons) ──
                {move || {
                    let h = psi_history.get();
                    if h.len() >= 15 { render_sparkline_ring(&h, cx, cy, r_spark_psi, "#00ff41", 0.4) }
                    else { view! { <g></g> }.into_view() }
                }}
                {move || {
                    let h = fe_history.get();
                    if h.len() >= 15 { render_sparkline_ring(&h, cx, cy, r_spark_fe, "#00e5ff", 0.3) }
                    else { view! { <g></g> }.into_view() }
                }}
                {move || {
                    let h = fitness_history.get();
                    if h.len() >= 15 { render_sparkline_ring(&h, cx, cy, r_spark_fit, "#ffa000", 0.3) }
                    else { view! { <g></g> }.into_view() }
                }}

                // ── α compass (above center orb) ──
                {move || {
                    let s = soul.get().unwrap_or_default();
                    let accel = s.get("acceleration");
                    let alpha_str = accel.and_then(|a| a.get("alpha")).and_then(|v| v.as_str()).unwrap_or("0.0000");
                    let alpha: f64 = alpha_str.parse().unwrap_or(0.0);
                    let regime = accel.and_then(|a| a.get("regime")).and_then(|v| v.as_str()).unwrap_or("STALLED");
                    let (color, symbol) = match regime {
                        "ACCELERATING" => ("#00ff41", "\u{25B2}"),  // green ▲
                        "CRUISING" => ("#00e5ff", "\u{25C6}"),       // cyan ◆
                        "DECELERATING" => ("#ff1744", "\u{25BC}"),   // red ▼
                        _ => ("#5a6a5a", "\u{25CB}"),                // dim ○
                    };
                    view! {
                        <text x=cx.to_string() y="60" text-anchor="middle"
                            class="mandala-text-alpha" fill=color>
                            {format!("{} \u{03B1}={:+.4}", symbol, alpha)}
                        </text>
                        <text x=cx.to_string() y="74" text-anchor="middle"
                            class="mandala-text-regime" fill=color opacity="0.6">
                            {regime.to_string()}
                        </text>
                    }
                }}

                // ── Ψ center orb ──
                {move || {
                    let s = soul.get().unwrap_or_default();
                    let role = s.get("role");
                    let psi = role.and_then(|r| r.get("psi")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let fe = s.get("free_energy");
                    let regime = fe.and_then(|f| f.get("regime")).and_then(|v| v.as_str()).unwrap_or("EXPLOIT");
                    let f_val = fe.and_then(|f| f.get("F")).and_then(|v| v.as_str()).unwrap_or("--");

                    let orb_r = 25.0 + psi * 30.0;
                    let (orb_color, glow_color) = match regime {
                        "EXPLORE" => ("#00e5ff", "#003344"),
                        "LEARN" => ("#b388ff", "#2a1a44"),
                        "EXPLOIT" => ("#00ff41", "#003311"),
                        "ANOMALY" => ("#ff1744", "#440011"),
                        _ => ("#00ff41", "#003311"),
                    };
                    let iq = s.get("benchmark").and_then(|b| b.get("opus_iq")).and_then(|v| v.as_str()).unwrap_or("--");

                    view! {
                        <circle cx=cx.to_string() cy=cy.to_string() r=(orb_r * 2.5).to_string()
                            fill="url(#psi-grad)" opacity="0.3"/>
                        <circle cx=cx.to_string() cy=cy.to_string() r=orb_r.to_string()
                            fill=glow_color stroke=orb_color stroke-width="2"
                            filter="url(#glow-green)" class="psi-orb"/>
                        <circle cx=(cx - orb_r * 0.2).to_string() cy=(cy - orb_r * 0.2).to_string()
                            r=(orb_r * 0.3).to_string()
                            fill=orb_color opacity="0.3"/>
                        <text x=cx.to_string() y=(cy - 2.0).to_string()
                            text-anchor="middle" class="mandala-text-psi" fill=orb_color>
                            {format!("\u{03A8} {:.3}", psi)}
                        </text>
                        <text x=cx.to_string() y=(cy + 12.0).to_string()
                            text-anchor="middle" class="mandala-text-small" fill="#ffffff" opacity="0.5">
                            {format!("F={} {}", f_val, regime)}
                        </text>
                        <text x=cx.to_string() y=(cy - orb_r - 12.0).to_string()
                            text-anchor="middle" class="mandala-text-iq" fill=orb_color>
                            {format!("IQ {}", iq)}
                        </text>
                    }
                }}

                // ── Model ring (inner) ──
                {move || {
                    let s = soul.get().unwrap_or_default();
                    let p = pulses.get();
                    let now = js_sys::Date::now();
                    models.iter().map(|(name, i, evt_code)| {
                        let angle = (*i as f64) * std::f64::consts::TAU / 4.0 - std::f64::consts::FRAC_PI_2;
                        let mx = cx + r_inner * angle.cos();
                        let my = cy + r_inner * angle.sin();
                        let pulse = pulse_intensity(&p, evt_code, now);

                        let (node_r, color, label) = match *name {
                            "brain" => {
                                let b = s.get("brain");
                                let loss = b.and_then(|b| b.get("running_loss")).and_then(|v| v.as_f64()).unwrap_or(1.0);
                                let steps = b.and_then(|b| b.get("train_steps")).and_then(|v| v.as_u64()).unwrap_or(0);
                                let brightness = (1.0 - loss.min(1.0)) * 0.8 + 0.2 + pulse * 0.3;
                                (8.0 + (steps as f64 / 5000.0).min(8.0), format!("rgba(0,255,65,{:.2})", brightness.min(1.0)), format!("{}K", steps/1000))
                            }
                            "xformer" => {
                                let t = s.get("transformer");
                                let loss = t.and_then(|t| t.get("last_train_loss")).and_then(|v| v.as_f64()).unwrap_or(2.0);
                                let steps = t.and_then(|t| t.get("train_steps")).and_then(|v| v.as_u64()).unwrap_or(0);
                                let brightness = (1.0 - (loss / 2.0).min(1.0)) * 0.8 + 0.2 + pulse * 0.3;
                                (8.0 + (steps as f64 / 1000.0).min(8.0), format!("rgba(0,229,255,{:.2})", brightness.min(1.0)), format!("{}K", steps/1000))
                            }
                            "quality" => {
                                let q = s.get("quality");
                                let steps = q.and_then(|q| q.get("train_steps")).and_then(|v| v.as_u64()).unwrap_or(0);
                                (6.0 + (steps as f64 / 500.0).min(6.0), format!("rgba(179,136,255,{:.2})", 0.6 + pulse * 0.3), format!("{}s", steps))
                            }
                            "codegen" => {
                                let cg = s.get("codegen");
                                let steps = cg.and_then(|c| c.get("model_steps")).and_then(|v| v.as_u64()).unwrap_or(0);
                                let sols = cg.and_then(|c| c.get("solutions_stored")).and_then(|v| v.as_u64()).unwrap_or(0);
                                let can = cg.and_then(|c| c.get("can_generate")).and_then(|v| v.as_bool()).unwrap_or(false);
                                let base_color = if can { 0.8 } else if sols > 0 { 0.5 } else { 0.2 };
                                let brightness = base_color + pulse * 0.3;
                                let color = if can { format!("rgba(0,255,65,{:.2})", brightness.min(1.0)) }
                                    else if sols > 0 { format!("rgba(255,160,0,{:.2})", brightness.min(1.0)) }
                                    else { format!("rgba(255,23,68,{:.2})", brightness.min(1.0)) };
                                (6.0 + (steps as f64 / 50.0).min(10.0), color, format!("{}d", sols))
                            }
                            _ => (6.0, "rgba(100,100,100,0.5)".to_string(), String::new()),
                        };

                        let filter = if pulse > 0.3 { "url(#glow-pulse)" } else { "" };
                        view! {
                            <circle cx=mx.to_string() cy=my.to_string() r=node_r.to_string()
                                fill=color.clone() stroke=color stroke-width="1"
                                filter=filter />
                            <text x=mx.to_string() y=(my + node_r + 10.0).to_string()
                                text-anchor="middle" class="mandala-text-tiny" fill="#5a6a5a">
                                {name.to_string()}
                            </text>
                            <text x=mx.to_string() y=(my + node_r + 19.0).to_string()
                                text-anchor="middle" class="mandala-text-tiny" fill="#3a4a3a">
                                {label}
                            </text>
                        }
                    }).collect::<Vec<_>>()
                }}

                // ── System ring (outer) — health-driven, event-brightened ──
                {move || {
                    let s = soul.get().unwrap_or_default();
                    let p = pulses.get();
                    let now = js_sys::Date::now();
                    systems.iter().map(|(name, i, evt_prefix)| {
                        let angle = (*i as f64) * std::f64::consts::TAU / 9.0 - std::f64::consts::FRAC_PI_2;
                        let sx = cx + r_outer * angle.cos();
                        let sy = cy + r_outer * angle.sin();
                        let (node_r, color) = system_health(&s, name);
                        let pulse = pulse_intensity(&p, evt_prefix, now);
                        let opacity = 0.7 + pulse * 0.3;
                        let filter = if pulse > 0.3 { "url(#glow-pulse)" } else { "" };

                        view! {
                            <circle cx=sx.to_string() cy=sy.to_string() r=node_r.to_string()
                                fill="none" stroke=color.clone() stroke-width="1.5"
                                opacity=opacity.to_string() filter=filter />
                            <circle cx=sx.to_string() cy=sy.to_string() r=(node_r * 0.6).to_string()
                                fill=color.clone() opacity="0.15"/>
                            <text x=sx.to_string() y=(sy + 1.0).to_string()
                                text-anchor="middle" class="mandala-text-system" fill=color>
                                {name.to_string()}
                            </text>
                        }
                    }).collect::<Vec<_>>()
                }}

                // ── Colony peers ──
                {move || {
                    let d = info.get().unwrap_or_default();
                    let peers = d.get("peers").or_else(|| d.get("children"))
                        .and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    if peers.is_empty() { return vec![]; }
                    let n = peers.len();
                    peers.iter().enumerate().map(|(i, p)| {
                        let angle = (i as f64) * std::f64::consts::TAU / (n as f64) - std::f64::consts::FRAC_PI_2;
                        let px = cx + r_colony * angle.cos();
                        let py = cy + r_colony * angle.sin();
                        let status = p.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                        let color = if status == "running" { "#00ff41" } else { "#ff1744" };
                        let id = p.get("instance_id").and_then(|v| v.as_str()).unwrap_or("?");
                        let short = if id.len() > 6 { &id[..6] } else { id };
                        view! {
                            <line x1=cx.to_string() y1=cy.to_string()
                                x2=px.to_string() y2=py.to_string()
                                stroke=color stroke-width="0.3" stroke-opacity="0.1"/>
                            <circle cx=px.to_string() cy=py.to_string() r="4"
                                fill="none" stroke=color stroke-width="1" opacity="0.5"/>
                            <text x=px.to_string() y=(py + 12.0).to_string()
                                text-anchor="middle" class="mandala-text-tiny" fill="#3a4a3a">
                                {short.to_string()}
                            </text>
                        }
                    }).collect::<Vec<_>>()
                }}

                // ── Metrics overlay ──
                {move || {
                    let d = info.get().unwrap_or_default();
                    let s = soul.get().unwrap_or_default();
                    let fitness = d.get("fitness").and_then(|f| f.get("total")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let cycles = s.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                    let mode = s.get("mode").and_then(|v| v.as_str()).unwrap_or("--");
                    let active = s.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                    let bench = s.get("benchmark");
                    let pass = bench.and_then(|b| b.get("pass_at_1")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let elo = bench.and_then(|b| b.get("elo_rating")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let sys = system.get().unwrap_or_default();
                    let cpu = sys.get("cpu_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let mem = sys.get("mem_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let status_color = if active { "#00ff41" } else { "#ff1744" };

                    view! {
                        <text x="16" y="20" class="mandala-text-label" fill="#3a4a3a">
                            {format!("{} | cycle {} | cpu {:.0}% mem {:.0}%", mode, cycles, cpu, mem)}
                        </text>
                        <text x="16" y="780" class="mandala-text-label" fill="#ffa000">
                            {format!("fitness {:.0}%", fitness * 100.0)}
                        </text>
                        <text x="400" y="780" text-anchor="middle" class="mandala-text-label" fill="#5a6a5a">
                            {format!("pass@1 {:.1}% | ELO {:.0}", pass, elo)}
                        </text>
                        <text x="784" y="780" text-anchor="end" class="mandala-text-label" fill="#2a2a3a">
                            {concat!("v", env!("CARGO_PKG_VERSION"))}
                        </text>
                        <circle cx="10" cy="16" r="3" fill=status_color/>
                    }
                }}

                // ── Live event log (replaces fake orbiting particles) ──
                {move || {
                    let evts = events.get();
                    evts.iter().rev().take(8).enumerate().map(|(i, evt)| {
                        let y = 720.0 - (i as f64) * 12.0;
                        let opacity = 0.7 - (i as f64) * 0.07;
                        let color = event_color(&evt.code);
                        let abbr = event_abbr(&evt.code);
                        let msg: String = evt.message.chars().take(60).collect();
                        view! {
                            <text x="16" y=y.to_string() class="mandala-text-tiny" fill=color opacity=opacity.to_string()>
                                {format!("[{}] {}", abbr, msg)}
                            </text>
                        }
                    }).collect::<Vec<_>>()
                }}
            </svg>

            // ── Floating control panel ──
            <div class="mandala-controls">
                <button class="mandala-toggle" on:click=move |_| set_panel_open.update(|v| *v = !*v)>
                    {move || if panel_open.get() { "\u{2715}" } else { "\u{2630}" }}
                </button>
                <Show when=move || panel_open.get() fallback=|| ()>
                    <div class="mandala-panel">
                        <div class="mandala-panel-section">
                            <div class="mandala-panel-label">"ACCOUNT"</div>
                            <WalletButtons wallet=wallet set_wallet=set_wallet />
                        </div>
                        {move || {
                            let w = wallet.get();
                            if !w.connected { return view! { <div></div> }.into_view(); }
                            let addr = w.address.unwrap_or_default();
                            let short = if addr.len() > 10 { format!("{}...{}", &addr[..6], &addr[addr.len()-4..]) } else { addr };
                            view! {
                                <div class="mandala-panel-section">
                                    <div style="font-size:10px;color:var(--text-dim)">{short}</div>
                                </div>
                            }.into_view()
                        }}
                        {move || {
                            let d = info.get().unwrap_or_default();
                            let clone_available = d.get("clone_available").and_then(|v| v.as_bool()).unwrap_or(false);
                            let clone_price = d.get("clone_price").and_then(|v| v.as_str()).unwrap_or("N/A").to_string();
                            if !clone_available { return view! { <div></div> }.into_view(); }
                            let do_clone = move |_: web_sys::MouseEvent| {
                                if clone_loading.get() { return; }
                                let w = wallet.get();
                                if !w.connected { return; }
                                set_clone_loading.set(true);
                                set_clone_result.set(None);
                                spawn_local(async move {
                                    match api::clone_instance(&w).await {
                                        Ok(resp) => {
                                            let msg = format!("Clone {} at {}", resp.instance_id.unwrap_or_default(), resp.url.unwrap_or_default());
                                            set_clone_result.set(Some(Ok(msg)));
                                        }
                                        Err(e) => set_clone_result.set(Some(Err(e))),
                                    }
                                    set_clone_loading.set(false);
                                });
                            };
                            view! {
                                <div class="mandala-panel-section">
                                    <div class="mandala-panel-label">"CLONE"</div>
                                    <button class="btn btn-primary" on:click=do_clone
                                        disabled=move || clone_loading.get() || !wallet.get().connected>
                                        {move || if clone_loading.get() { "Cloning..." } else { "Clone Node" }}
                                    </button>
                                    <div style="font-size:9px;color:var(--text-muted);margin-top:2px">{format!("${}", clone_price)}</div>
                                    {move || clone_result.get().map(|r| match r {
                                        Ok(msg) => view! { <div style="font-size:9px;color:var(--green);margin-top:4px">{msg}</div> }.into_view(),
                                        Err(e) => view! { <div style="font-size:9px;color:var(--red);margin-top:4px">{e}</div> }.into_view(),
                                    })}
                                </div>
                            }.into_view()
                        }}
                        <div class="mandala-panel-section">
                            <div class="mandala-panel-label">"NAVIGATE"</div>
                            <a href="/dashboard" class="mandala-nav-link">"Dashboard"</a>
                            <a href="/studio" class="mandala-nav-link">"Studio"</a>
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}

// ── Helper functions ──

/// How bright a connection should be based on recent events (0.0 = baseline, 1.0 = just fired).
fn pulse_intensity(pulses: &std::collections::HashMap<String, f64>, prefix: &str, now: f64) -> f64 {
    let last = pulses.iter()
        .filter(|(code, _)| code.starts_with(prefix))
        .map(|(_, ts)| *ts)
        .fold(0.0f64, f64::max);
    if last == 0.0 { return 0.0; }
    let age_ms = now - last;
    if age_ms < 0.0 { return 1.0; }
    // Fade over 10 seconds
    (1.0 - (age_ms / 10_000.0)).max(0.0)
}

fn event_color(code: &str) -> &'static str {
    if code.starts_with("brain") { "#00ff41" }
    else if code.starts_with("transformer") { "#00e5ff" }
    else if code.starts_with("codegen") { "#ffa000" }
    else if code.starts_with("plan.step") { "#00e5ff" }
    else if code.starts_with("plan") { "#b388ff" }
    else if code.starts_with("benchmark") { "#00ff41" }
    else if code.starts_with("peer") { "#00e5ff" }
    else if code.starts_with("system") { "#5a6a5a" }
    else { "#3a4a3a" }
}

fn event_abbr(code: &str) -> &'static str {
    if code.starts_with("brain.trained") { "BRAIN" }
    else if code.starts_with("transformer") { "XFORM" }
    else if code.starts_with("codegen") { "COGEN" }
    else if code.starts_with("plan.step.completed") { "STEP+" }
    else if code.starts_with("plan.step.failed") { "STEP!" }
    else if code.starts_with("plan.step.started") { "STEP>" }
    else if code.starts_with("plan.completed") { "PLAN+" }
    else if code.starts_with("plan.failed") { "PLAN!" }
    else if code.starts_with("benchmark") { "BENCH" }
    else if code.starts_with("peer") { "PEER" }
    else if code.starts_with("acceleration") { "ACCEL" }
    else { "EVENT" }
}

fn render_sparkline_ring(data: &[f64], cx: f64, cy: f64, radius: f64, color: &str, opacity: f64) -> leptos::View {
    if data.len() < 2 {
        return view! { <g></g> }.into_view();
    }
    let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = (max - min).max(0.001);
    let n = data.len();
    let arc_span = std::f64::consts::TAU * 0.75;
    let start_angle = std::f64::consts::FRAC_PI_2 + std::f64::consts::FRAC_PI_4;
    let points: Vec<String> = data.iter().enumerate().map(|(i, v)| {
        let t = i as f64 / (n - 1) as f64;
        let angle = start_angle + t * arc_span;
        let norm = (v - min) / range;
        let r = radius + norm * 15.0 - 7.5;
        format!("{:.1},{:.1}", cx + r * angle.cos(), cy + r * angle.sin())
    }).collect();
    let path_d = format!("M {} L {}", points[0], points[1..].join(" L "));
    let color = color.to_string();
    let opacity = opacity.to_string();
    view! {
        <path d=path_d fill="none" stroke=color stroke-width="1" opacity=opacity stroke-linecap="round"/>
    }.into_view()
}

fn system_health(soul: &serde_json::Value, name: &str) -> (f64, String) {
    match name {
        "BRAIN" => {
            let loss = soul.get("brain").and_then(|b| b.get("running_loss")).and_then(|v| v.as_f64()).unwrap_or(1.0);
            (10.0 + (1.0 - loss.min(1.0)) * 6.0, health_color(1.0 - loss.min(1.0)))
        }
        "CORTEX" => {
            let acc = soul.get("cortex").and_then(|c| c.get("prediction_accuracy")).and_then(|v| v.as_str())
                .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok()).unwrap_or(0.0) / 100.0;
            (10.0 + acc * 6.0, health_color(acc))
        }
        "GENESIS" => {
            let gen = soul.get("genesis").and_then(|g| g.get("generation")).and_then(|v| v.as_u64()).unwrap_or(0);
            let h = (gen as f64 / 200.0).min(1.0);
            (10.0 + h * 6.0, health_color(h))
        }
        "HIVEMND" => {
            let trails = soul.get("hivemind").and_then(|h| h.get("total_trails")).and_then(|v| v.as_u64()).unwrap_or(0);
            let h = (trails as f64 / 100.0).min(1.0);
            (10.0 + h * 6.0, health_color(h))
        }
        "SYNTH" => {
            let state = soul.get("synthesis").and_then(|s| s.get("state")).and_then(|v| v.as_str()).unwrap_or("--");
            let h = match state { "coherent" | "exploiting" => 0.9, "exploring" => 0.6, "conflicted" => 0.3, "stuck" => 0.1, _ => 0.5 };
            (10.0 + h * 6.0, health_color(h))
        }
        "EVAL" => {
            let rec = soul.get("evaluation").and_then(|e| e.get("total_records")).and_then(|v| v.as_u64()).unwrap_or(0);
            let h = (rec as f64 / 100.0).min(1.0);
            (10.0 + h * 6.0, health_color(h))
        }
        "FREE-E" => {
            let f = soul.get("free_energy").and_then(|f| f.get("F")).and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);
            (10.0 + (1.0 - f.min(1.0)) * 6.0, health_color(1.0 - f.min(1.0)))
        }
        _ => (10.0, "#3a4a3a".to_string()),
    }
}

fn health_color(health: f64) -> String {
    if health > 0.7 { format!("rgba(0,255,65,{:.2})", 0.4 + health * 0.4) }
    else if health > 0.4 { format!("rgba(255,160,0,{:.2})", 0.4 + health * 0.3) }
    else { format!("rgba(255,23,68,{:.2})", 0.3 + health * 0.3) }
}
