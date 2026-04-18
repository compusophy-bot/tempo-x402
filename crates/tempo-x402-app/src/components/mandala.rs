use crate::api;
use crate::WalletState;
use gloo_timers::callback::Interval;
use leptos::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::wallet_panel::WalletButtons;

#[derive(Clone, Debug)]
struct SoulEventMsg {
    code: String,
    message: String,
}

/// The Colony — one distributed mind visualized as a living organism.
/// Shows collective intelligence, not individual node stats.
#[component]
pub fn Mandala() -> impl IntoView {
    let (wallet, set_wallet) =
        expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();
    let (soul, set_soul) = create_signal(None::<serde_json::Value>);
    let (info, set_info) = create_signal(None::<serde_json::Value>);
    let (system, set_system) = create_signal(None::<serde_json::Value>);
    let (colony, set_colony) = create_signal(None::<serde_json::Value>);
    let (panel_open, set_panel_open) = create_signal(false);
    let (clone_loading, set_clone_loading) = create_signal(false);
    let (_clone_result, set_clone_result) = create_signal(None::<Result<String, String>>);
    let (events, set_events) = create_signal(Vec::<SoulEventMsg>::new());
    let (pulses, set_pulses) = create_signal(std::collections::HashMap::<String, f64>::new());
    let (selected_organ, set_selected_organ) = create_signal(None::<String>);

    let fetch_all = move || {
        spawn_local(async move {
            let base = api::gateway_base_url();
            if let Ok(r) = gloo_net::http::Request::get(&format!("{}/instance/info", base))
                .send()
                .await
            {
                if r.ok() {
                    if let Ok(d) = r.json::<serde_json::Value>().await {
                        set_info.set(Some(d));
                    }
                }
            }
            if let Ok(d) = api::fetch_soul_status().await {
                set_soul.set(Some(d));
            }
            if let Ok(r) = gloo_net::http::Request::get(&format!("{}/soul/system", base))
                .send()
                .await
            {
                if r.ok() {
                    if let Ok(d) = r.json::<serde_json::Value>().await {
                        set_system.set(Some(d));
                    }
                }
            }
            // Fetch colony peers (works on queen, returns empty on standalone)
            if let Ok(r) = gloo_net::http::Request::get(&format!("{}/soul/colony/peers", base))
                .send()
                .await
            {
                if r.ok() {
                    if let Ok(d) = r.json::<serde_json::Value>().await {
                        set_colony.set(Some(d));
                    }
                }
            }
        });
    };
    fetch_all();
    let interval = Interval::new(8_000, move || {
        fetch_all();
    });
    on_cleanup(move || drop(interval));

    // SSE
    {
        let base = api::gateway_base_url().to_string();
        spawn_local(async move {
            let es = match web_sys::EventSource::new(&format!("{}/soul/events/stream", base)) {
                Ok(e) => e,
                Err(_) => return,
            };
            let on_msg =
                Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                    let s = ev.data().as_string().unwrap_or_default();
                    if let Ok(p) = serde_json::from_str::<serde_json::Value>(&s) {
                        let code = p
                            .get("code")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let msg = p
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !code.is_empty() && code != "heartbeat" {
                            set_events.update(|e| {
                                e.push(SoulEventMsg {
                                    code: code.clone(),
                                    message: msg,
                                });
                                if e.len() > 40 {
                                    e.drain(..e.len() - 40);
                                }
                            });
                            set_pulses.update(|p| {
                                p.insert(code, js_sys::Date::now());
                            });
                        }
                    }
                });
            es.add_event_listener_with_callback("soul_event", on_msg.as_ref().unchecked_ref())
                .ok();
            on_msg.forget();
        });
    }

    view! {
        <div class="organism">
            <Show when=move || soul.get().is_some() fallback=move || view! {
                <div class="org-loading">
                    <div class="org-loading-pulse"></div>
                    <span>"waking up..."</span>
                </div>
            }>

            // ═══════════════════════════════════════════════════
            // COLONY MIND — IQ trajectory + nodes + progress
            // ═══════════════════════════════════════════════════
            {move || {
                let s = soul.get().unwrap_or_default();
                let b = s.get("benchmark");
                let col = colony.get().unwrap_or_default();

                let collective = b.and_then(|b| b.get("collective"));
                let solved = collective.and_then(|c| c.get("unique_solved")).and_then(|v| v.as_u64()).unwrap_or(0);
                let total = collective.and_then(|c| c.get("total_problems")).and_then(|v| v.as_u64()).unwrap_or(100);

                // Colony info
                let colony_size = col.get("colony_size").and_then(|v| v.as_u64())
                    .or_else(|| s.get("role").and_then(|r| r.get("colony_size")).and_then(|v| v.as_u64()))
                    .unwrap_or(1);
                let workers = col.get("workers").and_then(|v| v.as_array()).cloned().unwrap_or_default();

                // IQ history
                let elo_history = b.and_then(|b| b.get("elo_history")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let iq_points: Vec<f64> = elo_history.iter()
                    .filter_map(|h| h.get("pass_at_1").and_then(|v| v.as_f64()))
                    .map(|p| 85.0 + (p / 100.0) * 65.0)
                    .collect();

                let current_iq = iq_points.last().copied().unwrap_or(0.0);
                let prev_iq = if iq_points.len() >= 2 { iq_points[iq_points.len() - 2] } else { current_iq };
                let iq_delta = current_iq - prev_iq;
                let delta_cls = if iq_delta > 0.5 { "org-delta-up" } else if iq_delta < -0.5 { "org-delta-down" } else { "org-delta-flat" };

                let iq_min = iq_points.iter().copied().fold(f64::MAX, f64::min).min(current_iq) - 2.0;
                let iq_max = iq_points.iter().copied().fold(f64::MIN, f64::max).max(current_iq) + 2.0;
                let iq_range = (iq_max - iq_min).max(1.0);

                let chart_w = 300.0_f64;
                let chart_h = 70.0_f64;
                let points_str: String = iq_points.iter().enumerate().map(|(i, &iq)| {
                    let x = if iq_points.len() > 1 { (i as f64 / (iq_points.len() - 1) as f64) * chart_w } else { chart_w / 2.0 };
                    let y = chart_h - ((iq - iq_min) / iq_range) * chart_h;
                    format!("{:.1},{:.1}", x, y)
                }).collect::<Vec<_>>().join(" ");

                let last_x = chart_w;
                let last_y = if !iq_points.is_empty() { chart_h - ((current_iq - iq_min) / iq_range) * chart_h } else { chart_h / 2.0 };

                let elo = b.and_then(|b| b.get("elo_rating")).and_then(|v| v.as_f64()).unwrap_or(0.0);

                let accel = s.get("acceleration");
                let alpha: f64 = accel.and_then(|a| a.get("alpha")).and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
                    .or_else(|| accel.and_then(|a| a.get("alpha")).and_then(|v| v.as_f64()))
                    .unwrap_or(0.0);
                let regime = accel.and_then(|a| a.get("regime")).and_then(|v| v.as_str()).unwrap_or("STALLED");
                let psi = s.get("role").and_then(|r| r.get("psi")).and_then(|v| v.as_f64()).unwrap_or(0.0);

                view! {
                    <div class="org-mind">
                        // IQ chart
                        <div class="org-iq-chart">
                            <div class="org-iq-header">
                                <span class="org-colony-title">{format!("COLONY ({} nodes)", colony_size)}</span>
                                <span class="org-iq-current">{format!("IQ {:.0}", current_iq)}</span>
                                <span class={format!("org-iq-delta {}", delta_cls)}>{format!("{:+.0}", iq_delta)}</span>
                                <span class="org-iq-elo">{format!("ELO {:.0}", elo)}</span>
                            </div>
                            <svg viewBox={format!("0 0 {} {}", chart_w, chart_h)} class="org-chart-svg" preserveAspectRatio="none">
                                {(0..5).map(|i| {
                                    let y = (i as f64 / 4.0) * chart_h;
                                    view! { <line x1="0" y1=y.to_string() x2=chart_w.to_string() y2=y.to_string() stroke="#1a1a2e" stroke-width="0.5"/> }
                                }).collect::<Vec<_>>()}
                                {(!points_str.is_empty()).then(|| view! {
                                    <polyline points=points_str.clone() fill="none" stroke="#00ff41" stroke-width="2" stroke-linejoin="round"/>
                                    <polyline points=format!("{} {},{} 0,{}", points_str, chart_w, chart_h, chart_h)
                                        fill="url(#iq-glow)" stroke="none"/>
                                })}
                                <circle cx=last_x.to_string() cy=last_y.to_string() r="3" fill="#00ff41" class="org-pulse-dot"/>
                                <circle cx=last_x.to_string() cy=last_y.to_string() r="6" fill="none" stroke="#00ff41" stroke-width="0.5" opacity="0.4" class="org-pulse-ring"/>
                                <defs>
                                    <linearGradient id="iq-glow" x1="0" y1="0" x2="0" y2="1">
                                        <stop offset="0%" stop-color="#00ff41" stop-opacity="0.15"/>
                                        <stop offset="100%" stop-color="#00ff41" stop-opacity="0"/>
                                    </linearGradient>
                                </defs>
                            </svg>
                            <div class="org-iq-sub">
                                <span class={format!("org-regime {}", regime.to_lowercase())}>{format!("\u{03B1}{:+.3} {}", alpha, regime)}</span>
                                <span class="org-psi-badge">{format!("\u{03A8}={:.3}", psi)}</span>
                            </div>
                        </div>

                        // Right: progress ring + node list
                        <div class="org-right">
                            {render_progress_ring(solved, total)}
                            // Colony nodes as stacked indicators
                            <div class="org-nodes">
                                <div class="org-node-self">
                                    <span class="org-node-dot-on"></span>
                                    <span class="org-node-id">"queen"</span>
                                </div>
                                {workers.iter().map(|w| {
                                    let id = w.get("instance_id").and_then(|v| v.as_str()).unwrap_or("?");
                                    let short: String = id.chars().take(8).collect();
                                    view! {
                                        <div class="org-node-worker">
                                            <span class="org-node-dot-on"></span>
                                            <span class="org-node-id">{short}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>
                    </div>
                }
            }}

            // ═══════════════════════════════════════════════════
            // NERVOUS SYSTEM — 9 interconnected organs (click to expand)
            // ═══════════════════════════════════════════════════
            {move || {
                let s = soul.get().unwrap_or_default();
                let p = pulses.get();
                let now = js_sys::Date::now();
                let sel = selected_organ.get();

                let brain_loss = s.get("brain").and_then(|b| b.get("running_loss")).and_then(|v| v.as_f64()).unwrap_or(1.0);
                let brain_steps = s.get("brain").and_then(|b| b.get("train_steps")).and_then(|v| v.as_u64()).unwrap_or(0);
                let brain_health = (1.0 - brain_loss.min(1.0)).max(0.1);

                let cg_loss = s.get("codegen").and_then(|c| c.get("model_loss")).and_then(|v| v.as_f64())
                    .or_else(|| s.get("codegen").and_then(|c| c.get("model_loss")).and_then(|v| v.as_str()).and_then(|s| s.parse().ok()))
                    .unwrap_or(10.0);
                let cg_steps = s.get("codegen").and_then(|c| c.get("model_steps")).and_then(|v| v.as_u64()).unwrap_or(0);
                let cg_solutions = s.get("codegen").and_then(|c| c.get("solutions_stored")).and_then(|v| v.as_u64()).unwrap_or(0);
                let cg_health = (1.0 - (cg_loss / 10.0).min(1.0)).max(0.1);

                let xf_loss = s.get("transformer").and_then(|t| t.get("last_train_loss")).and_then(|v| v.as_f64()).unwrap_or(2.0);
                let xf_plans = s.get("transformer").and_then(|t| t.get("plans_generated")).and_then(|v| v.as_u64()).unwrap_or(0);
                let xf_health = (1.0 - (xf_loss / 3.0).min(1.0)).max(0.1);

                let cortex_acc_str = s.get("cortex").and_then(|c| c.get("prediction_accuracy")).and_then(|v| v.as_str()).unwrap_or("50");
                let cortex_acc: f64 = cortex_acc_str.replace('%', "").parse().unwrap_or(50.0);
                let cortex_exp = s.get("cortex").and_then(|c| c.get("total_experiences")).and_then(|v| v.as_u64()).unwrap_or(0);
                let cortex_edges = s.get("cortex").and_then(|c| c.get("causal_edges")).and_then(|v| v.as_u64()).unwrap_or(0);
                let cortex_health = (cortex_acc / 100.0).max(0.1);

                let gen_gen = s.get("genesis").and_then(|g| g.get("generation")).and_then(|v| v.as_u64()).unwrap_or(0);
                let gen_tmpl = s.get("genesis").and_then(|g| g.get("templates")).and_then(|v| v.as_u64()).unwrap_or(0);
                let gen_muts = s.get("genesis").and_then(|g| g.get("total_mutations")).and_then(|v| v.as_u64()).unwrap_or(0);
                let gen_health = if gen_gen > 100 { 0.8 } else { (gen_gen as f64 / 100.0).max(0.2) };

                let hive_trails = s.get("hivemind").and_then(|h| h.get("total_trails")).and_then(|v| v.as_u64()).unwrap_or(0);
                let hive_deposits = s.get("hivemind").and_then(|h| h.get("total_deposits")).and_then(|v| v.as_u64()).unwrap_or(0);
                let hive_health = if hive_trails > 20 { 0.8 } else { (hive_trails as f64 / 20.0).max(0.2) };

                let synth_state = s.get("synthesis").and_then(|sy| sy.get("state")).and_then(|v| v.as_str()).unwrap_or("stuck");
                let synth_conflicts = s.get("synthesis").and_then(|sy| sy.get("conflicts")).and_then(|v| v.as_u64()).unwrap_or(0);
                let synth_health = match synth_state { "coherent"|"exploiting" => 0.9, "exploring" => 0.6, "conflicted" => 0.4, _ => 0.1 };

                let ch = s.get("cycle_health");
                let plans_ok = ch.and_then(|h| h.get("completed_plans_count")).and_then(|v| v.as_u64()).unwrap_or(0);
                let plans_fail = ch.and_then(|h| h.get("failed_plans_count")).and_then(|v| v.as_u64()).unwrap_or(0);
                let feedback_health = if plans_ok + plans_fail == 0 { 0.5 } else { plans_ok as f64 / (plans_ok + plans_fail) as f64 };

                let fe_str = s.get("free_energy").and_then(|f| f.get("F")).and_then(|v| v.as_str()).unwrap_or("0.5");
                let fe_val: f64 = fe_str.parse().unwrap_or(0.5);
                let fe_regime = s.get("free_energy").and_then(|f| f.get("regime")).and_then(|v| v.as_str()).unwrap_or("--");
                let fe_health = (1.0 - fe_val).max(0.1);
                let psi = s.get("role").and_then(|r| r.get("psi")).and_then(|v| v.as_f64()).unwrap_or(0.0);

                // Organs: (name, health, short_detail, expanded_detail, prefix, color_override)
                let organs: Vec<(&str, f64, String, String, &str)> = vec![
                    ("BRAIN", brain_health, format!("L={:.2}", brain_loss), format!("1.2M params | Loss {:.4} | {}K steps | Predicts step success + error type", brain_loss, brain_steps/1000), "brain"),
                    ("CODEGEN", cg_health, format!("L={:.1}", cg_loss), format!("29M params | Loss {:.2} | {}K steps | {} solutions | Generating Rust code locally", cg_loss, cg_steps/1000, cg_solutions), "codegen"),
                    ("XFORMER", xf_health, format!("L={:.2}", xf_loss), format!("2.2M params | Loss {:.3} | {} plans generated | Predicts plan sequences", xf_loss, xf_plans), "transformer"),
                    ("CORTEX", cortex_health, format!("{:.0}%", cortex_acc), format!("Prediction accuracy {:.1}% | {} experiences | {} causal edges | World model", cortex_acc, cortex_exp, cortex_edges), "cortex"),
                    ("GENESIS", gen_health, format!("g{}", gen_gen), format!("Generation {} | {} templates | {} mutations | Evolving plan DNA", gen_gen, gen_tmpl, gen_muts), "genesis"),
                    ("HIVEMIND", hive_health, format!("{}t", hive_trails), format!("{} pheromone trails | {} deposits | Stigmergic swarm coordination", hive_trails, hive_deposits), "hivemind"),
                    ("SYNTH", synth_health, synth_state.to_string(), format!("State: {} | {} conflicts | Metacognitive self-awareness | Unifies all systems", synth_state, synth_conflicts), "synthesis"),
                    ("FEEDBACK", feedback_health, format!("{:.0}%", feedback_health * 100.0), format!("{} plans succeeded | {} failed | {:.0}% success rate | Error classification + lessons", plans_ok, plans_fail, feedback_health * 100.0), "plan"),
                    ("F(t)", fe_health, format!("{}", fe_regime), format!("F={} | Regime: {} | Free energy principle | Lower = smarter", fe_str, fe_regime), "free_energy"),
                ];

                let cx = 200.0_f64;
                let cy = 170.0_f64;
                let radius = 130.0_f64;
                let n = organs.len() as f64;

                view! {
                    <div class="org-nervous">
                        <svg viewBox="0 0 400 340" class="org-nerve-svg">
                            // Center: Psi
                            <circle cx=cx.to_string() cy=cy.to_string() r="22"
                                fill="none" stroke="#00ff41"
                                stroke-width="1.5"
                                opacity=(0.3 + psi * 0.7).to_string()/>
                            <text x=cx.to_string() y=(cy - 4.0).to_string()
                                text-anchor="middle" class="org-psi-text">{"\u{03A8}"}</text>
                            <text x=cx.to_string() y=(cy + 8.0).to_string()
                                text-anchor="middle" class="org-psi-val">{format!("{:.3}", psi)}</text>

                            // Connections to center
                            {organs.iter().enumerate().map(|(i, _)| {
                                let angle = (i as f64 / n) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
                                let ox = cx + radius * angle.cos();
                                let oy = cy + radius * angle.sin();
                                view! {
                                    <line x1=cx.to_string() y1=cy.to_string()
                                        x2=ox.to_string() y2=oy.to_string()
                                        stroke="#1a1a2e" stroke-width="1" stroke-dasharray="3,3"/>
                                }
                            }).collect::<Vec<_>>()}

                            // Adjacent connections
                            {(0..organs.len()).map(|i| {
                                let j = (i + 1) % organs.len();
                                let ai = (i as f64 / n) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
                                let aj = (j as f64 / n) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
                                view! {
                                    <line x1=(cx + radius * ai.cos()).to_string() y1=(cy + radius * ai.sin()).to_string()
                                        x2=(cx + radius * aj.cos()).to_string() y2=(cy + radius * aj.sin()).to_string()
                                        stroke="#1a1a2e" stroke-width="0.5" opacity="0.4"/>
                                }
                            }).collect::<Vec<_>>()}

                            // Organ nodes
                            {organs.iter().enumerate().map(|(i, (name, health, detail, _, prefix))| {
                                let angle = (i as f64 / n) * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
                                let ox = cx + radius * angle.cos();
                                let oy = cy + radius * angle.sin();
                                let r = 14.0 + health * 8.0;
                                let active = p.iter().any(|(c, t)| c.starts_with(prefix) && (now - t) < 8_000.0);
                                let color = health_color(*health);
                                let is_selected = sel.as_deref() == Some(*name);
                                let opacity = if active || is_selected { 1.0 } else { 0.6 + health * 0.3 };
                                let stroke_w = if is_selected { "2.5" } else if active { "2" } else { "1" };
                                let glow_r = if active { r + 6.0 } else { 0.0 };
                                let name_owned = name.to_string();
                                view! {
                                    {(active).then(|| view! {
                                        <circle cx=ox.to_string() cy=oy.to_string() r=glow_r.to_string()
                                            fill="none" stroke=color stroke-width="1" opacity="0.3" class="org-pulse-ring"/>
                                    })}
                                    <circle cx=ox.to_string() cy=oy.to_string() r=r.to_string()
                                        fill=color fill-opacity=(health * 0.2).to_string()
                                        stroke=color stroke-width=stroke_w
                                        opacity=opacity.to_string()
                                        style="cursor:pointer"
                                        on:click=move |_| {
                                            set_selected_organ.update(|s| {
                                                if s.as_deref() == Some(name_owned.as_str()) { *s = None; } else { *s = Some(name_owned.clone()); }
                                            });
                                        }/>
                                    <text x=ox.to_string() y=(oy - 3.0).to_string()
                                        text-anchor="middle" class="org-node-name" fill=color
                                        style="pointer-events:none">
                                        {name.to_string()}
                                    </text>
                                    <text x=ox.to_string() y=(oy + 7.0).to_string()
                                        text-anchor="middle" class="org-node-detail"
                                        style="pointer-events:none">
                                        {detail.clone()}
                                    </text>
                                }
                            }).collect::<Vec<_>>()}
                        </svg>

                        // Expanded detail panel (shows when an organ is clicked)
                        {sel.and_then(|ref name| {
                            organs.iter().find(|(n, _, _, _, _)| *n == name.as_str()).map(|(organ_name, health, _, expanded, _)| {
                                let color = health_color(*health);
                                view! {
                                    <div class="org-detail-panel" style=format!("border-color:{}", color)>
                                        <div class="org-detail-name" style=format!("color:{}", color)>{organ_name.to_string()}</div>
                                        <div class="org-detail-text">{expanded.clone()}</div>
                                    </div>
                                }
                            })
                        })}
                    </div>
                }
            }}

            // ═══════════════════════════════════════════════════
            // PULSE — events stream
            // ═══════════════════════════════════════════════════
            <div class="org-events">
                {move || {
                    let evts = events.get();
                    evts.iter().rev().take(6).map(|evt| {
                        let color = event_color(&evt.code);
                        let msg: String = evt.message.chars().take(90).collect();
                        view! {
                            <div class="org-event">
                                <span class="org-event-dot" style=format!("background:{}", color)></span>
                                <span class="org-event-msg">{msg}</span>
                            </div>
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>

            // Status bar
            {move || {
                let s = soul.get().unwrap_or_default();
                let cycles = s.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                let mode = s.get("mode").and_then(|v| v.as_str()).unwrap_or("--");
                let d = info.get().unwrap_or_default();
                let fitness = d.get("fitness").and_then(|f| f.get("total")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let active = s.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                let sys = system.get().unwrap_or_default();
                let cpu = sys.get("cpu_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let colony_size = s.get("role").and_then(|r| r.get("colony_size")).and_then(|v| v.as_u64()).unwrap_or(1);
                view! {
                    <div class="org-status">
                        <span class={if active { "org-dot-on" } else { "org-dot-off" }}></span>
                        <span>{format!("{} | cycle {} | fitness {:.0}% | cpu {:.0}% | {} nodes", mode, cycles, fitness * 100.0, cpu, colony_size)}</span>
                        <span style="flex:1"></span>
                        <span class="org-ver">{concat!("v", env!("CARGO_PKG_VERSION"))}</span>
                    </div>
                }
            }}

            </Show>

            // Controls
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
                            let do_clone = move |_: web_sys::MouseEvent| {
                                if clone_loading.get() { return; }
                                let w = wallet.get();
                                if !w.connected { return; }
                                set_clone_loading.set(true); set_clone_result.set(None);
                                spawn_local(async move {
                                    match api::clone_instance(&w).await {
                                        Ok(r) => set_clone_result.set(Some(Ok(format!("{}", r.instance_id.unwrap_or_default())))),
                                        Err(e) => set_clone_result.set(Some(Err(e))),
                                    }
                                    set_clone_loading.set(false);
                                });
                            };
                            view! {
                                <div class="mandala-panel-section">
                                    <button class="btn btn-primary" on:click=do_clone disabled=move || clone_loading.get() || !wallet.get().connected>
                                        {move || if clone_loading.get() { "cloning..." } else { "spawn clone" }}
                                    </button>
                                </div>
                            }.into_view()
                        }}
                        <div class="mandala-panel-section">
                            <a href="/dashboard" class="mandala-nav-link">"cockpit"</a>
                            <a href="/studio" class="mandala-nav-link">"studio"</a>
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}

fn render_progress_ring(solved: u64, total: u64) -> impl IntoView {
    let pct = if total > 0 {
        solved as f64 / total as f64
    } else {
        0.0
    };
    let r = 32.0_f64;
    let circumference = 2.0 * std::f64::consts::PI * r;
    let filled = circumference * pct;
    let gap = circumference - filled;
    view! {
        <svg viewBox="0 0 80 80" class="org-ring-svg">
            <circle cx="40" cy="40" r=r.to_string()
                fill="none" stroke="#1a1a2e" stroke-width="3"/>
            <circle cx="40" cy="40" r=r.to_string()
                fill="none" stroke="#00ff41" stroke-width="3"
                stroke-dasharray=format!("{:.1} {:.1}", filled, gap)
                stroke-dashoffset=format!("{:.1}", circumference * 0.25)
                stroke-linecap="round"
                class="org-ring-progress"/>
            <text x="40" y="37" text-anchor="middle" class="org-ring-count">{format!("{}", solved)}</text>
            <text x="40" y="48" text-anchor="middle" class="org-ring-label">{format!("/{}", total)}</text>
        </svg>
    }
}

fn health_color(health: f64) -> &'static str {
    if health > 0.7 {
        "#00ff41"
    } else if health > 0.4 {
        "#ffa000"
    } else {
        "#ff1744"
    }
}

fn event_color(code: &str) -> &'static str {
    if code.starts_with("brain") {
        "#00ff41"
    } else if code.starts_with("transformer") {
        "#00e5ff"
    } else if code.starts_with("codegen") {
        "#ffa000"
    } else if code.starts_with("plan") {
        "#b388ff"
    } else if code.starts_with("benchmark") {
        "#00ff41"
    } else if code.starts_with("peer") || code.starts_with("colony") {
        "#00e5ff"
    } else {
        "#5a6a5a"
    }
}
