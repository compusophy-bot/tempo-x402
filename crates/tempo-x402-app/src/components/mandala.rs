use crate::api;
use crate::WalletState;
use gloo_timers::callback::Interval;
use leptos::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::wallet_panel::WalletButtons;

#[derive(Clone, Debug)]
struct SoulEventMsg { code: String, message: String }

/// The Engine Bay — first-principles visualization of the cognitive architecture.
/// Each system rendered as what it IS: neural net as layers, causal graph as nodes, trails as trails.
#[component]
pub fn Mandala() -> impl IntoView {
    let (wallet, set_wallet) = expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();
    let (soul, set_soul) = create_signal(None::<serde_json::Value>);
    let (info, set_info) = create_signal(None::<serde_json::Value>);
    let (system, set_system) = create_signal(None::<serde_json::Value>);
    let (panel_open, set_panel_open) = create_signal(false);
    let (clone_loading, set_clone_loading) = create_signal(false);
    let (clone_result, set_clone_result) = create_signal(None::<Result<String, String>>);
    let (events, set_events) = create_signal(Vec::<SoulEventMsg>::new());
    let (pulses, set_pulses) = create_signal(std::collections::HashMap::<String, f64>::new());

    // Fetch
    let fetch_all = move || {
        spawn_local(async move {
            let base = api::gateway_base_url();
            if let Ok(r) = gloo_net::http::Request::get(&format!("{}/instance/info", base)).send().await {
                if r.ok() { if let Ok(d) = r.json::<serde_json::Value>().await { set_info.set(Some(d)); } }
            }
            if let Ok(d) = api::fetch_soul_status().await { set_soul.set(Some(d)); }
            if let Ok(r) = gloo_net::http::Request::get(&format!("{}/soul/system", base)).send().await {
                if r.ok() { if let Ok(d) = r.json::<serde_json::Value>().await { set_system.set(Some(d)); } }
            }
        });
    };
    fetch_all();
    let interval = Interval::new(10_000, move || { fetch_all(); });
    on_cleanup(move || drop(interval));

    // SSE
    {
        let base = api::gateway_base_url().to_string();
        spawn_local(async move {
            let es = match web_sys::EventSource::new(&format!("{}/soul/events/stream", base)) { Ok(e) => e, Err(_) => return };
            let on_msg = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                let s = ev.data().as_string().unwrap_or_default();
                if let Ok(p) = serde_json::from_str::<serde_json::Value>(&s) {
                    let code = p.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let msg = p.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if !code.is_empty() && code != "heartbeat" {
                        set_events.update(|e| { e.push(SoulEventMsg { code: code.clone(), message: msg }); if e.len() > 20 { e.drain(..e.len()-20); } });
                        set_pulses.update(|p| { p.insert(code, js_sys::Date::now()); });
                    }
                }
            });
            es.add_event_listener_with_callback("soul_event", on_msg.as_ref().unchecked_ref()).ok();
            on_msg.forget();
        });
    }

    view! {
        <div class="engine-bay">
            // ═══ TOP BAR: IQ / ELO / α ═══
            {move || {
                let s = soul.get().unwrap_or_default();
                let b = s.get("benchmark");
                let iq = b.and_then(|b| b.get("opus_iq")).and_then(|v| v.as_str()).unwrap_or("--");
                let elo = b.and_then(|b| b.get("elo_rating")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let pass = b.and_then(|b| b.get("pass_at_1")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let a = s.get("acceleration");
                let alpha: f64 = a.and_then(|a| a.get("alpha")).and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let regime = a.and_then(|a| a.get("regime")).and_then(|v| v.as_str()).unwrap_or("STALLED");
                let (sym, cls) = match regime { "ACCELERATING" => ("\u{25B2}", "acc-up"), "DECELERATING" => ("\u{25BC}", "acc-down"), "CRUISING" => ("\u{25C6}", "acc-flat"), _ => ("\u{25CB}", "acc-stall") };
                view! {
                    <div class="eb-top">
                        <span class="eb-iq">{format!("IQ {}", iq)}</span>
                        <span class="eb-sep">"|"</span>
                        <span class="eb-stat">{format!("ELO {:.0}", elo)}</span>
                        <span class="eb-sep">"|"</span>
                        <span class="eb-stat">{format!("{:.1}%", pass)}</span>
                        <span class="eb-sep">"|"</span>
                        <span class={format!("eb-alpha {}", cls)}>{format!("{} \u{03B1}={:+.4} {}", sym, alpha, regime)}</span>
                    </div>
                }
            }}

            // ═══ MAIN: THINK (φ) | SENSE (1) ═══
            <div class="eb-main">

                // ── THINK card (brain + cortex + genesis → synthesis) ──
                <div class="eb-card">
                    <div class="eb-card-label">"THINK"</div>
                    <div class="eb-row">
                        // BRAIN
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            let b = s.get("brain");
                            let loss = b.and_then(|b| b.get("running_loss")).and_then(|v| v.as_f64()).unwrap_or(1.0);
                            let steps = b.and_then(|b| b.get("train_steps")).and_then(|v| v.as_u64()).unwrap_or(0);
                            let p = pulses.get();
                            let pulse = pulse_intensity(&p, "brain", js_sys::Date::now());
                            let glow = if pulse > 0.2 { " eb-glow" } else { "" };
                            let layers = [(32, 6), (1024, 48), (1024, 48), (23, 5)];
                            let health = (1.0 - loss.min(1.0)) * 0.7 + 0.3;
                            view! {
                                <div class={format!("eb-module{}", glow)}>
                                    <div class="eb-label">"BRAIN"</div>
                                    <svg viewBox="0 0 120 60" class="eb-svg">
                                        {layers.iter().enumerate().map(|(i, (neurons, h))| {
                                            let x = 10 + i * 28;
                                            let y = 30 - h / 2;
                                            view! {
                                                <rect x=x.to_string() y=y.to_string() width="18" height=h.to_string()
                                                    fill="#00ff41" opacity=health.to_string() rx="2"/>
                                                <text x=(x+9).to_string() y="58" text-anchor="middle" class="eb-tiny" fill="#3a4a3a">{neurons.to_string()}</text>
                                            }
                                        }).collect::<Vec<_>>()}
                                        {[0,1,2].iter().map(|i| {
                                            let x1 = 28 + i * 28; let x2 = x1 + 10;
                                            view! { <line x1=x1.to_string() y1="30" x2=x2.to_string() y2="30" stroke="#00ff41" stroke-width="0.5" opacity="0.3"/> }
                                        }).collect::<Vec<_>>()}
                                    </svg>
                                    <div class="eb-stats"><span>{format!("L={:.3} {}K", loss, steps/1000)}</span></div>
                                </div>
                            }
                        }}
                        // CORTEX
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            let c = s.get("cortex");
                            let exp = c.and_then(|c| c.get("total_experiences")).and_then(|v| v.as_u64()).unwrap_or(0);
                            let edges = c.and_then(|c| c.get("causal_edges")).and_then(|v| v.as_u64()).unwrap_or(0);
                            let acc = c.and_then(|c| c.get("prediction_accuracy")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                            let n_nodes = ((exp as f64).sqrt().min(12.0)) as usize;
                            let n_nodes = n_nodes.max(4);
                            let nodes: Vec<(f64, f64)> = (0..n_nodes).map(|i| {
                                let angle = (i as f64) * 2.399 + 0.5;
                                let r = 15.0 + ((i as f64) * 7.3) % 20.0;
                                (60.0 + r * angle.cos(), 30.0 + r * angle.sin())
                            }).collect();
                            let n_edges = (edges as usize).min(n_nodes * 2);
                            view! {
                                <div class="eb-module">
                                    <div class="eb-label">"CORTEX"</div>
                                    <svg viewBox="0 0 120 60" class="eb-svg">
                                        {(0..n_edges.min(nodes.len().saturating_sub(1))).map(|i| {
                                            let a = i % nodes.len(); let b = (i+1+(i*3)%2) % nodes.len();
                                            view! { <line x1=nodes[a].0.to_string() y1=nodes[a].1.to_string() x2=nodes[b].0.to_string() y2=nodes[b].1.to_string() stroke="#00e5ff" stroke-width="0.5" opacity="0.3"/> }
                                        }).collect::<Vec<_>>()}
                                        {nodes.iter().enumerate().map(|(i, (x, y))| {
                                            let br = 0.3 + (1.0 - (i as f64 / n_nodes as f64)) * 0.5;
                                            view! { <circle cx=x.to_string() cy=y.to_string() r="2.5" fill="#00e5ff" opacity=br.to_string()/> }
                                        }).collect::<Vec<_>>()}
                                    </svg>
                                    <div class="eb-stats"><span>{format!("{} {}exp {}e", acc, exp, edges)}</span></div>
                                </div>
                            }
                        }}
                        // GENESIS
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            let g = s.get("genesis");
                            let gen = g.and_then(|g| g.get("generation")).and_then(|v| v.as_u64()).unwrap_or(0);
                            let templates = g.and_then(|g| g.get("templates")).and_then(|v| v.as_u64()).unwrap_or(0);
                            let n = (templates as usize).min(10).max(2);
                            view! {
                                <div class="eb-module">
                                    <div class="eb-label">"GENESIS"</div>
                                    <svg viewBox="0 0 120 60" class="eb-svg">
                                        {(0..n).map(|i| {
                                            let y = 5 + i * 5;
                                            let width = 30 + ((i * 17 + 23) % 60);
                                            let br = 0.3 + (1.0 - i as f64 / n as f64) * 0.5;
                                            view! { <rect x="10" y=y.to_string() width=width.to_string() height="3" fill="#b388ff" opacity=br.to_string() rx="1"/> }
                                        }).collect::<Vec<_>>()}
                                    </svg>
                                    <div class="eb-stats"><span>{format!("gen{} {}tmpl", gen, templates)}</span></div>
                                </div>
                            }
                        }}
                    </div>
                    // SYNTHESIS (inline within THINK card)
                    {move || {
                        let s = soul.get().unwrap_or_default();
                        let sy = s.get("synthesis");
                        let state = sy.and_then(|s| s.get("state")).and_then(|v| v.as_str()).unwrap_or("--");
                        let weights = sy.and_then(|s| s.get("weights"));
                        let wb = weights.and_then(|w| w.get("brain")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                        let wc = weights.and_then(|w| w.get("cortex")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                        let wg = weights.and_then(|w| w.get("genesis")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                        let wh = weights.and_then(|w| w.get("hivemind")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                        let role = s.get("role");
                        let psi = role.and_then(|r| r.get("psi")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let state_cls = match state { "coherent"|"exploiting" => "eb-state-ok", "exploring" => "eb-state-learn", "conflicted" => "eb-state-warn", _ => "eb-state-err" };
                        view! {
                            <div class="eb-synth-bar">
                                <span class="eb-label" style="margin:0">"SYNTH"</span>
                                <span class="eb-weight">{format!("B:{} C:{} G:{} H:{}", wb, wc, wg, wh)}</span>
                                <span class={format!("eb-state {}", state_cls)}>{state.to_string()}</span>
                                <span class="eb-psi">{format!("\u{03A8}={:.3}", psi)}</span>
                            </div>
                        }
                    }}
                </div>

                // ── SENSE card (hivemind + eval + colony) ──
                <div class="eb-card">
                    <div class="eb-card-label">"SENSE"</div>
                    // HIVEMIND
                    {move || {
                        let s = soul.get().unwrap_or_default();
                        let h = s.get("hivemind");
                        let trails = h.and_then(|h| h.get("total_trails")).and_then(|v| v.as_u64()).unwrap_or(0);
                        let deposits = h.and_then(|h| h.get("total_deposits")).and_then(|v| v.as_u64()).unwrap_or(0);
                        let top = h.and_then(|h| h.get("top_attractants")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
                        view! {
                            <div class="eb-module">
                                <div class="eb-label">"HIVEMIND"</div>
                                <svg viewBox="0 0 120 60" class="eb-svg">
                                    {top.iter().take(6).enumerate().map(|(i, t)| {
                                        let resource = t.get("resource").and_then(|v| v.as_str()).unwrap_or("?");
                                        let intensity = t.get("intensity").and_then(|v| v.as_f64()).unwrap_or(0.3);
                                        let angle = (i as f64) * 0.9 - 1.2;
                                        let len = 20.0 + intensity * 25.0;
                                        let x2 = 30.0 + len * angle.cos();
                                        let y2 = 30.0 + len * angle.sin();
                                        let w = 0.5 + intensity * 2.5;
                                        let short: String = resource.chars().take(8).collect();
                                        view! {
                                            <line x1="30" y1="30" x2=x2.to_string() y2=y2.to_string() stroke="#ffa000" stroke-width=w.to_string() opacity=intensity.to_string()/>
                                            <text x=x2.to_string() y=(y2+3.0).to_string() class="eb-micro" fill="#ffa000" opacity="0.5">{short}</text>
                                        }
                                    }).collect::<Vec<_>>()}
                                    {top.is_empty().then(|| view! { <text x="60" y="30" text-anchor="middle" class="eb-tiny" fill="#3a4a3a">"no trails"</text> })}
                                </svg>
                                <div class="eb-stats"><span>{format!("{}t {}d", trails, deposits)}</span></div>
                            </div>
                        }
                    }}
                    // EVAL
                    {move || {
                        let s = soul.get().unwrap_or_default();
                        let e = s.get("evaluation");
                        let records = e.and_then(|e| e.get("total_records")).and_then(|v| v.as_u64()).unwrap_or(0);
                        let benefit = e.and_then(|e| e.get("colony_benefit")).and_then(|c| c.get("avg_sync_benefit")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                        view! {
                            <div class="eb-module">
                                <div class="eb-label">"EVAL"</div>
                                <div class="eb-stats"><span>{format!("{}rec {:+.3}\u{0394}", records, benefit)}</span></div>
                            </div>
                        }
                    }}
                </div>
            </div>

            // ═══ ACT card (free energy → codegen + feedback) ═══
            <div class="eb-card">
                <div class="eb-card-label">"ACT"</div>
                // FREE ENERGY (inline)
                {move || {
                    let s = soul.get().unwrap_or_default();
                    let fe = s.get("free_energy");
                    let f_val = fe.and_then(|f| f.get("F")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                    let regime = fe.and_then(|f| f.get("regime")).and_then(|v| v.as_str()).unwrap_or("--");
                    let regime_cls = match regime { "EXPLORE" => "eb-regime-explore", "EXPLOIT" => "eb-regime-exploit", "LEARN" => "eb-regime-learn", _ => "eb-regime-anomaly" };
                    let f_num: f64 = f_val.parse().unwrap_or(0.5);
                    let bar_pct = ((1.0 - f_num.min(1.0)) * 100.0) as u32;
                    view! {
                        <div class="eb-synth-bar">
                            <span class="eb-label" style="margin:0">"F"</span>
                            <div class="eb-fe-bar" style="flex:1">
                                <div class={format!("eb-fe-fill {}", regime_cls)} style=format!("width:{}%", bar_pct)></div>
                            </div>
                            <span class="eb-stat">{format!("={}", f_val)}</span>
                            <span class={format!("eb-regime {}", regime_cls)}>{regime.to_string()}</span>
                        </div>
                    }
                }}
                <div class="eb-row-2">
                    // CODEGEN
                    {move || {
                        let s = soul.get().unwrap_or_default();
                        let cg = s.get("codegen");
                        let sols = cg.and_then(|c| c.get("solutions_stored")).and_then(|v| v.as_u64()).unwrap_or(0);
                        let steps = cg.and_then(|c| c.get("model_steps")).and_then(|v| v.as_u64()).unwrap_or(0);
                        let loss = cg.and_then(|c| c.get("model_loss")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                        let can = cg.and_then(|c| c.get("can_generate")).and_then(|v| v.as_bool()).unwrap_or(false);
                        let params = cg.and_then(|c| c.get("model_params")).and_then(|v| v.as_u64()).unwrap_or(0);
                        let badge_cls = if can { "eb-badge-ok" } else { "eb-badge-off" };
                        let loss_num: f64 = loss.parse().unwrap_or(10.0);
                        let bar_pct = ((1.0 - (loss_num / 10.0).min(1.0)) * 100.0) as u32;
                        view! {
                            <div class="eb-module">
                                <div class="eb-label">"CODEGEN"</div>
                                <div class="eb-fe-bar" style="margin:4px 0">
                                    <div class="eb-fe-fill eb-regime-exploit" style=format!("width:{}%", bar_pct)></div>
                                </div>
                                <div class="eb-stats">
                                    <span>{format!("{}M L={} {}d {}s", params/1_000_000, loss, sols, steps)}</span>
                                </div>
                                <span class={format!("eb-badge {}", badge_cls)}>{if can { "CAN GEN" } else { "NO GEN" }}</span>
                            </div>
                        }
                    }}
                    // FEEDBACK
                    {move || {
                        let s = soul.get().unwrap_or_default();
                        let ch = s.get("cycle_health");
                        let completed = ch.and_then(|h| h.get("completed_plans_count")).and_then(|v| v.as_u64()).unwrap_or(0);
                        let failed = ch.and_then(|h| h.get("failed_plans_count")).and_then(|v| v.as_u64()).unwrap_or(0);
                        let total = (completed + failed).max(1);
                        let rate = completed * 100 / total;
                        let rate_cls = if rate >= 70 { "eb-rate-ok" } else if rate >= 40 { "eb-rate-warn" } else { "eb-rate-bad" };
                        view! {
                            <div class="eb-module">
                                <div class="eb-label">"FEEDBACK"</div>
                                <div class="eb-feedback">
                                    <span class="eb-fb-ok">{format!("{}\u{2713}", completed)}</span>
                                    <span class="eb-fb-fail">{format!("{}\u{2717}", failed)}</span>
                                    <span class=rate_cls>{format!("{}%", rate)}</span>
                                </div>
                                <div class="eb-stats"><span>{"\u{25B2}brain \u{25B2}genesis"}</span></div>
                            </div>
                        }
                    }}
                </div>
            </div>

            // ═══ EVENTS + STATUS ═══
            <div class="eb-events">
                {move || {
                    let evts = events.get();
                    evts.iter().rev().take(4).map(|evt| {
                        let color = event_color(&evt.code);
                        let abbr = event_abbr(&evt.code);
                        let msg: String = evt.message.chars().take(60).collect();
                        view! {
                            <div class="eb-event">
                                <span class="eb-event-tag" style=format!("color:{}", color)>{format!("[{}]", abbr)}</span>
                                <span class="eb-event-msg">{msg}</span>
                            </div>
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
            {move || {
                let s = soul.get().unwrap_or_default();
                let cycles = s.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                let mode = s.get("mode").and_then(|v| v.as_str()).unwrap_or("--");
                let d = info.get().unwrap_or_default();
                let fitness = d.get("fitness").and_then(|f| f.get("total")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let active = s.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                let sys = system.get().unwrap_or_default();
                let cpu = sys.get("cpu_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
                view! {
                    <div class="eb-bottom">
                        <span class={if active { "eb-dot-on" } else { "eb-dot-off" }}></span>
                        <span>{format!("{} | cycle {} | fitness {:.0}% | cpu {:.0}%", mode, cycles, fitness * 100.0, cpu)}</span>
                        <span style="flex:1"></span>
                        <span class="eb-ver">{concat!("v", env!("CARGO_PKG_VERSION"))}</span>
                    </div>
                }
            }}

            // ═══ CONTROLS ═══
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
                            view! { <div class="mandala-panel-section"><div style="font-size:10px;color:var(--text-dim)">{short}</div></div> }.into_view()
                        }}
                        {move || {
                            let d = info.get().unwrap_or_default();
                            let avail = d.get("clone_available").and_then(|v| v.as_bool()).unwrap_or(false);
                            if !avail { return view! { <div></div> }.into_view(); }
                            let price = d.get("clone_price").and_then(|v| v.as_str()).unwrap_or("N/A").to_string();
                            let do_clone = move |_: web_sys::MouseEvent| {
                                if clone_loading.get() { return; }
                                let w = wallet.get();
                                if !w.connected { return; }
                                set_clone_loading.set(true); set_clone_result.set(None);
                                spawn_local(async move {
                                    match api::clone_instance(&w).await {
                                        Ok(r) => set_clone_result.set(Some(Ok(format!("{} at {}", r.instance_id.unwrap_or_default(), r.url.unwrap_or_default())))),
                                        Err(e) => set_clone_result.set(Some(Err(e))),
                                    }
                                    set_clone_loading.set(false);
                                });
                            };
                            view! {
                                <div class="mandala-panel-section">
                                    <div class="mandala-panel-label">"CLONE"</div>
                                    <button class="btn btn-primary" on:click=do_clone disabled=move || clone_loading.get() || !wallet.get().connected>
                                        {move || if clone_loading.get() { "..." } else { "Clone" }}
                                    </button>
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

fn pulse_intensity(pulses: &std::collections::HashMap<String, f64>, prefix: &str, now: f64) -> f64 {
    let last = pulses.iter().filter(|(c, _)| c.starts_with(prefix)).map(|(_, t)| *t).fold(0.0f64, f64::max);
    if last == 0.0 { return 0.0; }
    (1.0 - ((now - last) / 10_000.0)).max(0.0)
}

fn event_color(code: &str) -> &'static str {
    if code.starts_with("brain") { "#00ff41" }
    else if code.starts_with("transformer") { "#00e5ff" }
    else if code.starts_with("codegen") { "#ffa000" }
    else if code.starts_with("plan") { "#b388ff" }
    else if code.starts_with("benchmark") { "#00ff41" }
    else if code.starts_with("peer") { "#00e5ff" }
    else { "#5a6a5a" }
}

fn event_abbr(code: &str) -> &'static str {
    if code.starts_with("brain.trained") { "BRAIN" }
    else if code.starts_with("transformer") { "XFORM" }
    else if code.starts_with("codegen") { "COGEN" }
    else if code.starts_with("plan.step") { "STEP" }
    else if code.starts_with("plan") { "PLAN" }
    else if code.starts_with("benchmark") { "BENCH" }
    else if code.starts_with("peer") { "PEER" }
    else { "EVENT" }
}
