use crate::api;
use crate::WalletState;
use gloo_timers::callback::Interval;
use leptos::*;

use super::wallet_panel::WalletButtons;

/// Single-page cockpit — Bloomberg terminal x spaceship bridge.
/// All panels visible at once, no page navigation.
#[component]
pub fn CockpitPage() -> impl IntoView {
    let (wallet, set_wallet) =
        expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();

    // ── Data signals ──
    let (info, set_info) = create_signal(None::<serde_json::Value>);
    let (soul, set_soul) = create_signal(None::<serde_json::Value>);
    let (system, set_system) = create_signal(None::<serde_json::Value>);
    let (cartridges, set_cartridges) = create_signal(Vec::<serde_json::Value>::new());
    let (loading, set_loading) = create_signal(true);

    // ── Chat signals ──
    let (chat_msgs, set_chat_msgs) = create_signal(Vec::<ChatMsg>::new());
    let (chat_input, set_chat_input) = create_signal(String::new());
    let (chat_loading, set_chat_loading) = create_signal(false);
    let (chat_error, set_chat_error) = create_signal(None::<String>);
    let (session_id, set_session_id) = create_signal(None::<String>);
    let (pending_plan, set_pending_plan) = create_signal(None::<serde_json::Value>);
    let chat_ref = create_node_ref::<html::Div>();

    // ── Bottom panel tab ──
    let (active_tab, set_active_tab) = create_signal("chat");

    // ── Fetch all data ──
    let fetch_all = move || {
        spawn_local(async move {
            // Instance info
            let base = api::gateway_base_url();
            if let Ok(resp) = gloo_net::http::Request::get(&format!("{}/instance/info", base))
                .send()
                .await
            {
                if resp.ok() {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        set_info.set(Some(data));
                    }
                }
            }

            // Soul status
            if let Ok(data) = api::fetch_soul_status().await {
                set_soul.set(Some(data));
            }

            // System metrics
            if let Ok(resp) =
                gloo_net::http::Request::get(&format!("{}/soul/system", base))
                    .send()
                    .await
            {
                if resp.ok() {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        set_system.set(Some(data));
                    }
                }
            }

            // Cartridges
            if let Ok(data) = api::fetch_json("/c").await {
                if let Some(arr) = data.get("cartridges").and_then(|v| v.as_array()) {
                    set_cartridges.set(arr.clone());
                }
            }

            // Pending plan
            if let Ok(plan) = api::get_pending_plan().await {
                if plan.is_null() {
                    set_pending_plan.set(None);
                } else {
                    set_pending_plan.set(Some(plan));
                }
            }

            set_loading.set(false);
        });
    };

    fetch_all();

    // Auto-refresh every 8s
    let interval = Interval::new(8_000, move || {
        fetch_all();
    });
    on_cleanup(move || drop(interval));

    // ── Chat send ──
    let scroll_bottom = move || {
        if let Some(el) = chat_ref.get() {
            el.set_scroll_top(el.scroll_height());
        }
    };

    let now_ts = move || (js_sys::Date::now() / 1000.0) as i64;

    let do_send = move || {
        let msg = chat_input.get().trim().to_string();
        if msg.is_empty() || chat_loading.get() {
            return;
        }
        let ts = now_ts();
        set_chat_msgs.update(|msgs| {
            msgs.push(ChatMsg {
                role: "user",
                content: msg.clone(),
                tools: vec![],
                timestamp: ts,
                hemisphere: None,
            });
        });
        set_chat_input.set(String::new());
        set_chat_loading.set(true);
        set_chat_error.set(None);

        let sid = session_id.get();
        spawn_local(async move {
            match api::send_soul_chat(&msg, sid.as_deref()).await {
                Ok(resp) => {
                    if let Some(sid) = resp.get("session_id").and_then(|v| v.as_str()) {
                        set_session_id.set(Some(sid.to_string()));
                    }
                    let reply = resp
                        .get("reply")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tools = resp
                        .get("tool_executions")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let hemisphere = resp
                        .get("hemisphere")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let ts = (js_sys::Date::now() / 1000.0) as i64;
                    set_chat_msgs.update(|msgs| {
                        msgs.push(ChatMsg {
                            role: "soul",
                            content: reply,
                            tools,
                            timestamp: ts,
                            hemisphere,
                        });
                    });
                }
                Err(e) => set_chat_error.set(Some(e)),
            }
            set_chat_loading.set(false);
            scroll_bottom();
        });
        scroll_bottom();
    };

    let on_chat_key = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            do_send();
        }
    };

    let on_chat_click = move |_: web_sys::MouseEvent| {
        do_send();
    };

    let clear_chat = move |_: web_sys::MouseEvent| {
        set_chat_msgs.set(Vec::new());
        set_chat_error.set(None);
        set_session_id.set(None);
    };

    let approve_handler = move |_: web_sys::MouseEvent| {
        if let Some(plan) = pending_plan.get() {
            if let Some(plan_id) = plan.get("id").and_then(|v| v.as_str()) {
                let plan_id = plan_id.to_string();
                spawn_local(async move {
                    let _ = api::approve_plan(&plan_id).await;
                    set_pending_plan.set(None);
                });
            }
        }
    };

    let reject_handler = move |_: web_sys::MouseEvent| {
        if let Some(plan) = pending_plan.get() {
            if let Some(plan_id) = plan.get("id").and_then(|v| v.as_str()) {
                let plan_id = plan_id.to_string();
                spawn_local(async move {
                    let _ = api::reject_plan(&plan_id, None).await;
                    set_pending_plan.set(None);
                });
            }
        }
    };

    view! {
        <div class="cockpit">
            // ═══ TOP BAR ═══
            <div class="top-bar">
                <div class="top-bar-left">
                    <span class="brand">"tempo-x402"</span>
                    {move || {
                        let d = info.get().unwrap_or_default();
                        let version = d.get("version").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                        let instance = d.get("instance_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let short_id = if instance.len() > 10 { format!("{}..{}", &instance[..4], &instance[instance.len()-4..]) } else { instance };
                        let uptime = d.get("uptime_seconds").and_then(|v| v.as_i64()).unwrap_or(0);
                        let balance = d.get("balance").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                        view! {
                            <span class="sep">"|"</span>
                            <span class="dim">"v"</span><span class="val">{version}</span>
                            <span class="sep">"|"</span>
                            <span class="val">{short_id}</span>
                            <span class="sep">"|"</span>
                            <span class="accent">{balance}" pathUSD"</span>
                            <span class="sep">"|"</span>
                            <span class="dim">{format_uptime(uptime)}</span>
                        }
                    }}
                </div>
                <div class="top-bar-right">
                    <WalletButtons wallet=wallet set_wallet=set_wallet />
                </div>
            </div>

            // ═══ MAIN 3-COLUMN GRID ═══
            <Show when=move || !loading.get() || info.get().is_some() fallback=|| view! { <div class="loading-text">"Loading cockpit..."</div> }>
                <div class="cockpit-main">

                // ─── LEFT COLUMN: PSI + FITNESS ───
                <div class="left-col">
                    // PSI Panel
                    <div class="panel">
                        <div class="panel-title">{"\u{03A8}(t)"}</div>
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            let colony = s.get("colony");
                            let psi = colony.and_then(|c| c.get("psi")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let psi_trend = colony.and_then(|c| c.get("psi_trend")).and_then(|v| v.as_f64()).unwrap_or(0.0);

                            let fe = s.get("free_energy");
                            let f_val = fe.and_then(|f| f.get("F")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                            let regime = fe.and_then(|f| f.get("regime")).and_then(|v| v.as_str()).unwrap_or("--").to_string();

                            let trend_class = if psi_trend > 0.001 { "psi-trend up" } else if psi_trend < -0.001 { "psi-trend down" } else { "psi-trend" };
                            let trend_arrow = if psi_trend > 0.001 { "\u{2191}" } else if psi_trend < -0.001 { "\u{2193}" } else { "\u{2192}" };

                            view! {
                                <div class="psi-value">{format!("\u{03A8}={:.4}", psi)}</div>
                                <div class=trend_class>{trend_arrow}{format!("{:+.4}", psi_trend)}</div>
                                <div class="fe-row">
                                    <span class="fe-value">{"F="}{f_val}</span>
                                    <span class={format!("regime-badge {}", regime.to_lowercase())}>{regime}</span>
                                </div>
                            }
                        }}
                    </div>

                    // Fitness Panel
                    <div class="panel" style="flex:1">
                        <div class="panel-title">"FITNESS"</div>
                        {move || {
                            let d = info.get().unwrap_or_default();
                            let fitness = d.get("fitness");
                            let total = fitness.and_then(|f| f.get("total")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let trend = fitness.and_then(|f| f.get("trend")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let components = [
                                ("eco", fitness.and_then(|f| f.get("economic")).and_then(|v| v.as_f64()).unwrap_or(0.0)),
                                ("exec", fitness.and_then(|f| f.get("execution")).and_then(|v| v.as_f64()).unwrap_or(0.0)),
                                ("evol", fitness.and_then(|f| f.get("evolution")).and_then(|v| v.as_f64()).unwrap_or(0.0)),
                                ("coord", fitness.and_then(|f| f.get("coordination")).and_then(|v| v.as_f64()).unwrap_or(0.0)),
                                ("intro", fitness.and_then(|f| f.get("introspection")).and_then(|v| v.as_f64()).unwrap_or(0.0)),
                            ];
                            let trend_arrow = if trend > 0.001 { "\u{2191}" } else if trend < -0.001 { "\u{2193}" } else { "" };

                            view! {
                                <div style="display:flex;align-items:baseline;gap:4px;margin-bottom:4px">
                                    <span style="font-size:14px;font-weight:700;color:var(--green)">{format!("{:.0}%", total * 100.0)}</span>
                                    <span style="font-size:9px;color:var(--text-dim)">{trend_arrow}{format!("{:+.3}", trend)}</span>
                                </div>
                                {components.iter().map(|(name, value)| {
                                    let pct = (*value * 100.0) as u64;
                                    let fill_class = if pct < 30 { "fitness-fill low" } else if pct < 60 { "fitness-fill mid" } else { "fitness-fill high" };
                                    view! {
                                        <div class="fitness-row">
                                            <span class="fitness-label">{name.to_string()}</span>
                                            <div class="fitness-bar">
                                                <div class=fill_class style=format!("width:{}%", pct)></div>
                                            </div>
                                            <span class="fitness-pct">{format!("{}%", pct)}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            }
                        }}

                        // Goals
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            let goals = s.get("goals")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            if goals.is_empty() { return view! { <div></div> }.into_view(); }
                            view! {
                                <div class="panel-title" style="margin-top:6px">{format!("GOALS ({})", goals.len())}</div>
                                {goals.iter().take(5).map(|g| {
                                    let desc = g.get("description").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let status = g.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                    let priority = g.get("priority").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let truncated = if desc.len() > 60 {
                                        let mut end = 60;
                                        while end > 0 && !desc.is_char_boundary(end) { end -= 1; }
                                        format!("{}...", &desc[..end])
                                    } else { desc };
                                    view! {
                                        <div class="goal-item">
                                            <span class={format!("goal-status {}", status)}>{status.chars().next().unwrap_or('?').to_uppercase().to_string()}</span>
                                            <span class="goal-priority">{format!("P{}", priority)}</span>
                                            <span class="goal-desc">{truncated}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            }.into_view()
                        }}
                    </div>
                </div>

                // ─── CENTER COLUMN: COGNITIVE SYSTEMS + BENCHMARK ───
                <div class="center-col">
                    // Cognitive Systems Grid
                    <div class="panel">
                        <div class="panel-title">"COGNITIVE SYSTEMS"</div>
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            view! { <div class="cog-grid">
                                // BRAIN
                                {render_cog_item("BRAIN",
                                    s.get("brain").map(|b| vec![
                                        ("params", format!("{}K", b.get("parameters").and_then(|v| v.as_u64()).unwrap_or(0) / 1000)),
                                        ("loss", format!("{:.3}", b.get("running_loss").and_then(|v| v.as_f64()).unwrap_or(0.0))),
                                        ("steps", b.get("train_steps").and_then(|v| v.as_u64()).unwrap_or(0).to_string()),
                                    ]).unwrap_or_default()
                                )}
                                // TRANSFORMER
                                {render_cog_item("XFORMER",
                                    s.get("transformer").map(|t| vec![
                                        ("params", format!("{}K", t.get("param_count").and_then(|v| v.as_u64()).unwrap_or(0) / 1000)),
                                        ("loss", format!("{:.3}", t.get("last_train_loss").and_then(|v| v.as_f64()).unwrap_or(0.0))),
                                        ("plans", t.get("plans_generated").and_then(|v| v.as_u64()).unwrap_or(0).to_string()),
                                    ]).unwrap_or_default()
                                )}
                                // QUALITY
                                {s.get("quality").map(|q| {
                                    render_cog_item("QUALITY", vec![
                                        ("steps", q.get("train_steps").and_then(|v| v.as_u64()).unwrap_or(0).to_string()),
                                        ("loss", format!("{:.3}", q.get("running_loss").and_then(|v| v.as_f64()).unwrap_or(0.0))),
                                    ])
                                }).unwrap_or_else(|| render_cog_item("QUALITY", vec![]))}
                                // CODEGEN
                                {s.get("codegen").map(|c| {
                                    let steps = c.get("model_steps").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let solutions = c.get("solutions_stored").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let loss = c.get("model_loss").and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                    let can_gen = c.get("can_generate").and_then(|v| v.as_bool()).unwrap_or(false);
                                    let params = c.get("model_params").and_then(|v| v.as_u64()).unwrap_or(0);
                                    render_cog_item("CODEGEN", vec![
                                        ("params", format!("{}M", params / 1_000_000)),
                                        ("steps", steps.to_string()),
                                        ("data", solutions.to_string()),
                                        ("gen", if can_gen { "YES" } else { "no" }.to_string()),
                                    ])
                                }).unwrap_or_else(|| render_cog_item("CODEGEN", vec![("status", "not loaded".to_string())]))}
                                // CORTEX
                                {render_cog_item("CORTEX",
                                    s.get("cortex").map(|c| {
                                        let drive = c.get("emotion").and_then(|e| e.get("drive")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                        let acc = c.get("prediction_accuracy").and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                        let val = c.get("emotion").and_then(|e| e.get("valence")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                        vec![
                                            ("drive", drive),
                                            ("acc", acc),
                                            ("val", format!("{:+.2}", val)),
                                        ]
                                    }).unwrap_or_default()
                                )}
                                // GENESIS
                                {render_cog_item("GENESIS",
                                    s.get("genesis").map(|g| vec![
                                        ("gen", g.get("generation").and_then(|v| v.as_u64()).unwrap_or(0).to_string()),
                                        ("tmpl", g.get("templates").and_then(|v| v.as_u64()).unwrap_or(0).to_string()),
                                        ("mut", g.get("total_mutations").and_then(|v| v.as_u64()).unwrap_or(0).to_string()),
                                    ]).unwrap_or_default()
                                )}
                                // HIVEMIND
                                {render_cog_item("HIVEMND",
                                    s.get("hivemind").map(|h| vec![
                                        ("trails", h.get("total_trails").and_then(|v| v.as_u64()).unwrap_or(0).to_string()),
                                        ("dep", h.get("total_deposits").and_then(|v| v.as_u64()).unwrap_or(0).to_string()),
                                    ]).unwrap_or_default()
                                )}
                                // SYNTHESIS
                                {render_cog_item("SYNTH",
                                    s.get("synthesis").map(|sy| {
                                        let state = sy.get("state").and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                        let conflicts = sy.get("conflicts").and_then(|v| v.as_u64()).unwrap_or(0);
                                        vec![
                                            ("state", state),
                                            ("conf", conflicts.to_string()),
                                        ]
                                    }).unwrap_or_default()
                                )}
                                // EVALUATION
                                {render_cog_item("EVAL",
                                    s.get("evaluation").map(|e| {
                                        let delta = e.get("colony_benefit").and_then(|c| c.get("avg_sync_benefit")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                        let records = e.get("total_records").and_then(|v| v.as_u64()).unwrap_or(0);
                                        vec![
                                            ("delta", format!("{:+.3}", delta)),
                                            ("rec", records.to_string()),
                                        ]
                                    }).unwrap_or_default()
                                )}
                            </div> }
                        }}
                    </div>

                    // Benchmark Panel
                    <div class="panel">
                        <div class="panel-title">"BENCHMARK"</div>
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            match s.get("benchmark") {
                                Some(b) => {
                                    let pass = b.get("pass_at_1").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    let elo = b.get("elo_display").and_then(|v| v.as_str()).unwrap_or("--").to_string();
                                    let passed = b.get("problems_passed").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let attempted = b.get("problems_attempted").and_then(|v| v.as_u64()).unwrap_or(0);
                                    view! {
                                        <div class="bench-row">
                                            <span class="bench-big">{format!("{:.1}%", pass)}</span>
                                            <span class="bench-label">"pass@1"</span>
                                            <span class="bench-label">{elo}</span>
                                            <span class="bench-label">{format!("{}/{} solved", passed, attempted)}</span>
                                        </div>
                                    }.into_view()
                                }
                                None => {
                                    let cycles = s.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                                    view! {
                                        <span class="empty-text">{format!("waiting (cycle {})", cycles)}</span>
                                    }.into_view()
                                }
                            }
                        }}
                    </div>

                    // Active Plan
                    <div class="panel" style="flex:1;min-height:0">
                        <div class="panel-title">"ACTIVE PLAN"</div>
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            let active = s.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                            match s.get("active_plan") {
                                Some(p) => {
                                    let status = p.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                    let current = p.get("current_step").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let total = p.get("total_steps").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let replan = p.get("replan_count").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let pct = if total > 0 { (current as f64 / total as f64 * 100.0) as u64 } else { 0 };
                                    let badge_class = match status.as_str() {
                                        "executing" => "plan-progress-badge executing",
                                        "pending_approval" => "plan-progress-badge pending",
                                        "completed" => "plan-progress-badge completed",
                                        "failed" => "plan-progress-badge failed",
                                        _ => "plan-progress-badge",
                                    };
                                    view! {
                                        <div class="plan-progress">
                                            <div class="plan-progress-info">
                                                <span class=badge_class>{status}</span>
                                                <span style="color:var(--text)">{format!("{}/{}", current, total)}</span>
                                                {(replan > 0).then(|| view! {
                                                    <span style="color:var(--text-dim)">{format!("(replan #{})", replan)}</span>
                                                })}
                                            </div>
                                            <div class="plan-progress-track">
                                                <div class="plan-progress-fill" style=format!("width:{}%", pct)></div>
                                            </div>
                                        </div>
                                    }.into_view()
                                }
                                None if active => view! {
                                    <span class="empty-text">"no plan -- waiting for next cycle"</span>
                                }.into_view(),
                                _ => view! {
                                    <span class="empty-text">"soul not active"</span>
                                }.into_view(),
                            }
                        }}

                        // Recent thoughts
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            let thoughts: Vec<serde_json::Value> = s.get("recent_thoughts")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            if thoughts.is_empty() { return view! { <div></div> }.into_view(); }
                            view! {
                                <div class="panel-title" style="margin-top:6px">{format!("THOUGHTS ({})", thoughts.len())}</div>
                                {thoughts.iter().take(5).map(|t| {
                                    let thought_type = t.get("type").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let content = t.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let created_at = t.get("created_at").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let abbr = match thought_type.as_str() {
                                        "observation" => "obs",
                                        "reasoning" => "rsn",
                                        "decision" => "dec",
                                        "reflection" => "ref",
                                        "tool_execution" => "tool",
                                        _ => &thought_type,
                                    };
                                    let truncated = if content.len() > 80 {
                                        let mut end = 80;
                                        while end > 0 && !content.is_char_boundary(end) { end -= 1; }
                                        format!("{}...", &content[..end])
                                    } else { content };
                                    let log_class = match thought_type.as_str() {
                                        "observation" => "log-type obs",
                                        "reasoning" => "log-type reason",
                                        "decision" => "log-type decide",
                                        "reflection" => "log-type reflect",
                                        "tool_execution" => "log-type tool",
                                        "mutation" => "log-type mutate",
                                        _ => "log-type",
                                    };
                                    view! {
                                        <div class="log-line">
                                            <span class="log-ts">{format_relative_time(created_at)}</span>
                                            <span class=log_class>{abbr.to_string()}</span>
                                            {truncated}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            }.into_view()
                        }}
                    </div>
                </div>

                // ─── RIGHT COLUMN: PROCESSES + CARTRIDGES + COLONY ───
                <div class="right-col">
                    // Processes (current soul activity)
                    <div class="panel">
                        <div class="panel-title">"PROCESSES"</div>
                        {move || {
                            let s = soul.get().unwrap_or_default();
                            let active = s.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                            let dormant = s.get("dormant").and_then(|v| v.as_bool()).unwrap_or(false);
                            let mode = s.get("mode").and_then(|v| v.as_str()).unwrap_or("inactive").to_string();
                            let tools_on = s.get("tools_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                            let coding_on = s.get("coding_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                            let total_cycles = s.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);

                            let status_class = if active && !dormant { "process-indicator ok" } else if dormant { "process-indicator running" } else { "process-indicator fail" };
                            let status_label = if active && !dormant { "active" } else if dormant { "dormant" } else { "inactive" };

                            view! {
                                <div class="process-item">
                                    <span class=status_class></span>
                                    <span class="process-cmd">{format!("soul [{}]", mode)}</span>
                                    <span class="process-status">{status_label}</span>
                                </div>
                                <div class="process-item">
                                    <span class={if tools_on { "process-indicator ok" } else { "process-indicator fail" }}></span>
                                    <span class="process-cmd">"tools"</span>
                                    <span class="process-status">{if tools_on { "on" } else { "off" }}</span>
                                </div>
                                <div class="process-item">
                                    <span class={if coding_on { "process-indicator ok" } else { "process-indicator fail" }}></span>
                                    <span class="process-cmd">"coding"</span>
                                    <span class="process-status">{if coding_on { "on" } else { "off" }}</span>
                                </div>
                                <div class="process-item">
                                    <span class="process-indicator ok"></span>
                                    <span class="process-cmd">"cycles"</span>
                                    <span class="process-status">{total_cycles.to_string()}</span>
                                </div>
                            }
                        }}
                    </div>

                    // Cartridges
                    <div class="panel">
                        <div class="panel-title">"CARTRIDGES"</div>
                        {move || {
                            let carts = cartridges.get();
                            if carts.is_empty() {
                                return view! { <span class="empty-text">"no cartridges"</span> }.into_view();
                            }
                            view! {
                                {carts.iter().map(|c| {
                                    let slug = c.get("slug").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let compiled = c.get("compiled").and_then(|v| v.as_bool()).unwrap_or(false);
                                    let dot_class = if compiled { "cart-dot live" } else { "cart-dot fail" };
                                    let status = if compiled { "live" } else { "fail" };
                                    view! {
                                        <div class="cart-item">
                                            <span class=dot_class></span>
                                            <span class="cart-slug">{slug}</span>
                                            <span class="cart-status">{"["}{status}{"]"}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            }.into_view()
                        }}
                    </div>

                    // Colony
                    <div class="panel" style="flex:1;min-height:0">
                        <div class="panel-title">"COLONY"</div>
                        {move || {
                            let d = info.get().unwrap_or_default();
                            let s = soul.get().unwrap_or_default();
                            let peers = d.get("peers")
                                .or_else(|| d.get("children"))
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();

                            // Show self first
                            let self_fitness = d.get("fitness").and_then(|f| f.get("total")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let self_id = d.get("instance_id").and_then(|v| v.as_str()).unwrap_or("self").to_string();
                            let short_self = if self_id.len() > 8 { &self_id[..8] } else { &self_id };

                            // Colony sync
                            let eval = s.get("evaluation");
                            let syncs = eval.and_then(|e| e.get("colony_benefit")).and_then(|c| c.get("syncs_measured")).and_then(|v| v.as_u64()).unwrap_or(0);
                            let delta = eval.and_then(|e| e.get("colony_benefit")).and_then(|c| c.get("avg_sync_benefit")).and_then(|v| v.as_f64()).unwrap_or(0.0);

                            view! {
                                <div class="colony-peer">
                                    <span class="colony-peer-name">{short_self.to_string()}</span>
                                    <div class="colony-peer-bar">
                                        <div class="colony-peer-fill" style=format!("width:{}%", (self_fitness * 100.0) as u64)></div>
                                    </div>
                                    <span class="colony-peer-pct">{format!("{:.0}%", self_fitness * 100.0)}</span>
                                    <span class="colony-peer-role">"queen"</span>
                                </div>
                                {peers.iter().map(|p| {
                                    let id = p.get("instance_id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let short = if id.len() > 8 { id[..8].to_string() } else { id };
                                    let status = p.get("status").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                    let peer_fitness = p.get("fitness").and_then(|f| f.get("total")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    let pct = (peer_fitness * 100.0) as u64;
                                    view! {
                                        <div class="colony-peer">
                                            <span class="colony-peer-name">{short}</span>
                                            <div class="colony-peer-bar">
                                                <div class="colony-peer-fill" style=format!("width:{}%", pct)></div>
                                            </div>
                                            <span class="colony-peer-pct">{format!("{}%", pct)}</span>
                                            <span class="colony-peer-role">{status}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                                {(syncs > 0).then(|| view! {
                                    <div style="font-size:9px;color:var(--text-dim);margin-top:4px">
                                        {format!("sync: {} peers, \u{0394}{:+.3}", syncs, delta)}
                                    </div>
                                })}
                            }
                        }}
                    </div>
                </div>

                </div> // end cockpit-main
            </Show>

            // ═══ BOTTOM PANEL (tabbed: CHAT | LOGS) ═══
            <div class="bottom-panel">
                // Plan approval bar
                <Show when=move || pending_plan.get().is_some() fallback=|| ()>
                    <div class="plan-bar">
                        <span class="plan-bar-label">"PLAN"</span>
                        <span class="plan-bar-desc">
                            {move || pending_plan.get()
                                .and_then(|p| p.get("goal_description")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()))
                                .unwrap_or_else(|| "Unknown goal".to_string())
                            }
                        </span>
                        <div class="plan-bar-actions">
                            <button class="btn btn-approve" on:click=approve_handler>"Approve"</button>
                            <button class="btn btn-reject" on:click=reject_handler>"Reject"</button>
                        </div>
                    </div>
                </Show>

                <div class="bottom-tabs">
                    <button
                        class=move || if active_tab.get() == "chat" { "bottom-tab active" } else { "bottom-tab" }
                        on:click=move |_| set_active_tab.set("chat")
                    >"CHAT"</button>
                    <button
                        class=move || if active_tab.get() == "logs" { "bottom-tab active" } else { "bottom-tab" }
                        on:click=move |_| set_active_tab.set("logs")
                    >"LOGS"</button>
                    <button class="btn" style="margin-left:auto;border:none;font-size:9px" on:click=clear_chat>"New"</button>
                </div>

                // Chat tab
                <Show when=move || active_tab.get() == "chat" fallback=move || {
                    // Logs tab — recent thoughts as log stream
                    view! {
                        <div class="bottom-content">
                            {move || {
                                let s = soul.get().unwrap_or_default();
                                let thoughts: Vec<serde_json::Value> = s.get("recent_thoughts")
                                    .and_then(|v| v.as_array())
                                    .cloned()
                                    .unwrap_or_default();
                                if thoughts.is_empty() {
                                    return view! { <span class="empty-text">"no thoughts yet"</span> }.into_view();
                                }
                                view! {
                                    {thoughts.iter().map(|t| {
                                        let thought_type = t.get("type").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                        let content = t.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let created_at = t.get("created_at").and_then(|v| v.as_i64()).unwrap_or(0);
                                        let abbr = match thought_type.as_str() {
                                            "observation" => "obs", "reasoning" => "rsn", "decision" => "dec",
                                            "reflection" => "ref", "tool_execution" => "tool", "mutation" => "mut",
                                            "cross_hemisphere" => "cross", "escalation" => "esc", "memory_consolidation" => "mem",
                                            _ => &thought_type,
                                        };
                                        let log_class = match thought_type.as_str() {
                                            "observation" => "log-type obs", "reasoning" => "log-type reason",
                                            "decision" => "log-type decide", "reflection" => "log-type reflect",
                                            "tool_execution" => "log-type tool", "mutation" => "log-type mutate",
                                            _ => "log-type",
                                        };
                                        view! {
                                            <div class="log-line">
                                                <span class="log-ts">{format_relative_time(created_at)}</span>
                                                <span class=log_class>{abbr.to_string()}</span>
                                                {content}
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                }.into_view()
                            }}
                        </div>
                    }
                }>
                    <div class="bottom-content" node_ref=chat_ref>
                        <div class="chat-messages">
                            <For
                                each=move || {
                                    let msgs = chat_msgs.get();
                                    msgs.into_iter().enumerate().collect::<Vec<_>>()
                                }
                                key=|(i, _)| *i
                                children=move |(_, msg)| {
                                    let role_class = format!("chat-msg-role {}", msg.role);
                                    let role_label = if msg.role == "user" { "you" } else { "soul" };
                                    let tools = msg.tools.clone();
                                    let hemisphere = msg.hemisphere.clone();
                                    view! {
                                        <div class="chat-msg">
                                            <span class=role_class>{role_label}</span>
                                            {hemisphere.map(|h| view! {
                                                <span class="chat-msg-hemisphere">{h}</span>
                                            })}
                                            <span class="chat-msg-time">{format_timestamp(msg.timestamp)}</span>
                                            <div class="chat-msg-content">{msg.content.clone()}</div>
                                            {if !tools.is_empty() {
                                                Some(view! {
                                                    {tools.into_iter().map(|t| {
                                                        let cmd = t.get("command").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                                        let stdout = t.get("stdout").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                        let stderr = t.get("stderr").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                        let exit_code = t.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
                                                        let (expanded, set_expanded) = create_signal(false);
                                                        view! {
                                                            <div class="chat-tool-block">
                                                                <button class="chat-tool-header" on:click=move |_| set_expanded.update(|v| *v = !*v)>
                                                                    <span class="chat-tool-cmd">"$ "{cmd.clone()}</span>
                                                                    <span class="chat-tool-exit">{format!("exit {}", exit_code)}</span>
                                                                </button>
                                                                <Show when=move || expanded.get() fallback=|| ()>
                                                                    <pre class="chat-tool-output">
                                                                        {if !stdout.is_empty() { stdout.clone() } else if !stderr.is_empty() { stderr.clone() } else { "(no output)".to_string() }}
                                                                    </pre>
                                                                </Show>
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                })
                                            } else { None }}
                                        </div>
                                    }
                                }
                            />
                            <Show when=move || chat_loading.get() fallback=|| ()>
                                <div class="chat-msg">
                                    <span class="chat-msg-role soul">"soul"</span>
                                    <span class="chat-typing">" thinking..."</span>
                                </div>
                            </Show>
                            <Show when=move || chat_error.get().is_some() fallback=|| ()>
                                <div class="chat-error">{move || chat_error.get().unwrap_or_default()}</div>
                            </Show>
                        </div>
                    </div>
                    <div class="chat-input-bar">
                        <input
                            type="text"
                            class="chat-input"
                            placeholder="> talk to the soul..."
                            prop:value=move || chat_input.get()
                            on:input=move |ev| set_chat_input.set(event_target_value(&ev))
                            on:keydown=on_chat_key
                            disabled=move || chat_loading.get()
                        />
                        <button
                            class="btn btn-primary"
                            on:click=on_chat_click
                            disabled=move || chat_loading.get() || chat_input.get().trim().is_empty()
                        >{move || if chat_loading.get() { "..." } else { "Send" }}</button>
                    </div>
                </Show>
            </div>

            // ═══ STATUS BAR ═══
            <div class="status-bar">
                {move || {
                    let sys = system.get().unwrap_or_default();
                    let cpu = sys.get("cpu_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let mem = sys.get("mem_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let disk = sys.get("disk_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);

                    let s = soul.get().unwrap_or_default();
                    let cycles = s.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);

                    let cpu_class = if cpu > 90.0 { "bad" } else if cpu > 70.0 { "warn" } else { "good" };
                    let mem_class = if mem > 90.0 { "bad" } else if mem > 70.0 { "warn" } else { "good" };
                    let disk_class = if disk > 90.0 { "bad" } else if disk > 70.0 { "warn" } else { "good" };

                    view! {
                        <span>"CPU "<span class=cpu_class>{format!("{:.0}%", cpu)}</span></span>
                        <span>"MEM "<span class=mem_class>{format!("{:.0}%", mem)}</span></span>
                        <span>"DISK "<span class=disk_class>{format!("{:.0}%", disk)}</span></span>
                        <span class="sep">"|"</span>
                        <span>"cycles="<span class="val">{cycles.to_string()}</span></span>
                        <span style="flex:1"></span>
                        <span class="val">{concat!("tempo-x402 v", env!("CARGO_PKG_VERSION"))}</span>
                    }
                }}
            </div>
        </div>
    }
}

// ── Helper types ──

#[derive(Clone, Debug)]
struct ChatMsg {
    role: &'static str,
    content: String,
    tools: Vec<serde_json::Value>,
    timestamp: i64,
    hemisphere: Option<String>,
}

// ── Helper functions ──

fn render_cog_item(name: &str, metrics: Vec<(&str, String)>) -> impl IntoView {
    let name = name.to_string();
    view! {
        <div class="cog-item">
            <div class="cog-name">{name}</div>
            {metrics.into_iter().map(|(k, v)| {
                view! {
                    <div class="cog-metric">
                        <span class="k">{k.to_string()}</span>
                        <span class="v">{v}</span>
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}

fn format_uptime(secs: i64) -> String {
    if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d{}h", secs / 86400, (secs % 86400) / 3600)
    }
}

fn format_relative_time(unix_ts: i64) -> String {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let diff = now - unix_ts;
    if diff < 0 {
        return "now".to_string();
    }
    if diff < 60 {
        format!("{}s", diff)
    } else if diff < 3600 {
        format!("{}m", diff / 60)
    } else if diff < 86400 {
        format!("{}h", diff / 3600)
    } else {
        format!("{}d", diff / 86400)
    }
}

fn format_timestamp(unix_ts: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((unix_ts as f64) * 1000.0);
    format!("{:02}:{:02}", date.get_hours(), date.get_minutes())
}
