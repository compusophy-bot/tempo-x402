use crate::api;
use leptos::*;

#[component]
pub fn SoulPanel(status: ReadSignal<Option<serde_json::Value>>) -> impl IntoView {
    let (expanded_idx, set_expanded_idx) = create_signal(None::<usize>);
    let (nudge_input, set_nudge_input) = create_signal(String::new());
    let (nudge_sending, set_nudge_sending) = create_signal(false);
    let (nudge_result, set_nudge_result) = create_signal(None::<Result<(), String>>);
    let (model_turbo, set_model_turbo) = create_signal(false);
    let (model_switching, set_model_switching) = create_signal(false);

    // Fetch model status on mount
    spawn_local(async move {
        if let Ok(data) = api::fetch_model_status().await {
            let is_turbo = data.get("turbo").and_then(|v| v.as_bool()).unwrap_or(false);
            set_model_turbo.set(is_turbo);
        }
    });

    view! {
        <div class="soul-section">
            {move || {
                let data = match status.get() {
                    Some(d) => d,
                    None => return view! {
                        <div class="soul-card soul-card--inactive">
                            <div class="soul-header">
                                <h2>"Soul"</h2>
                                <span class="soul-status-badge soul-status--gray">"No Data"</span>
                            </div>
                            <p class="soul-muted">"Soul status unavailable"</p>
                        </div>
                    }.into_view(),
                };

                let active = data.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                let dormant = data.get("dormant").and_then(|v| v.as_bool()).unwrap_or(false);
                let total_cycles = data.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                let last_think_at = data.get("last_think_at").and_then(|v| v.as_i64());
                let thoughts: Vec<serde_json::Value> = data.get("recent_thoughts")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect();

                // Mode from status response
                let mode = data.get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or(if !active { "inactive" } else if dormant { "dormant" } else { "observe" })
                    .to_string();

                let tools_enabled = data.get("tools_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                let coding_enabled = data.get("coding_enabled").and_then(|v| v.as_bool()).unwrap_or(false);

                // Cycle health metrics
                let cycle_health = data.get("cycle_health");
                let total_code_entries = cycle_health
                    .and_then(|h| h.get("total_code_entries"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let cycles_since_commit = cycle_health
                    .and_then(|h| h.get("cycles_since_last_commit"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let completed_plans = cycle_health
                    .and_then(|h| h.get("completed_plans_count"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let failed_plans = cycle_health
                    .and_then(|h| h.get("failed_plans_count"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let goals_active = cycle_health
                    .and_then(|h| h.get("goals_active"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                // Fitness from soul status
                let soul_fitness = data.get("fitness");
                let soul_fitness_total = soul_fitness
                    .and_then(|f| f.get("total"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let soul_fitness_trend = soul_fitness
                    .and_then(|f| f.get("trend"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                let (badge_class, badge_label) = if !active {
                    ("soul-status--gray", "Inactive")
                } else if dormant {
                    ("soul-status--yellow", "Dormant")
                } else {
                    ("soul-status--green", "Active")
                };

                let mode_class = match mode.as_str() {
                    "observe" => "soul-mode--observe",
                    "chat" => "soul-mode--chat",
                    "code" => "soul-mode--code",
                    "review" => "soul-mode--review",
                    _ => "soul-mode--observe",
                };

                let last_thought_str = last_think_at
                    .map(format_relative_time)
                    .unwrap_or_else(|| "never".to_string());

                view! {
                    <div class="soul-card">
                        <div class="soul-header">
                            <h2>"Soul"</h2>
                            <div class="soul-header-badges">
                                {if active && !dormant {
                                    Some(view! {
                                        <span class={format!("soul-mode-badge {}", mode_class)}>
                                            {mode.clone()}
                                        </span>
                                    })
                                } else {
                                    None
                                }}
                                <span class={format!("soul-status-badge {}", badge_class)}>
                                    {badge_label}
                                </span>
                                // Lifecycle phase badge
                                {
                                    let lifecycle = data.get("lifecycle");
                                    let phase = lifecycle
                                        .and_then(|l| l.get("phase"))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("fork");
                                    let own_commits = lifecycle
                                        .and_then(|l| l.get("own_commits"))
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    let (phase_class, phase_label) = match phase {
                                        "birth" => ("lifecycle-badge--birth", format!("born ({own_commits} commits)")),
                                        "branch" => ("lifecycle-badge--branch", format!("branching ({own_commits} commits)")),
                                        _ => ("lifecycle-badge--fork", "fork".to_string()),
                                    };
                                    view! {
                                        <span class={format!("lifecycle-badge {}", phase_class)}>
                                            {phase_label}
                                        </span>
                                    }
                                }
                            </div>
                        </div>

                        <div class="stats-grid">
                            <div class="stat-card">
                                <span class="stat-label">"Fitness"</span>
                                <span class="stat-value">
                                    {format!("{:.0}%", soul_fitness_total * 100.0)}
                                    {if soul_fitness_trend > 0.001 {
                                        " \u{25B2}"
                                    } else if soul_fitness_trend < -0.001 {
                                        " \u{25BC}"
                                    } else {
                                        ""
                                    }}
                                </span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"Cycles"</span>
                                <span class="stat-value">{total_cycles.to_string()}</span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"Last Thought"</span>
                                <span class="stat-value">{last_thought_str}</span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"Code Entries"</span>
                                <span class="stat-value">{total_code_entries.to_string()}</span>
                            </div>
                        </div>

                        // Plan-driven health bar
                        {if active && !dormant {
                            let health_class = if cycles_since_commit > 30 {
                                "soul-streak soul-streak--danger"
                            } else if cycles_since_commit > 15 {
                                "soul-streak soul-streak--warn"
                            } else if goals_active > 0 {
                                "soul-streak soul-streak--active"
                            } else {
                                "soul-streak"
                            };
                            let health_label = if cycles_since_commit > 30 {
                                format!("stagnant ({} cycles, no commit)", cycles_since_commit)
                            } else if goals_active > 0 {
                                format!("{} goals active, {} cycles since commit", goals_active, cycles_since_commit)
                            } else {
                                format!("mode: {}", mode)
                            };
                            Some(view! {
                                <div class={health_class}>
                                    {health_label}
                                </div>
                            })
                        } else {
                            None
                        }}

                        // Plan outcome summary
                        {if completed_plans > 0 || failed_plans > 0 {
                            let total = completed_plans + failed_plans;
                            let rate = if total > 0 { completed_plans * 100 / total } else { 0 };
                            let class = if rate >= 50 {
                                "soul-plans-summary soul-plans-summary--ok"
                            } else if completed_plans > 0 {
                                "soul-plans-summary soul-plans-summary--warn"
                            } else {
                                "soul-plans-summary soul-plans-summary--danger"
                            };
                            Some(view! {
                                <div class={class}>
                                    {format!("{} completed / {} failed ({}% success)", completed_plans, failed_plans, rate)}
                                </div>
                            })
                        } else {
                            None
                        }}

                        // Feature flags
                        {if active {
                            Some(view! {
                                <div class="soul-flags">
                                    <span class={if tools_enabled { "soul-flag soul-flag--on" } else { "soul-flag" }}>
                                        {if tools_enabled { "tools: on" } else { "tools: off" }}
                                    </span>
                                    <span class={if coding_enabled { "soul-flag soul-flag--on" } else { "soul-flag" }}>
                                        {if coding_enabled { "coding: on" } else { "coding: off" }}
                                    </span>
                                </div>
                            })
                        } else {
                            None
                        }}

                        // Active goals
                        {
                            let goals = data.get("goals")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            if !goals.is_empty() {
                                Some(view! {
                                    <div class="soul-goals">
                                        <h3>{format!("Active Goals ({})", goals.len())}</h3>
                                        {goals.iter().map(|g| {
                                            let desc = g.get("description")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("?")
                                                .to_string();
                                            let status = g.get("status")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            let priority = g.get("priority")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(0);
                                            let retry_count = g.get("retry_count")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(0);
                                            let status_class = match status.as_str() {
                                                "active" => "goal-status--active",
                                                "completed" => "goal-status--completed",
                                                "abandoned" => "goal-status--abandoned",
                                                _ => "goal-status--unknown",
                                            };
                                            let truncated = if desc.len() > 100 {
                                                let mut end = 100;
                                                while end > 0 && !desc.is_char_boundary(end) {
                                                    end -= 1;
                                                }
                                                format!("{}...", &desc[..end])
                                            } else {
                                                desc
                                            };
                                            view! {
                                                <div class="soul-goal">
                                                    <span class={format!("goal-status-badge {}", status_class)}>
                                                        {status.clone()}
                                                    </span>
                                                    <span class="goal-priority">
                                                        {"P".to_string() + &priority.to_string()}
                                                    </span>
                                                    <span class="goal-desc">{truncated}</span>
                                                    {if retry_count > 0 {
                                                        Some(view! {
                                                            <span class="goal-retries">
                                                                {format!("({} retries)", retry_count)}
                                                            </span>
                                                        })
                                                    } else {
                                                        None
                                                    }}
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                }.into_view())
                            } else {
                                None
                            }
                        }

                        // Active plan progress
                        {
                            let plan = data.get("active_plan");
                            if let Some(p) = plan {
                                let plan_status = p.get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let current_step = p.get("current_step")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let total_steps = p.get("total_steps")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let replan_count = p.get("replan_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let plan_class = match plan_status.as_str() {
                                    "executing" => "plan-status--executing",
                                    "pending_approval" => "plan-status--pending",
                                    "completed" => "plan-status--completed",
                                    "failed" => "plan-status--failed",
                                    _ => "",
                                };
                                let progress_pct = if total_steps > 0 {
                                    (current_step as f64 / total_steps as f64 * 100.0) as u64
                                } else {
                                    0
                                };
                                Some(view! {
                                    <div class="soul-plan">
                                        <h3>"Active Plan"</h3>
                                        <div class="plan-info">
                                            <span class={format!("plan-status-badge {}", plan_class)}>
                                                {plan_status}
                                            </span>
                                            <span class="plan-progress-text">
                                                {format!("Step {}/{}", current_step, total_steps)}
                                            </span>
                                            {if replan_count > 0 {
                                                Some(view! {
                                                    <span class="plan-replan">
                                                        {format!("(replan #{})", replan_count)}
                                                    </span>
                                                })
                                            } else {
                                                None
                                            }}
                                        </div>
                                        <div class="plan-progress-bar">
                                            <div class="plan-progress-fill"
                                                style=format!("width: {}%", progress_pct)>
                                            </div>
                                        </div>
                                    </div>
                                }.into_view())
                            } else if active && !dormant {
                                Some(view! {
                                    <div class="soul-plan">
                                        <h3>"Active Plan"</h3>
                                        <p class="soul-muted">"No active plan — waiting for next cycle"</p>
                                    </div>
                                }.into_view())
                            } else {
                                None
                            }
                        }

                        // Nudge form
                        {if active && !dormant {
                            let send_nudge = move |_: web_sys::MouseEvent| {
                                let msg = nudge_input.get().trim().to_string();
                                if msg.is_empty() || nudge_sending.get() {
                                    return;
                                }
                                set_nudge_sending.set(true);
                                set_nudge_result.set(None);
                                spawn_local(async move {
                                    match api::send_nudge(&msg, 5).await {
                                        Ok(()) => {
                                            set_nudge_input.set(String::new());
                                            set_nudge_result.set(Some(Ok(())));
                                        }
                                        Err(e) => {
                                            set_nudge_result.set(Some(Err(e)));
                                        }
                                    }
                                    set_nudge_sending.set(false);
                                });
                            };
                            Some(view! {
                                <div class="soul-nudge-form">
                                    <h3>"Nudge"</h3>
                                    <div class="soul-nudge-row">
                                        <input
                                            type="text"
                                            class="soul-nudge-input"
                                            placeholder="Send a message to the soul..."
                                            prop:value=move || nudge_input.get()
                                            on:input=move |ev| set_nudge_input.set(event_target_value(&ev))
                                            disabled=move || nudge_sending.get()
                                        />
                                        <button
                                            class="btn btn-primary btn-sm"
                                            on:click=send_nudge
                                            disabled=move || nudge_sending.get() || nudge_input.get().trim().is_empty()
                                        >
                                            {move || if nudge_sending.get() { "Sending..." } else { "Send" }}
                                        </button>
                                    </div>
                                    {move || match nudge_result.get() {
                                        Some(Ok(())) => Some(view! {
                                            <p class="soul-nudge-ok">"Nudge sent!"</p>
                                        }.into_view()),
                                        Some(Err(e)) => Some(view! {
                                            <p class="soul-nudge-err">{e}</p>
                                        }.into_view()),
                                        None => None,
                                    }}
                                </div>
                            })
                        } else {
                            None
                        }}

                        // Model toggle (turbo boost)
                        {if active && !dormant {
                            let toggle_model = move |_: web_sys::MouseEvent| {
                                if model_switching.get() { return; }
                                set_model_switching.set(true);
                                let is_turbo = model_turbo.get();
                                spawn_local(async move {
                                    let new_model = if is_turbo { None } else { Some("gemini-3.1-pro-preview") };
                                    if api::set_model(new_model).await.is_ok() {
                                        set_model_turbo.set(!is_turbo);
                                    }
                                    set_model_switching.set(false);
                                });
                            };
                            Some(view! {
                                <div class="soul-model-toggle">
                                    <button
                                        class=move || if model_turbo.get() { "btn btn-sm btn-turbo-active" } else { "btn btn-sm btn-turbo" }
                                        on:click=toggle_model
                                        disabled=move || model_switching.get()
                                    >
                                        {move || if model_switching.get() {
                                            "Switching...".to_string()
                                        } else if model_turbo.get() {
                                            "Pro (turbo)".to_string()
                                        } else {
                                            "Flash Lite".to_string()
                                        }}
                                    </button>
                                </div>
                            })
                        } else {
                            None
                        }}

                        // Plan Transformer (284K param model)
                        {
                            let tfm = data.get("transformer");
                            if let Some(t) = tfm {
                                let param_count = t.get("param_count")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let train_steps = t.get("train_steps")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let running_loss = t.get("running_loss")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                let last_train_loss = t.get("last_train_loss")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                let vocab_size = t.get("vocab_size")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let templates_trained = t.get("templates_trained_on")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let plans_generated = t.get("plans_generated")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let status_label = if train_steps >= 50 {
                                    "generating"
                                } else if train_steps > 0 {
                                    "learning"
                                } else {
                                    "untrained"
                                };
                                let status_class = if train_steps >= 50 {
                                    "transformer-status--active"
                                } else if train_steps > 0 {
                                    "transformer-status--learning"
                                } else {
                                    "transformer-status--idle"
                                };
                                Some(view! {
                                    <div class="transformer-panel">
                                        <h3>
                                            "Plan Transformer "
                                            <span class={format!("transformer-status {}", status_class)}>
                                                {status_label}
                                            </span>
                                        </h3>
                                        <div class="transformer-stats">
                                            <div class="transformer-stat">
                                                <span class="transformer-stat-label">"params"</span>
                                                <span class="transformer-stat-value">
                                                    {format!("{}K", param_count / 1000)}
                                                </span>
                                            </div>
                                            <div class="transformer-stat">
                                                <span class="transformer-stat-label">"train steps"</span>
                                                <span class="transformer-stat-value">
                                                    {train_steps.to_string()}
                                                </span>
                                            </div>
                                            <div class="transformer-stat">
                                                <span class="transformer-stat-label">"loss"</span>
                                                <span class="transformer-stat-value">
                                                    {format!("{:.3}", last_train_loss)}
                                                </span>
                                            </div>
                                            <div class="transformer-stat">
                                                <span class="transformer-stat-label">"vocab"</span>
                                                <span class="transformer-stat-value">
                                                    {vocab_size.to_string()}
                                                </span>
                                            </div>
                                            <div class="transformer-stat">
                                                <span class="transformer-stat-label">"trained on"</span>
                                                <span class="transformer-stat-value">
                                                    {format!("{} templates", templates_trained)}
                                                </span>
                                            </div>
                                            <div class="transformer-stat">
                                                <span class="transformer-stat-label">"generated"</span>
                                                <span class="transformer-stat-value">
                                                    {format!("{} plans", plans_generated)}
                                                </span>
                                            </div>
                                        </div>
                                        {if running_loss > 0.0 {
                                            Some(view! {
                                                <div class="transformer-loss-bar">
                                                    <div class="transformer-loss-fill"
                                                        style=format!("width: {}%", (100.0 - (running_loss * 25.0).min(100.0)).max(5.0))>
                                                    </div>
                                                    <span class="transformer-loss-label">
                                                        {format!("running loss: {:.3}", running_loss)}
                                                    </span>
                                                </div>
                                            })
                                        } else {
                                            None
                                        }}
                                    </div>
                                }.into_view())
                            } else {
                                None
                            }
                        }

                        // Capability profile
                        {
                            let cap_profile = data.get("capability_profile");
                            let capabilities = cap_profile
                                .and_then(|p| p.get("capabilities"))
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            let measured: Vec<&serde_json::Value> = capabilities
                                .iter()
                                .filter(|c| c.get("attempts").and_then(|v| v.as_u64()).unwrap_or(0) > 0)
                                .collect();
                            if !measured.is_empty() {
                                let overall = cap_profile
                                    .and_then(|p| p.get("overall_success_rate"))
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.5);
                                let strongest = cap_profile
                                    .and_then(|p| p.get("strongest"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("-")
                                    .to_string();
                                let weakest_cap = cap_profile
                                    .and_then(|p| p.get("weakest"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("-")
                                    .to_string();
                                Some(view! {
                                    <div class="capability-panel">
                                        <h3>"Capabilities"</h3>
                                        <div class="capability-overview">
                                            <span class="capability-overall">
                                                {format!("{:.0}% overall", overall * 100.0)}
                                            </span>
                                            <span class="capability-best">
                                                {format!("best: {}", strongest)}
                                            </span>
                                            <span class="capability-worst">
                                                {format!("worst: {}", weakest_cap)}
                                            </span>
                                        </div>
                                        <div class="capability-bars">
                                            {measured.iter().map(|c| {
                                                let name = c.get("display_name")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("?")
                                                    .to_string();
                                                let rate = c.get("success_rate")
                                                    .and_then(|v| v.as_f64())
                                                    .unwrap_or(0.0);
                                                let attempts = c.get("attempts")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0);
                                                let pct = (rate * 100.0) as u64;
                                                let bar_class = if rate >= 0.8 {
                                                    "capability-bar-fill capability-bar-fill--good"
                                                } else if rate >= 0.5 {
                                                    "capability-bar-fill capability-bar-fill--ok"
                                                } else {
                                                    "capability-bar-fill capability-bar-fill--bad"
                                                };
                                                view! {
                                                    <div class="capability-bar-row">
                                                        <span class="capability-bar-label">{name}</span>
                                                        <div class="capability-bar-track">
                                                            <div class={bar_class}
                                                                style=format!("width: {}%", pct)>
                                                            </div>
                                                        </div>
                                                        <span class="capability-bar-value">
                                                            {format!("{}% ({})", pct, attempts)}
                                                        </span>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                }.into_view())
                            } else {
                                None
                            }
                        }

                        // Plan outcomes (feedback loop)
                        {
                            let outcomes = data.get("plan_outcomes")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            if !outcomes.is_empty() {
                                Some(view! {
                                    <div class="outcomes-panel">
                                        <h3>{format!("Recent Outcomes ({})", outcomes.len())}</h3>
                                        <div class="outcomes-list">
                                            {outcomes.iter().take(5).map(|o| {
                                                let status = o.get("status")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("unknown")
                                                    .to_string();
                                                let lesson = o.get("lesson")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let steps_done = o.get("steps_completed")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0);
                                                let total_steps_o = o.get("total_steps")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0);
                                                let error_cat = o.get("error_category")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let status_icon = if status == "completed" {
                                                    "\u{2705}"
                                                } else {
                                                    "\u{274C}"
                                                };
                                                let truncated_lesson = if lesson.len() > 120 {
                                                    let mut end = 120;
                                                    while end > 0 && !lesson.is_char_boundary(end) {
                                                        end -= 1;
                                                    }
                                                    format!("{}...", &lesson[..end])
                                                } else {
                                                    lesson
                                                };
                                                view! {
                                                    <div class={format!("outcome-item outcome-item--{}", status)}>
                                                        <span class="outcome-icon">{status_icon}</span>
                                                        <div class="outcome-info">
                                                            <div class="outcome-lesson">{truncated_lesson}</div>
                                                            <div class="outcome-meta">
                                                                {format!("{}/{} steps", steps_done, total_steps_o)}
                                                                {if !error_cat.is_empty() {
                                                                    format!(" | {}", error_cat)
                                                                } else {
                                                                    String::new()
                                                                }}
                                                            </div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                }.into_view())
                            } else {
                                None
                            }
                        }

                        // Opus IQ Benchmark
                        {
                            let bench = data.get("benchmark");
                            if let Some(b) = bench {
                                let pass_at_1 = b.get("pass_at_1")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                let elo_display = b.get("elo_display")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("ELO 1000")
                                    .to_string();
                                let attempted = b.get("problems_attempted")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let passed_b = b.get("problems_passed")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let refs = b.get("reference_scores")
                                    .and_then(|v| v.as_array())
                                    .cloned()
                                    .unwrap_or_default();
                                // Score history for trend
                                let history = b.get("history")
                                    .and_then(|v| v.as_array())
                                    .cloned()
                                    .unwrap_or_default();
                                let trend_str = if history.len() >= 2 {
                                    let prev = history[history.len() - 2].get("pass_at_1")
                                        .and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    let diff = pass_at_1 - prev;
                                    if diff > 0.5 {
                                        format!("\u{25B2} +{:.1}%", diff)
                                    } else if diff < -0.5 {
                                        format!("\u{25BC} {:.1}%", diff)
                                    } else {
                                        "\u{25C6} stable".to_string()
                                    }
                                } else {
                                    "first run".to_string()
                                };
                                // Collective intelligence
                                let collective = b.get("collective");
                                let collective_pass = collective
                                    .and_then(|c| c.get("pass_at_1"))
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                let collective_solved = collective
                                    .and_then(|c| c.get("unique_solved"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let collective_total = collective
                                    .and_then(|c| c.get("total_problems"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(164);
                                Some(view! {
                                    <div class="benchmark-panel">
                                        <h3>"Opus IQ Benchmark"</h3>
                                        <div class="benchmark-score">
                                            <span class="benchmark-pass-at-1">
                                                {format!("{:.1}%", pass_at_1)}
                                            </span>
                                            <span class="benchmark-label">"pass@1"</span>
                                        </div>
                                        <div class="benchmark-meta">
                                            <span class="benchmark-elo">{elo_display}</span>
                                            <span class="benchmark-trend">{trend_str}</span>
                                        </div>
                                        <div class="benchmark-stats">
                                            {format!("{}/{} problems solved (this agent)", passed_b, attempted)}
                                        </div>
                                        // Collective intelligence
                                        {if collective_solved > 0 || collective_total > 0 {
                                            Some(view! {
                                                <div class="benchmark-collective">
                                                    <div class="benchmark-collective-label">"Collective Intelligence (swarm)"</div>
                                                    <div class="benchmark-collective-score">
                                                        <span class="benchmark-collective-pass">
                                                            {format!("{:.1}%", collective_pass)}
                                                        </span>
                                                        <span class="benchmark-collective-detail">
                                                            {format!("{}/{} unique problems solved across all agents", collective_solved, collective_total)}
                                                        </span>
                                                    </div>
                                                </div>
                                            })
                                        } else {
                                            None
                                        }}
                                        // Score history
                                        {if history.len() >= 2 {
                                            Some(view! {
                                                <div class="benchmark-history">
                                                    <div class="benchmark-history-label">"Score History"</div>
                                                    <div class="benchmark-history-bars">
                                                        {history.iter().map(|h| {
                                                            let score = h.get("pass_at_1")
                                                                .and_then(|v| v.as_f64())
                                                                .unwrap_or(0.0);
                                                            let bar_class = if score >= 80.0 {
                                                                "benchmark-history-bar benchmark-history-bar--high"
                                                            } else if score >= 50.0 {
                                                                "benchmark-history-bar benchmark-history-bar--mid"
                                                            } else {
                                                                "benchmark-history-bar benchmark-history-bar--low"
                                                            };
                                                            view! {
                                                                <div class={bar_class}
                                                                    style=format!("height: {}%", score.max(2.0))
                                                                    title=format!("{:.1}%", score)>
                                                                </div>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </div>
                                                </div>
                                            })
                                        } else {
                                            None
                                        }}
                                        // Reference models
                                        <div class="benchmark-refs">
                                            <div class="benchmark-refs-label">"vs. published baselines"</div>
                                            {refs.iter().map(|r| {
                                                let model = r.get("model")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("?")
                                                    .to_string();
                                                let ref_score = r.get("pass_at_1")
                                                    .and_then(|v| v.as_f64())
                                                    .unwrap_or(0.0);
                                                let comparison_class = if pass_at_1 > ref_score + 1.0 {
                                                    "benchmark-ref--above"
                                                } else if pass_at_1 < ref_score - 1.0 {
                                                    "benchmark-ref--below"
                                                } else {
                                                    "benchmark-ref--equal"
                                                };
                                                view! {
                                                    <div class={format!("benchmark-ref {}", comparison_class)}>
                                                        <span class="benchmark-ref-model">{model}</span>
                                                        <span class="benchmark-ref-score">
                                                            {format!("{:.1}%", ref_score)}
                                                        </span>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                }.into_view())
                            } else {
                                Some(view! {
                                    <div class="benchmark-panel benchmark-panel--waiting">
                                        <h3>"Opus IQ Benchmark"</h3>
                                        <p class="soul-muted">"Waiting for first benchmark run (triggers every 50 cycles)"</p>
                                        <p class="soul-muted">{format!("Current cycle: {}", total_cycles)}</p>
                                    </div>
                                }.into_view())
                            }
                        }

                        // Neural brain panel
                        {
                            if let Some(brain) = data.get("brain").and_then(|v| v.as_object()) {
                                let params = brain.get("parameters")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let train_steps = brain.get("train_steps")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let loss = brain.get("running_loss")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                Some(view! {
                                    <div class="brain-panel">
                                        <h3>"Neural Brain"</h3>
                                        <div class="brain-stats">
                                            <div class="brain-stat">
                                                <span class="brain-stat-value">{format!("{}K", params / 1000)}</span>
                                                <span class="brain-stat-label">"parameters"</span>
                                            </div>
                                            <div class="brain-stat">
                                                <span class="brain-stat-value">{format!("{}", train_steps)}</span>
                                                <span class="brain-stat-label">"training steps"</span>
                                            </div>
                                            <div class="brain-stat">
                                                <span class="brain-stat-value">{format!("{:.4}", loss)}</span>
                                                <span class="brain-stat-label">"loss"</span>
                                            </div>
                                        </div>
                                    </div>
                                }.into_view())
                            } else {
                                None
                            }
                        }

                        {if thoughts.is_empty() && !active {
                            view! {
                                <p class="soul-muted">"Soul not active"</p>
                            }.into_view()
                        } else if thoughts.is_empty() {
                            view! {
                                <p class="soul-muted">"No thoughts recorded yet"</p>
                            }.into_view()
                        } else {
                            let expanded = expanded_idx.get();
                            view! {
                                <div class="soul-thoughts">
                                    <h3>"Recent Thoughts"</h3>
                                    {thoughts.iter().enumerate().map(|(idx, t)| {
                                        let thought_type = t.get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown")
                                            .to_string();
                                        let content = t.get("content")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let created_at = t.get("created_at")
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0);

                                        let badge_abbr = match thought_type.as_str() {
                                            "observation" => "obs",
                                            "reasoning" => "reason",
                                            "decision" => "decide",
                                            "reflection" => "reflect",
                                            "mutation" => "mutate",
                                            "tool_execution" => "tool",
                                            "cross_hemisphere" => "cross",
                                            "escalation" => "escalate",
                                            "memory_consolidation" => "memory",
                                            _ => &thought_type,
                                        };

                                        let is_expanded = expanded == Some(idx);
                                        let display_content = if is_expanded || content.len() <= 120 {
                                            content.clone()
                                        } else {
                                            let mut end = 120;
                                            while end > 0 && !content.is_char_boundary(end) {
                                                end -= 1;
                                            }
                                            format!("{}...", &content[..end])
                                        };
                                        let is_truncatable = content.len() > 120;
                                        let content_class = if is_expanded {
                                            "thought-content thought-content--expanded"
                                        } else {
                                            "thought-content"
                                        };

                                        view! {
                                            <div
                                                class="soul-thought"
                                                on:click=move |_| {
                                                    if is_truncatable {
                                                        set_expanded_idx.set(
                                                            if expanded == Some(idx) { None } else { Some(idx) }
                                                        );
                                                    }
                                                }
                                            >
                                                <span class={format!("thought-badge thought-badge--{}", thought_type)}>
                                                    {badge_abbr.to_string()}
                                                </span>
                                                <div class=content_class>
                                                    {display_content}
                                                    {if is_truncatable && !is_expanded {
                                                        Some(view! { <div class="thought-expand-hint">"click to expand"</div> })
                                                    } else {
                                                        None
                                                    }}
                                                </div>
                                                <span class="thought-time">{format_relative_time(created_at)}</span>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            }.into_view()
                        }}
                    </div>
                }.into_view()
            }}
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
