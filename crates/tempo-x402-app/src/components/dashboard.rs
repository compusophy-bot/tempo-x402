use super::soul_panel::SoulPanel;
use crate::api;
use crate::{WalletMode, WalletState};
use gloo_timers::callback::Interval;
use leptos::*;

#[component]
pub fn DashboardPage() -> impl IntoView {
    let (info, set_info) = create_signal(None::<serde_json::Value>);
    let (endpoints, set_endpoints) = create_signal(Vec::<serde_json::Value>::new());
    let (analytics, set_analytics) = create_signal(None::<serde_json::Value>);
    let (soul_status, set_soul_status) = create_signal(None::<serde_json::Value>);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal(None::<String>);
    let (tick, set_tick) = create_signal(0u32);

    // Clone action state
    let (clone_loading, set_clone_loading) = create_signal(false);
    let (clone_result, set_clone_result) =
        create_signal(None::<Result<api::CloneResponse, String>>);

    // Fetch all dashboard data
    let fetch_data = move || {
        spawn_local(async move {
            let base = api::gateway_base_url();

            // Fetch instance info
            match gloo_net::http::Request::get(&format!("{}/instance/info", base))
                .send()
                .await
            {
                Ok(resp) if resp.ok() => {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        set_info.set(Some(data));
                    }
                }
                Ok(resp) => {
                    set_error.set(Some(format!("HTTP {}", resp.status())));
                }
                Err(e) => {
                    set_error.set(Some(format!("{}", e)));
                }
            }

            // Fetch endpoints
            if let Ok(eps) = api::list_endpoints().await {
                set_endpoints.set(eps);
            }

            // Fetch analytics
            if let Ok(data) = api::fetch_analytics().await {
                set_analytics.set(Some(data));
            }

            // Fetch soul status
            if let Ok(data) = api::fetch_soul_status().await {
                set_soul_status.set(Some(data));
            }

            set_loading.set(false);
        });
    };

    // Initial fetch
    fetch_data();

    // Auto-refresh every 10s
    let interval = Interval::new(10_000, move || {
        set_tick.update(|t| *t = t.wrapping_add(1));
        fetch_data();
    });

    on_cleanup(move || {
        drop(interval);
    });

    view! {
        <div class="tmux">
            <Show when=move || loading.get() && info.get().is_none() fallback=|| ()>
                <p class="loading" style="padding: 20px;">"Loading dashboard..."</p>
            </Show>

            <Show when=move || error.get().is_some() && info.get().is_none() fallback=|| ()>
                <p class="error-text" style="padding: 20px;">{move || error.get().unwrap_or_default()}</p>
            </Show>

            <Show when=move || info.get().is_some() fallback=|| ()>
                {move || {
                    let data = info.get().unwrap_or_default();

                    let version = data.get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let uptime = data.get("uptime_seconds")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let eps = endpoints.get();
                    let ep_count = eps.len();

                    let analytics_data = analytics.get();
                    let total_payments = analytics_data.as_ref()
                        .and_then(|a| a.get("total_payments"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let _total_revenue_usd = analytics_data.as_ref()
                        .and_then(|a| a.get("total_revenue_usd"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("$0")
                        .to_string();
                    let analytics_endpoints = analytics_data.as_ref()
                        .and_then(|a| a.get("endpoints"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let _active_endpoints = analytics_endpoints.len();

                    // Fitness data from instance info
                    let fitness = data.get("fitness");
                    let fitness_total = fitness
                        .and_then(|f| f.get("total"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let fitness_trend = fitness
                        .and_then(|f| f.get("trend"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let fitness_economic = fitness
                        .and_then(|f| f.get("economic"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let fitness_execution = fitness
                        .and_then(|f| f.get("execution"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let fitness_evolution = fitness
                        .and_then(|f| f.get("evolution"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let fitness_coordination = fitness
                        .and_then(|f| f.get("coordination"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let fitness_introspection = fitness
                        .and_then(|f| f.get("introspection"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);

                    // Find weakest component
                    let components = [
                        ("economic", fitness_economic),
                        ("execution", fitness_execution),
                        ("evolution", fitness_evolution),
                        ("coordination", fitness_coordination),
                        ("introspection", fitness_introspection),
                    ];
                    let weakest = components.iter()
                        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                        .map(|(name, _)| *name)
                        .unwrap_or("");

                    // Peer data
                    let children = data.get("peers")
                        .or_else(|| data.get("children"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let peer_count = children.len();

                    let clone_available = data.get("clone_available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let clone_price = data.get("clone_price")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();

                    let trend_class = if fitness_trend > 0.001 {
                        "fitness-trend fitness-trend--up"
                    } else if fitness_trend < -0.001 {
                        "fitness-trend fitness-trend--down"
                    } else {
                        "fitness-trend fitness-trend--flat"
                    };
                    let trend_arrow = if fitness_trend > 0.001 {
                        "\u{25B2}"
                    } else if fitness_trend < -0.001 {
                        "\u{25BC}"
                    } else {
                        "\u{25C6}"
                    };

                    // Extract cognitive system data from soul_status
                    let soul_data = soul_status.get().unwrap_or_default();
                    let fe = soul_data.get("free_energy");
                    let fe_total = fe.and_then(|f| f.get("F")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                    let fe_regime = fe.and_then(|f| f.get("regime")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                    let fe_trend = fe.and_then(|f| f.get("trend")).and_then(|v| v.as_str()).unwrap_or("0").to_string();
                    let cortex_data = soul_data.get("cortex");
                    let genesis_data = soul_data.get("genesis");
                    let hivemind_data = soul_data.get("hivemind");
                    let synthesis_data = soul_data.get("synthesis");
                    let bench_data = soul_data.get("benchmark");
                    let brain_data = soul_data.get("brain");
                    let transformer_data = soul_data.get("transformer");
                    let eval_data = soul_data.get("evaluation");
                    let total_cycles = soul_data.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);

                    view! {
                        // ═══ TMUX STATUS BAR ═══
                        <div class="tmux-bar">
                            <div class="tmux-bar-section">
                                <span class="tmux-bar-value">"tempo-x402"</span>
                                <span class="tmux-bar-divider">"|"</span>
                                <span class="tmux-bar-label">"v"</span>
                                <span class="tmux-bar-value">{version.clone()}</span>
                                <span class="tmux-bar-divider">"|"</span>
                                <span class="tmux-bar-value">{format_uptime(uptime)}</span>
                                <span class="tmux-bar-divider">"|"</span>
                                <span class="tmux-bar-label">"F="</span>
                                <span class="tmux-bar-value">{fe_total.clone()}</span>
                                <span class={format!("tmux-tag tmux-tag--{}", match fe_regime.as_str() {
                                    "EXPLORE" => "blue", "LEARN" => "purple", "EXPLOIT" => "green", "ANOMALY" => "red", _ => "blue"
                                })}>{fe_regime.clone()}</span>
                            </div>
                            <div class="tmux-bar-section">
                                <span class="tmux-bar-label">"fitness"</span>
                                <span class="tmux-bar-value">{format!("{:.0}%", fitness_total * 100.0)}</span>
                                <span class={trend_class}>
                                    {trend_arrow}{format!("{:+.3}", fitness_trend)}
                                </span>
                                <span class="tmux-bar-divider">"|"</span>
                                <span class="tmux-bar-value">{total_cycles.to_string()}</span>
                                <span class="tmux-bar-label">"cycles"</span>
                                <span class="tmux-bar-divider">"|"</span>
                                <span class="tmux-bar-value">{peer_count.to_string()}</span>
                                <span class="tmux-bar-label">"peers"</span>
                                <span class="tmux-bar-divider">"|"</span>
                                <span class="tmux-bar-value">{total_payments.to_string()}</span>
                                <span class="tmux-bar-label">"payments"</span>
                                {move || { let _ = tick.get(); }}
                            </div>
                        </div>

                        // ═══ TMUX 3-COLUMN GRID ═══
                        <div class="tmux-grid">

                        // ─── LEFT PANE: AGENT ───
                        <div class="tmux-pane">
                            <div class="tmux-pane-title">"AGENT"</div>

                            // Fitness bars
                            <div class="tmux-section">
                                <div class="tmux-section-title">"Fitness"</div>
                                {components.iter().map(|(name, value)| {
                                    let pct = (*value * 100.0) as u64;
                                    let fill_class = format!("tmux-bar-fill tmux-bar-fill--{}", name);
                                    view! {
                                        <div class="tmux-kv">
                                            <span class="tmux-kv-label">{name.to_string()}</span>
                                            <span class="tmux-kv-value">{format!("{}%", pct)}</span>
                                        </div>
                                        <div class="tmux-bar-mini">
                                            <div class={fill_class} style=format!("width: {}%", pct)></div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>

                            // Endpoints compact
                            <div class="tmux-section">
                                <div class="tmux-section-title">{format!("Endpoints ({})", ep_count)}</div>
                                {
                                    // Sort endpoints by payment count (most active first)
                                    let mut sorted_eps = eps.clone();
                                    let analytics_for_sort = analytics_endpoints.clone();
                                    sorted_eps.sort_by(|a, b| {
                                        let slug_a = a.get("slug").and_then(|v| v.as_str()).unwrap_or("");
                                        let slug_b = b.get("slug").and_then(|v| v.as_str()).unwrap_or("");
                                        let pay_a = analytics_for_sort.iter().find(|x| x.get("slug").and_then(|v| v.as_str()) == Some(slug_a))
                                            .and_then(|s| s.get("payment_count")).and_then(|v| v.as_i64()).unwrap_or(0);
                                        let pay_b = analytics_for_sort.iter().find(|x| x.get("slug").and_then(|v| v.as_str()) == Some(slug_b))
                                            .and_then(|s| s.get("payment_count")).and_then(|v| v.as_i64()).unwrap_or(0);
                                        pay_b.cmp(&pay_a)
                                    });
                                    sorted_eps.iter().take(10).map(|ep| {
                                    let slug = ep.get("slug").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let price = ep.get("price").and_then(|v| v.as_str()).unwrap_or("0").to_string();
                                    let ep_stats = analytics_endpoints.iter().find(|a| a.get("slug").and_then(|v| v.as_str()) == Some(&slug));
                                    let payments = ep_stats.and_then(|s| s.get("payment_count")).and_then(|v| v.as_i64()).unwrap_or(0);
                                    view! {
                                        <div class="tmux-endpoint">
                                            <span class="tmux-endpoint-slug">{format!("/g/{}", slug)}</span>
                                            <span class="tmux-endpoint-stat">{format!("${}", price)}</span>
                                            <span class="tmux-endpoint-stat">{format!("{}pay", payments)}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()
                                }
                            </div>
                        </div>

                        // ─── CENTER PANE: SOUL ───
                        <div class="tmux-pane">
                            <div class="tmux-pane-title">"SOUL"</div>
                            <SoulPanel status=soul_status />
                        </div>

                        // ─── RIGHT PANE: INTELLIGENCE ───
                        <div class="tmux-pane">
                            <div class="tmux-pane-title">"INTELLIGENCE"</div>

                            // Free Energy
                            {fe.map(|f| {
                                let comps = f.get("components").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-fe">
                                            <span class="tmux-fe-value">{"F="}{fe_total.clone()}</span>
                                            <span class={format!("tmux-fe-regime tmux-fe-regime--{}", fe_regime.to_lowercase())}>{fe_regime.clone()}</span>
                                        </div>
                                        <div class="tmux-kv">
                                            <span class="tmux-kv-label">"trend"</span>
                                            <span class="tmux-kv-value">{fe_trend.clone()}</span>
                                        </div>
                                        {comps.iter().take(5).map(|c| {
                                            let sys = c.get("system").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                            let surp = c.get("surprise").and_then(|v| v.as_str()).unwrap_or("0").to_string();
                                            view! { <div class="tmux-kv"><span class="tmux-kv-label">{sys}</span><span class="tmux-kv-value">{surp}</span></div> }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                }
                            })}

                            // Benchmark
                            {bench_data.map(|b| {
                                let pass = b.get("pass_at_1").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let elo = b.get("elo_display").and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                let passed = b.get("problems_passed").and_then(|v| v.as_u64()).unwrap_or(0);
                                let attempted = b.get("problems_attempted").and_then(|v| v.as_u64()).unwrap_or(0);
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-section-title">"Benchmark"</div>
                                        <div class="tmux-metric-big">{format!("{:.1}%", pass)}</div>
                                        <div class="tmux-metric-label">"pass@1"</div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"ELO"</span><span class="tmux-kv-value">{elo}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"solved"</span><span class="tmux-kv-value">{format!("{}/{}", passed, attempted)}</span></div>
                                    </div>
                                }
                            })}

                            // Brain (feedforward, 23K params)
                            {brain_data.map(|b| {
                                let params = b.get("parameters").and_then(|v| v.as_u64()).unwrap_or(0);
                                let steps = b.get("train_steps").and_then(|v| v.as_u64()).unwrap_or(0);
                                let loss = b.get("running_loss").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-section-title">"Brain"</div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"params"</span><span class="tmux-kv-value">{format!("{}K", params/1000)}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"steps"</span><span class="tmux-kv-value">{steps.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"loss"</span><span class="tmux-kv-value">{format!("{:.4}", loss)}</span></div>
                                    </div>
                                }
                            })}

                            // Plan Transformer (284K params)
                            {transformer_data.map(|t| {
                                let params = t.get("param_count").and_then(|v| v.as_u64()).unwrap_or(0);
                                let steps = t.get("train_steps").and_then(|v| v.as_u64()).unwrap_or(0);
                                let loss = t.get("last_train_loss").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let generated = t.get("plans_generated").and_then(|v| v.as_u64()).unwrap_or(0);
                                let trained_on = t.get("templates_trained_on").and_then(|v| v.as_u64()).unwrap_or(0);
                                let status_label = if steps >= 50 { "generating" } else if steps > 0 { "learning" } else { "idle" };
                                let status_class = if steps >= 50 { "green" } else if steps > 0 { "yellow" } else { "purple" };
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-section-title">
                                            "Transformer "
                                            <span class={format!("tmux-tag tmux-tag--{}", status_class)}>{status_label}</span>
                                        </div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"params"</span><span class="tmux-kv-value">{format!("{}K", params/1000)}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"steps"</span><span class="tmux-kv-value">{steps.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"loss"</span><span class="tmux-kv-value">{format!("{:.4}", loss)}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"trained on"</span><span class="tmux-kv-value">{format!("{} templates", trained_on)}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"generated"</span><span class="tmux-kv-value">{format!("{} plans", generated)}</span></div>
                                    </div>
                                }
                            })}

                            // Cortex
                            {cortex_data.map(|c| {
                                let experiences = c.get("total_experiences").and_then(|v| v.as_u64()).unwrap_or(0);
                                let accuracy = c.get("prediction_accuracy").and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                let drive = c.get("emotion").and_then(|e| e.get("drive")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                let valence = c.get("emotion").and_then(|e| e.get("valence")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let arousal = c.get("emotion").and_then(|e| e.get("arousal")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let dreams = c.get("dream_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                                let edges = c.get("causal_edges").and_then(|v| v.as_u64()).unwrap_or(0);
                                let drive_tag = match drive.as_str() {
                                    "explore" => "blue", "exploit" => "green", "avoid" => "red", _ => "purple"
                                };
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-section-title">"Cortex"</div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"drive"</span><span class={format!("tmux-tag tmux-tag--{}", drive_tag)}>{drive}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"accuracy"</span><span class="tmux-kv-value">{accuracy}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"valence"</span><span class="tmux-kv-value">{format!("{:+.2}", valence)}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"arousal"</span><span class="tmux-kv-value">{format!("{:.2}", arousal)}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"experiences"</span><span class="tmux-kv-value">{experiences.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"causal edges"</span><span class="tmux-kv-value">{edges.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"dreams"</span><span class="tmux-kv-value">{dreams.to_string()}</span></div>
                                    </div>
                                }
                            })}

                            // Genesis
                            {genesis_data.map(|g| {
                                let templates = g.get("templates").and_then(|v| v.as_u64()).unwrap_or(0);
                                let generation = g.get("generation").and_then(|v| v.as_u64()).unwrap_or(0);
                                let crossovers = g.get("total_crossovers").and_then(|v| v.as_u64()).unwrap_or(0);
                                let mutations = g.get("total_mutations").and_then(|v| v.as_u64()).unwrap_or(0);
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-section-title">"Genesis"</div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"templates"</span><span class="tmux-kv-value">{templates.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"generation"</span><span class="tmux-kv-value">{generation.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"crossovers"</span><span class="tmux-kv-value">{crossovers.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"mutations"</span><span class="tmux-kv-value">{mutations.to_string()}</span></div>
                                    </div>
                                }
                            })}

                            // Hivemind
                            {hivemind_data.map(|h| {
                                let trails = h.get("total_trails").and_then(|v| v.as_u64()).unwrap_or(0);
                                let deposits = h.get("total_deposits").and_then(|v| v.as_u64()).unwrap_or(0);
                                let evap = h.get("evaporation_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-section-title">"Hivemind"</div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"trails"</span><span class="tmux-kv-value">{trails.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"deposits"</span><span class="tmux-kv-value">{deposits.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"evaporation"</span><span class="tmux-kv-value">{evap.to_string()}</span></div>
                                    </div>
                                }
                            })}

                            // Synthesis
                            {synthesis_data.map(|s| {
                                let state = s.get("state").and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                let preds = s.get("total_predictions").and_then(|v| v.as_u64()).unwrap_or(0);
                                let conflicts = s.get("conflicts").and_then(|v| v.as_u64()).unwrap_or(0);
                                let imagined = s.get("total_imagined").and_then(|v| v.as_u64()).unwrap_or(0);
                                let narrative = s.get("self_model").and_then(|m| m.get("narrative")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let state_tag = match state.as_str() {
                                    "coherent" => "green", "conflicted" => "yellow", "exploring" => "blue", "exploiting" => "green", "stuck" => "red", _ => "purple"
                                };
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-section-title">"Synthesis"</div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"state"</span><span class={format!("tmux-tag tmux-tag--{}", state_tag)}>{state}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"predictions"</span><span class="tmux-kv-value">{preds.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"conflicts"</span><span class="tmux-kv-value">{conflicts.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"imagined"</span><span class="tmux-kv-value">{imagined.to_string()}</span></div>
                                        {if !narrative.is_empty() {
                                            Some(view! { <div style="font-size: 10px; color: var(--text-muted); margin-top: 4px; line-height: 1.3;">{narrative}</div> })
                                        } else { None }}
                                    </div>
                                }
                            })}

                            // Evaluation
                            {eval_data.map(|e| {
                                let records = e.get("total_records").and_then(|v| v.as_u64()).unwrap_or(0);
                                let colony_benefit = e.get("colony_benefit").and_then(|c| c.get("avg_sync_benefit")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let syncs = e.get("colony_benefit").and_then(|c| c.get("syncs_measured")).and_then(|v| v.as_u64()).unwrap_or(0);
                                view! {
                                    <div class="tmux-section">
                                        <div class="tmux-section-title">"Evaluation"</div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"records"</span><span class="tmux-kv-value">{records.to_string()}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"colony delta"</span><span class="tmux-kv-value">{format!("{:+.3}", colony_benefit)}</span></div>
                                        <div class="tmux-kv"><span class="tmux-kv-label">"syncs"</span><span class="tmux-kv-value">{syncs.to_string()}</span></div>
                                    </div>
                                }
                            })}
                        </div>

                        </div> // end tmux-grid

                        // ═══ TERMINAL PANEL (bottom, full width) ═══
                        <div class="tmux-terminal">
                            <div class="tmux-terminal-tabs">
                                <button class="tmux-terminal-tab tmux-terminal-tab--active">"NETWORK"</button>
                                <button class="tmux-terminal-tab">"ACTIVITY"</button>
                                <button class="tmux-terminal-tab">"ENDPOINTS"</button>
                            </div>
                            <div class="tmux-terminal-body">
                                // Peer network as terminal rows
                                {children.iter().map(|child| {
                                    let id = child.get("instance_id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let url = child.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let status = child.get("status").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let dot_class = if status == "running" { "tmux-peer-dot tmux-peer-dot--running" } else { "tmux-peer-dot tmux-peer-dot--stopped" };
                                    let short = if id.len() > 12 { format!("{}...", &id[..12]) } else { id };
                                    view! {
                                        <div class="tmux-terminal-row">
                                            <span class={dot_class}></span>
                                            <span class="tmux-terminal-badge tmux-terminal-badge--peer">"PEER"</span>
                                            <span>{short}</span>
                                            <span class="tmux-terminal-msg">
                                                {if !url.is_empty() {
                                                    view! { <a href={url.clone()} target="_blank" style="color: var(--accent);">{url}</a> }.into_view()
                                                } else {
                                                    view! { <span style="color: var(--text-dim);">"no url"</span> }.into_view()
                                                }}
                                            </span>
                                            <span class="tmux-tag tmux-tag--green">{status}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}

                                // Hivemind pheromone activity
                                {
                                    let hive_data = soul_data.get("hivemind");
                                    let deposits = hive_data.and_then(|h| h.get("total_deposits")).and_then(|v| v.as_u64()).unwrap_or(0);
                                    let trails = hive_data.and_then(|h| h.get("total_trails")).and_then(|v| v.as_u64()).unwrap_or(0);
                                    let evap = hive_data.and_then(|h| h.get("evaporation_cycles")).and_then(|v| v.as_u64()).unwrap_or(0);
                                    let attractants = hive_data
                                        .and_then(|h| h.get("top_attractants"))
                                        .and_then(|v| v.as_array())
                                        .cloned()
                                        .unwrap_or_default();

                                    view! {
                                        <div class="tmux-terminal-row">
                                            <span class="tmux-terminal-badge tmux-terminal-badge--sync">"HIVE"</span>
                                            <span class="tmux-terminal-msg">
                                                {format!("{} trails, {} deposits, {} evaporation cycles", trails, deposits, evap)}
                                            </span>
                                        </div>
                                        {attractants.iter().take(3).map(|a| {
                                            let resource = a.get("resource").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                            let intensity = a.get("intensity").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                            let reinforced = a.get("reinforced").and_then(|v| v.as_u64()).unwrap_or(0);
                                            view! {
                                                <div class="tmux-terminal-row">
                                                    <span class="tmux-terminal-ts"></span>
                                                    <span class="tmux-terminal-badge tmux-terminal-badge--tx">"TRAIL"</span>
                                                    <span>{resource}</span>
                                                    <span class="tmux-terminal-msg">
                                                        {format!("intensity {:.0}% reinforced {}x", intensity * 100.0, reinforced)}
                                                    </span>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    }
                                }

                                // Colony sync info
                                {
                                    let eval = soul_data.get("evaluation");
                                    let syncs = eval.and_then(|e| e.get("colony_benefit")).and_then(|c| c.get("syncs_measured")).and_then(|v| v.as_u64()).unwrap_or(0);
                                    let delta = eval.and_then(|e| e.get("colony_benefit")).and_then(|c| c.get("avg_sync_benefit")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    if syncs > 0 {
                                        Some(view! {
                                            <div class="tmux-terminal-row">
                                                <span class="tmux-terminal-badge tmux-terminal-badge--sync">"SYNC"</span>
                                                <span class="tmux-terminal-msg">
                                                    {format!("{} cognitive syncs completed, avg benefit {:+.3}", syncs, delta)}
                                                </span>
                                            </div>
                                        })
                                    } else {
                                        None
                                    }
                                }
                            </div>
                        </div>

                        // Sticky footer with external links
                        <div class="tmux-footer">
                            <a href="https://docs.rs/tempo-x402" target="_blank">"docs"</a>
                            <span class="tmux-footer-dot">{"\u{00B7}"}</span>
                            <a href="https://crates.io/crates/tempo-x402" target="_blank">"crates"</a>
                            <span class="tmux-footer-dot">{"\u{00B7}"}</span>
                            <a href="https://github.com/compusophy/tempo-x402" target="_blank">"github"</a>
                            <span class="tmux-footer-spacer"></span>
                            <span class="tmux-footer-text">{concat!("tempo-x402 v", env!("CARGO_PKG_VERSION"))}</span>
                        </div>

                        // Old layout sections — hidden, kept for compilation only
                        <div style="display:none!important;height:0;overflow:hidden;position:absolute;pointer-events:none">
                        {if true {
                            Some(view! {
                                <div class="fitness-panel">
                                    <h2>"Fitness"</h2>
                                    <div class="fitness-overview">
                                        <span class="fitness-score-big">
                                            {format!("{:.0}%", fitness_total * 100.0)}
                                        </span>
                                        <div class={trend_class}>
                                            <span class="fitness-trend-arrow">{trend_arrow}</span>
                                            {format!("{:+.3}", fitness_trend)}
                                        </div>
                                    </div>
                                    <div class="fitness-bars">
                                        {components.iter().map(|(name, value)| {
                                            let is_weakest = *name == weakest;
                                            let row_class = if is_weakest {
                                                "fitness-bar-row fitness-bar-row--weakest"
                                            } else {
                                                "fitness-bar-row"
                                            };
                                            let fill_class = format!("fitness-bar-fill fitness-bar-fill--{}", name);
                                            let pct = (*value * 100.0) as u64;
                                            view! {
                                                <div class={row_class}>
                                                    <span class="fitness-bar-label">{name.to_string()}</span>
                                                    <div class="fitness-bar-track">
                                                        <div class={fill_class}
                                                            style=format!("width: {}%", pct)>
                                                        </div>
                                                    </div>
                                                    <span class="fitness-bar-value">{format!("{}%", pct)}</span>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            })
                        } else {
                            None
                        }}

                        // Peer network panel
                        {if !children.is_empty() {
                            Some(view! {
                                <div class="peer-panel">
                                    <h2>{format!("Peer Network ({})", peer_count)}</h2>
                                    <div class="peer-list">
                                        {children.iter().map(|child| {
                                            let instance_id = child.get("instance_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            let url = child.get("url")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let peer_status = child.get("status")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            let dot_class = match peer_status.as_str() {
                                                "running" => "peer-status-dot peer-status-dot--running",
                                                "stopped" | "crashed" => "peer-status-dot peer-status-dot--stopped",
                                                _ => "peer-status-dot peer-status-dot--unknown",
                                            };
                                            let label_class = match peer_status.as_str() {
                                                "running" => "peer-status-label peer-status-label--running",
                                                "stopped" | "crashed" => "peer-status-label peer-status-label--stopped",
                                                _ => "peer-status-label",
                                            };
                                            let short_id = if instance_id.len() > 12 {
                                                format!("{}...", &instance_id[..12])
                                            } else {
                                                instance_id.clone()
                                            };
                                            view! {
                                                <div class="peer-item">
                                                    <span class={dot_class}></span>
                                                    <div class="peer-info">
                                                        <div class="peer-id">{short_id}</div>
                                                        {if !url.is_empty() {
                                                            Some(view! {
                                                                <div class="peer-url">
                                                                    <a href={url.clone()} target="_blank">{url}</a>
                                                                </div>
                                                            })
                                                        } else {
                                                            None
                                                        }}
                                                    </div>
                                                    <span class={label_class}>{peer_status}</span>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            })
                        } else {
                            None
                        }}

                        // Clone section (hidden — moved to demo page)
                        {if false { Some(view! {
                        <div class="clone-section">
                            <h2>"Clone Instance"</h2>
                            <button
                                class="btn clone-btn"
                                disabled=move || {
                                    if !clone_available {
                                        return true;
                                    }
                                    let (wallet, _) = expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();
                                    wallet.get().mode == WalletMode::Disconnected || clone_loading.get()
                                }
                                on:click=move |_| {
                                    if !clone_available {
                                        return;
                                    }
                                    let (wallet, _) = expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();
                                    let w = wallet.get();
                                    set_clone_loading.set(true);
                                    set_clone_result.set(None);
                                    spawn_local(async move {
                                        let result = api::clone_instance(&w).await;
                                        set_clone_result.set(Some(result));
                                        set_clone_loading.set(false);
                                    });
                                }
                            >
                                {let cp = clone_price.clone(); move || if clone_loading.get() {
                                    "Cloning...".to_string()
                                } else if clone_available {
                                    format!("Clone ({})", cp)
                                } else {
                                    "Clone unavailable".to_string()
                                }}
                            </button>

                            {move || {
                                if !clone_available {
                                    Some(view! {
                                        <p class="hint">"Cloning not configured on this instance"</p>
                                    })
                                } else {
                                    let (wallet, _) = expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();
                                    (wallet.get().mode == WalletMode::Disconnected).then(|| view! {
                                        <p class="hint">"Connect wallet to clone"</p>
                                    })
                                }
                            }}

                            {move || clone_result.get().map(|res| match res {
                                Ok(cr) => {
                                    let url = cr.url.clone();
                                    let branch = cr.branch.clone();
                                    let tx = cr.transaction.clone();
                                    let new_id = cr.instance_id.clone().unwrap_or_default();
                                    view! {
                                        <div class="clone-success">
                                            <p>"Clone created: " <code>{new_id}</code></p>
                                            {url.map(|u| view! {
                                                <p>"URL: " <a href=u.clone() target="_blank">{u}</a></p>
                                            })}
                                            {branch.map(|b| view! {
                                                <p>"Branch: " <code>{b}</code></p>
                                            })}
                                            {tx.map(|t| {
                                                let explorer = format!("https://explore.moderato.tempo.xyz/tx/{}", t);
                                                view! {
                                                    <p>"Tx: " <a href=explorer target="_blank"><code>{t}</code></a></p>
                                                }
                                            })}
                                        </div>
                                    }.into_view()
                                }
                                Err(e) => view! {
                                    <p class="error-text">{e}</p>
                                }.into_view(),
                            })}
                        </div>

                        // Soul panel
                        <SoulPanel status=soul_status />

                        // Endpoints table
                        <div class="endpoints-section">
                            <h2>{format!("Registered Endpoints ({})", ep_count)}</h2>
                            {if eps.is_empty() {
                                view! { <p class="empty">"No endpoints registered yet. Register one from the Demo page."</p> }.into_view()
                            } else {
                                let analytics_eps = analytics_endpoints.clone();
                                view! {
                                    <div class="endpoints-table">
                                        <div class="endpoint-row endpoint-header">
                                            <span class="endpoint-slug">"Endpoint"</span>
                                            <span class="endpoint-price">"Price"</span>
                                            <span class="endpoint-stat">"Calls"</span>
                                            <span class="endpoint-stat">"Payments"</span>
                                            <span class="endpoint-stat">"Revenue"</span>
                                            <span class="endpoint-desc">"Description"</span>
                                        </div>
                                        {eps.iter().map(|ep| {
                                            let slug = ep.get("slug")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("?")
                                                .to_string();
                                            let price = ep.get("price")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("?")
                                                .to_string();
                                            let description = ep.get("description")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let gateway_url = ep.get("gateway_url")
                                                .and_then(|v| v.as_str())
                                                .map(String::from);

                                            let ep_stats = analytics_eps.iter().find(|a| {
                                                a.get("slug").and_then(|v| v.as_str()) == Some(&slug)
                                            });
                                            let calls = ep_stats
                                                .and_then(|s| s.get("request_count"))
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0);
                                            let payments = ep_stats
                                                .and_then(|s| s.get("payment_count"))
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0);
                                            let revenue = ep_stats
                                                .and_then(|s| s.get("revenue_usd"))
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("$0")
                                                .to_string();

                                            view! {
                                                <div class="endpoint-row">
                                                    <span class="endpoint-slug">
                                                        {gateway_url.as_ref().map(|url| view! {
                                                            <a href=url.clone() target="_blank">{format!("/g/{}", slug)}</a>
                                                        })}
                                                        {if gateway_url.is_none() {
                                                            Some(view! { <span>{format!("/g/{}", slug)}</span> })
                                                        } else {
                                                            None
                                                        }}
                                                    </span>
                                                    <span class="endpoint-price">{format!("${}", price)}</span>
                                                    <span class="endpoint-stat">{calls.to_string()}</span>
                                                    <span class="endpoint-stat">{payments.to_string()}</span>
                                                    <span class="endpoint-stat">{revenue}</span>
                                                    <span class="endpoint-desc">{description}</span>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                }.into_view()
                            }}
                        </div>
                    }) } else { None }}
                    </div> // end hidden wrapper
                    }
                }}
            </Show>
        </div>
    }
}

/// Format a unix timestamp as relative time (e.g., "2m ago")
fn format_relative_time(unix_ts: i64) -> String {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let diff = now - unix_ts;
    if diff < 0 {
        return "just now".to_string();
    }
    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

/// Format a unix timestamp as HH:MM
fn format_timestamp(unix_ts: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((unix_ts as f64) * 1000.0);
    let h = date.get_hours();
    let m = date.get_minutes();
    format!("{:02}:{:02}", h, m)
}

/// Format seconds into human-readable uptime string
fn format_uptime(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}
