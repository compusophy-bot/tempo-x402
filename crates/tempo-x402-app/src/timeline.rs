//! Timeline visualization: SVG-based agent life history.
//!
//! Three stacked regions:
//! 1. Free energy F(t) area chart with regime bands
//! 2. ELO + Fitness dual-axis line chart
//! 3. Event strip with categorized markers
//! Plus playback controls and scrub bar.

use leptos::*;

use crate::api;

// ── Constants ────────────────────────────────────────────────────────

const SVG_WIDTH: f64 = 1200.0;
const REGION_HEIGHT: f64 = 200.0;
const EVENT_STRIP_HEIGHT: f64 = 100.0;
const MARGIN_LEFT: f64 = 60.0;
const MARGIN_RIGHT: f64 = 20.0;
const MARGIN_TOP: f64 = 30.0;
const MARGIN_BOTTOM: f64 = 25.0;
const PLOT_WIDTH: f64 = SVG_WIDTH - MARGIN_LEFT - MARGIN_RIGHT;
const TOTAL_HEIGHT: f64 =
    REGION_HEIGHT * 2.0 + EVENT_STRIP_HEIGHT + MARGIN_TOP * 3.0 + MARGIN_BOTTOM + 40.0;

// Colors matching the dark theme
const COL_PRIMARY: &str = "#6366f1";
const COL_ACCENT: &str = "#06b6d4";
const COL_SUCCESS: &str = "#22c55e";
const COL_ERROR: &str = "#ef4444";
const COL_WARNING: &str = "#eab308";
const COL_GRID: &str = "#1e293b";
const COL_TEXT: &str = "#94a3b8";
const COL_TEXT_DIM: &str = "#475569";
const COL_BG: &str = "#0f172a";

// Regime band colors (semi-transparent)
const COL_REGIME_EXPLOIT: &str = "rgba(34,197,94,0.08)";
const COL_REGIME_LEARN: &str = "rgba(234,179,8,0.06)";
const COL_REGIME_EXPLORE: &str = "rgba(239,68,68,0.08)";
const COL_REGIME_ANOMALY: &str = "rgba(249,115,22,0.12)";

// ── Data extraction helpers ──────────────────────────────────────────

fn extract_fe_points(data: &serde_json::Value) -> Vec<(f64, f64, String)> {
    // (timestamp, total, regime)
    data.get("free_energy")
        .and_then(|fe| fe.get("measurements"))
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let t = m.get("timestamp")?.as_f64()?;
                    let total = m.get("total")?.as_f64()?;
                    let regime = m
                        .get("regime")
                        .and_then(|r| r.as_str())
                        .unwrap_or("Learn")
                        .to_string();
                    Some((t, total, regime))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_elo_points(data: &serde_json::Value) -> Vec<(f64, f64)> {
    data.get("elo")
        .and_then(|e| e.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let t = e.get("measured_at")?.as_f64()?;
                    let r = e.get("rating")?.as_f64()?;
                    Some((t, r))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_fitness_points(data: &serde_json::Value) -> Vec<(f64, f64)> {
    data.get("fitness")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    let t = f.get("measured_at")?.as_f64()?;
                    let total = f.get("total")?.as_f64()?;
                    Some((t, total))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone)]
struct EventPoint {
    t: f64,
    level: String,
    code: String,
    message: String,
}

fn extract_events(data: &serde_json::Value) -> Vec<EventPoint> {
    data.get("events")
        .and_then(|e| e.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(EventPoint {
                        t: e.get("created_at")?.as_f64()?,
                        level: e.get("level")?.as_str()?.to_string(),
                        code: e.get("code")?.as_str()?.to_string(),
                        message: e.get("message")?.as_str().unwrap_or("").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn time_range(data: &serde_json::Value) -> (f64, f64) {
    let start = data
        .get("time_range")
        .and_then(|tr| tr.get("start"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let end = data
        .get("time_range")
        .and_then(|tr| tr.get("end"))
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    if (end - start).abs() < 1.0 {
        (start, start + 1.0)
    } else {
        (start, end)
    }
}

/// Map a timestamp to x-coordinate within the plot area.
fn tx(t: f64, t_start: f64, t_end: f64) -> f64 {
    MARGIN_LEFT + (t - t_start) / (t_end - t_start) * PLOT_WIDTH
}

/// Map a value [0,1] to y-coordinate within a region (top=1, bottom=0).
fn vy(v: f64, region_top: f64, region_height: f64) -> f64 {
    region_top + region_height * (1.0 - v.clamp(0.0, 1.0))
}

/// Format timestamp as short time string.
fn format_time(ts: f64) -> String {
    let secs = ts as i64;
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    format!("{:02}:{:02}", hours, mins)
}

/// Build an SVG polyline points string from a list of (x,y) coordinates.
fn polyline_points(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{:.1},{:.1}", x, y))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build an SVG polygon points string for area fill (closed to bottom).
fn area_points(points: &[(f64, f64)], baseline_y: f64) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut pts: Vec<String> = points
        .iter()
        .map(|(x, y)| format!("{:.1},{:.1}", x, y))
        .collect();
    // Close the polygon along the baseline
    if let Some(last) = points.last() {
        pts.push(format!("{:.1},{:.1}", last.0, baseline_y));
    }
    if let Some(first) = points.first() {
        pts.push(format!("{:.1},{:.1}", first.0, baseline_y));
    }
    pts.join(" ")
}

// ── SVG Components ───────────────────────────────────────────────────

/// Grid lines for a region.
#[component]
fn GridLines(
    region_top: f64,
    region_height: f64,
    y_count: usize,
    t_start: f64,
    t_end: f64,
) -> impl IntoView {
    let h_lines: Vec<_> = (0..=y_count)
        .map(|i| {
            let y = region_top + region_height * i as f64 / y_count as f64;
            view! {
                <line x1={MARGIN_LEFT.to_string()} y1={y.to_string()}
                      x2={(SVG_WIDTH - MARGIN_RIGHT).to_string()} y2={y.to_string()}
                      stroke=COL_GRID stroke-width="1" />
            }
        })
        .collect();

    // Time grid: ~6 vertical lines
    let duration = t_end - t_start;
    let v_count = 6usize;
    let v_lines: Vec<_> = (0..=v_count)
        .map(|i| {
            let t = t_start + duration * i as f64 / v_count as f64;
            let x = tx(t, t_start, t_end);
            let label = format_time(t);
            view! {
                <line x1={x.to_string()} y1={region_top.to_string()}
                      x2={x.to_string()} y2={(region_top + region_height).to_string()}
                      stroke=COL_GRID stroke-width="1" stroke-dasharray="4,4" />
                <text x={x.to_string()} y={(region_top + region_height + 14.0).to_string()}
                      fill=COL_TEXT_DIM font-size="10" text-anchor="middle">{label}</text>
            }
        })
        .collect();

    view! {
        {h_lines}
        {v_lines}
    }
}

/// Y-axis labels for a region.
#[component]
fn YAxisLabels(
    region_top: f64,
    region_height: f64,
    labels: Vec<(f64, String)>, // (normalized_value, label)
) -> impl IntoView {
    let items: Vec<_> = labels
        .into_iter()
        .map(|(v, label)| {
            let y = vy(v, region_top, region_height);
            view! {
                <text x={(MARGIN_LEFT - 8.0).to_string()} y={(y + 3.0).to_string()}
                      fill=COL_TEXT font-size="10" text-anchor="end">{label}</text>
            }
        })
        .collect();
    view! { {items} }
}

/// Free energy region with regime bands and area chart.
#[component]
fn FreeEnergyRegion(points: Vec<(f64, f64, String)>, t_start: f64, t_end: f64) -> impl IntoView {
    let region_top = MARGIN_TOP;
    let rh = REGION_HEIGHT;

    // Build regime bands
    let regime_bands: Vec<_> = if points.len() >= 2 {
        points
            .windows(2)
            .map(|w| {
                let x1 = tx(w[0].0, t_start, t_end);
                let x2 = tx(w[1].0, t_start, t_end);
                let color = match w[0].2.as_str() {
                    "Exploit" | "EXPLOIT" => COL_REGIME_EXPLOIT,
                    "Learn" | "LEARN" => COL_REGIME_LEARN,
                    "Explore" | "EXPLORE" => COL_REGIME_EXPLORE,
                    "Anomaly" | "ANOMALY" => COL_REGIME_ANOMALY,
                    _ => COL_REGIME_LEARN,
                };
                let width = (x2 - x1).max(1.0);
                view! {
                    <rect x={x1.to_string()} y={region_top.to_string()}
                          width={width.to_string()} height={rh.to_string()}
                          fill=color />
                }
            })
            .collect()
    } else {
        vec![]
    };

    // Normalize F(t) — typically 0 to ~1.5, we map 0-1.5 to chart height
    let max_fe = points
        .iter()
        .map(|(_, v, _)| *v)
        .fold(0.5_f64, f64::max)
        .max(0.5);
    let xy_points: Vec<(f64, f64)> = points
        .iter()
        .map(|(t, v, _)| (tx(*t, t_start, t_end), vy(*v / max_fe, region_top, rh)))
        .collect();

    let line_str = polyline_points(&xy_points);
    let area_str = area_points(&xy_points, region_top + rh);

    let y_labels = vec![
        (0.0, "0".to_string()),
        (0.5, format!("{:.1}", max_fe * 0.5)),
        (1.0, format!("{:.1}", max_fe)),
    ];

    view! {
        // Region background
        <rect x={MARGIN_LEFT.to_string()} y={region_top.to_string()}
              width={PLOT_WIDTH.to_string()} height={rh.to_string()}
              fill=COL_BG rx="4" />

        // Title
        <text x={(MARGIN_LEFT + 8.0).to_string()} y={(region_top + 16.0).to_string()}
              fill=COL_TEXT font-size="11" font-weight="600">"Free Energy F(t)"</text>

        <GridLines region_top=region_top region_height=rh y_count=4 t_start=t_start t_end=t_end />
        <YAxisLabels region_top=region_top region_height=rh labels=y_labels />

        {regime_bands}

        // Area fill
        <polygon points=area_str fill=COL_PRIMARY fill-opacity="0.15" />
        // Line
        <polyline points=line_str fill="none" stroke=COL_PRIMARY stroke-width="2" />

        // Data point dots
        {xy_points.iter().map(|(x, y)| view! {
            <circle cx={x.to_string()} cy={y.to_string()} r="2.5"
                    fill=COL_PRIMARY stroke=COL_BG stroke-width="1" />
        }).collect::<Vec<_>>()}
    }
}

/// ELO + Fitness dual-axis region.
#[component]
fn EloFitnessRegion(
    elo_points: Vec<(f64, f64)>,
    fitness_points: Vec<(f64, f64)>,
    t_start: f64,
    t_end: f64,
) -> impl IntoView {
    let region_top = MARGIN_TOP * 2.0 + REGION_HEIGHT + MARGIN_BOTTOM;
    let rh = REGION_HEIGHT;

    // ELO range: 800-1600
    let elo_min = 800.0_f64;
    let elo_max = 1600.0_f64;
    let elo_xy: Vec<(f64, f64)> = elo_points
        .iter()
        .map(|(t, v)| {
            let norm = (v - elo_min) / (elo_max - elo_min);
            (tx(*t, t_start, t_end), vy(norm, region_top, rh))
        })
        .collect();

    // Fitness: 0-1.0
    let fitness_xy: Vec<(f64, f64)> = fitness_points
        .iter()
        .map(|(t, v)| (tx(*t, t_start, t_end), vy(*v, region_top, rh)))
        .collect();

    let elo_line = polyline_points(&elo_xy);
    let fitness_line = polyline_points(&fitness_xy);

    // Reference ELO lines
    let reference_elos = vec![
        (1280.0, "GPT-4o"),
        (1380.0, "Claude Opus 4"),
        (1340.0, "Gemini 3 Pro"),
    ];
    let ref_lines: Vec<_> = reference_elos
        .iter()
        .map(|(elo, name)| {
            let norm = (elo - elo_min) / (elo_max - elo_min);
            let y = vy(norm, region_top, rh);
            view! {
                <line x1={MARGIN_LEFT.to_string()} y1={y.to_string()}
                      x2={(SVG_WIDTH - MARGIN_RIGHT).to_string()} y2={y.to_string()}
                      stroke=COL_TEXT_DIM stroke-width="1" stroke-dasharray="6,4" opacity="0.4" />
                <text x={(SVG_WIDTH - MARGIN_RIGHT - 4.0).to_string()} y={(y - 3.0).to_string()}
                      fill=COL_TEXT_DIM font-size="9" text-anchor="end">{*name}</text>
            }
        })
        .collect();

    let y_labels_left = vec![
        (0.0, "800".to_string()),
        (0.5, "1200".to_string()),
        (1.0, "1600".to_string()),
    ];

    view! {
        <rect x={MARGIN_LEFT.to_string()} y={region_top.to_string()}
              width={PLOT_WIDTH.to_string()} height={rh.to_string()}
              fill=COL_BG rx="4" />

        <text x={(MARGIN_LEFT + 8.0).to_string()} y={(region_top + 16.0).to_string()}
              fill=COL_ACCENT font-size="11" font-weight="600">"ELO"</text>
        <text x={(MARGIN_LEFT + 50.0).to_string()} y={(region_top + 16.0).to_string()}
              fill=COL_TEXT_DIM font-size="11">" + "</text>
        <text x={(MARGIN_LEFT + 64.0).to_string()} y={(region_top + 16.0).to_string()}
              fill=COL_SUCCESS font-size="11" font-weight="600">"Fitness"</text>

        <GridLines region_top=region_top region_height=rh y_count=4 t_start=t_start t_end=t_end />
        <YAxisLabels region_top=region_top region_height=rh labels=y_labels_left />

        {ref_lines}

        // ELO line
        <polyline points=elo_line fill="none" stroke=COL_ACCENT stroke-width="2" />
        {elo_xy.iter().map(|(x, y)| view! {
            <circle cx={x.to_string()} cy={y.to_string()} r="3"
                    fill=COL_ACCENT stroke=COL_BG stroke-width="1" />
        }).collect::<Vec<_>>()}

        // Fitness line
        <polyline points=fitness_line fill="none" stroke=COL_SUCCESS stroke-width="2" />
        {fitness_xy.iter().map(|(x, y)| view! {
            <circle cx={x.to_string()} cy={y.to_string()} r="2"
                    fill=COL_SUCCESS stroke=COL_BG stroke-width="1" />
        }).collect::<Vec<_>>()}
    }
}

/// Event strip — categorical dot markers.
#[component]
fn EventStrip(events: Vec<EventPoint>, t_start: f64, t_end: f64) -> impl IntoView {
    let region_top = MARGIN_TOP * 3.0 + REGION_HEIGHT * 2.0 + MARGIN_BOTTOM * 2.0;
    let rh = EVENT_STRIP_HEIGHT;

    // Assign y-position by event category
    let category_y = |code: &str| -> f64 {
        if code.starts_with("plan.") {
            0.15
        } else if code.starts_with("peer.") || code.starts_with("colony.") {
            0.35
        } else if code.starts_with("system.") {
            0.55
        } else if code.starts_with("goal.") {
            0.75
        } else {
            0.90
        }
    };

    let dots: Vec<_> = events
        .iter()
        .map(|e| {
            let x = tx(e.t, t_start, t_end);
            let y_norm = category_y(&e.code);
            let y = region_top + rh * y_norm;
            let (color, radius) = match e.level.as_str() {
                "error" => (COL_ERROR, 4.0),
                "warn" => (COL_WARNING, 3.0),
                _ => (COL_PRIMARY, 2.5),
            };
            // Truncate message for title tooltip
            let msg: String = e.message.chars().take(120).collect();
            let title = format!("[{}] {} — {}", e.level, e.code, msg);
            view! {
                <circle cx={x.to_string()} cy={y.to_string()} r={radius.to_string()}
                        fill=color opacity="0.7">
                    <title>{title}</title>
                </circle>
            }
        })
        .collect();

    // Category labels
    let categories = vec![
        (0.15, "plan"),
        (0.35, "peer"),
        (0.55, "system"),
        (0.75, "goal"),
    ];
    let labels: Vec<_> = categories
        .iter()
        .map(|(y_norm, label)| {
            let y = region_top + rh * y_norm + 3.0;
            view! {
                <text x={(MARGIN_LEFT - 8.0).to_string()} y={y.to_string()}
                      fill=COL_TEXT_DIM font-size="9" text-anchor="end">{*label}</text>
            }
        })
        .collect();

    view! {
        <rect x={MARGIN_LEFT.to_string()} y={region_top.to_string()}
              width={PLOT_WIDTH.to_string()} height={rh.to_string()}
              fill=COL_BG rx="4" />

        <text x={(MARGIN_LEFT + 8.0).to_string()} y={(region_top + 14.0).to_string()}
              fill=COL_TEXT font-size="11" font-weight="600">"Events"</text>

        {labels}
        {dots}

        // Time axis at bottom
        <GridLines region_top=region_top region_height=rh y_count=0 t_start=t_start t_end=t_end />
    }
}

/// Playhead vertical line across all regions.
#[component]
fn Playhead(playhead_x: f64) -> impl IntoView {
    if playhead_x < MARGIN_LEFT || playhead_x > SVG_WIDTH - MARGIN_RIGHT {
        return view! { <g /> }.into_view();
    }
    view! {
        <line x1={playhead_x.to_string()} y1={MARGIN_TOP.to_string()}
              x2={playhead_x.to_string()} y2={(TOTAL_HEIGHT - 20.0).to_string()}
              stroke="#ffffff" stroke-width="1.5" opacity="0.6" stroke-dasharray="4,2" />
    }
    .into_view()
}

// ── Main Timeline Page ───────────────────────────────────────────────

#[component]
pub fn TimelinePage() -> impl IntoView {
    let (data, set_data) = create_signal(None::<serde_json::Value>);
    let (loading, set_loading) = create_signal(true);
    let (playhead, set_playhead) = create_signal(1.0_f64); // 0.0-1.0, 1.0 = live
    let (playing, set_playing) = create_signal(false);
    let (speed, set_speed) = create_signal(1.0_f64);
    let (hover_info, _set_hover_info) = create_signal(None::<String>);

    // Fetch data
    let fetch = move || {
        spawn_local(async move {
            if let Ok(history) = api::fetch_soul_history().await {
                set_data.set(Some(history));
                set_loading.set(false);
            }
        });
    };

    // Initial fetch
    fetch();

    // Auto-refresh every 30s
    let _interval = gloo_timers::callback::Interval::new(30_000, move || {
        fetch();
    });

    // Playback timer: advance playhead when playing
    let _play_interval = gloo_timers::callback::Interval::new(50, move || {
        if playing.get() {
            set_playhead.update(|p| {
                *p = (*p + speed.get() * 0.002).min(1.0);
                if *p >= 1.0 {
                    set_playing.set(false);
                }
            });
        }
    });

    view! {
        <div class="timeline-page">
            <div class="timeline-header">
                <h2 class="timeline-title">"Agent Life Timeline"</h2>
                {move || {
                    let d = data.get().unwrap_or_default();
                    let cycles = d.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                    view! {
                        <span class="timeline-meta">{format!("{} cycles", cycles)}</span>
                    }
                }}
            </div>

            // Playback controls
            <div class="timeline-controls">
                <button class="tl-btn" on:click=move |_| {
                    set_playhead.set(0.0);
                    set_playing.set(true);
                }>{"|<"}</button>
                <button class="tl-btn" on:click=move |_| {
                    set_playhead.update(|p| *p = (*p - 0.05).max(0.0));
                }>{"<<"}</button>
                <button class="tl-btn tl-btn-play" on:click=move |_| {
                    set_playing.update(|p| *p = !*p);
                }>
                    {move || if playing.get() { "\u{23F8}" } else { "\u{25B6}" }}
                </button>
                <button class="tl-btn" on:click=move |_| {
                    set_playhead.update(|p| *p = (*p + 0.05).min(1.0));
                }>{">>"}</button>
                <button class="tl-btn" on:click=move |_| {
                    set_playhead.set(1.0);
                    set_playing.set(false);
                }>{">|"}</button>

                <div class="tl-speed">
                    {["1", "2", "5", "10"].into_iter().map(|s| {
                        let sv: f64 = s.parse().unwrap();
                        view! {
                            <button
                                class=move || if (speed.get() - sv).abs() < 0.1 { "tl-speed-btn active" } else { "tl-speed-btn" }
                                on:click=move |_| set_speed.set(sv)
                            >{format!("{}x", s)}</button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                <div class="tl-scrub">
                    <input type="range" min="0" max="1000" class="tl-scrub-input"
                        prop:value=move || (playhead.get() * 1000.0) as i32
                        on:input=move |ev| {
                            let val: f64 = event_target_value(&ev).parse().unwrap_or(1000.0);
                            set_playhead.set(val / 1000.0);
                            set_playing.set(false);
                        }
                    />
                </div>
            </div>

            // Hover tooltip
            {move || hover_info.get().map(|info| view! {
                <div class="tl-tooltip">{info}</div>
            })}

            // Loading state
            {move || if loading.get() {
                view! { <div class="tl-loading">"Loading history..."</div> }.into_view()
            } else {
                view! { <span /> }.into_view()
            }}

            // SVG Timeline
            {move || {
                let d = data.get().unwrap_or_default();
                let fe_pts = extract_fe_points(&d);
                let elo_pts = extract_elo_points(&d);
                let fitness_pts = extract_fitness_points(&d);
                let events = extract_events(&d);
                let (t_start, t_end) = time_range(&d);

                // Playhead x position
                let ph = playhead.get();
                let ph_x = MARGIN_LEFT + ph * PLOT_WIDTH;

                if fe_pts.is_empty() && elo_pts.is_empty() && events.is_empty() {
                    return view! {
                        <div class="tl-empty">"No history data yet. Wait for a few cycles."</div>
                    }.into_view();
                }

                view! {
                    <div class="timeline-svg-container">
                        <svg
                            viewBox={format!("0 0 {} {}", SVG_WIDTH, TOTAL_HEIGHT)}
                            class="timeline-svg"
                            preserveAspectRatio="xMidYMid meet"
                        >
                            // Background
                            <rect width={SVG_WIDTH.to_string()} height={TOTAL_HEIGHT.to_string()}
                                  fill=COL_BG rx="8" />

                            <FreeEnergyRegion points=fe_pts t_start=t_start t_end=t_end />
                            <EloFitnessRegion elo_points=elo_pts fitness_points=fitness_pts
                                              t_start=t_start t_end=t_end />
                            <EventStrip events=events t_start=t_start t_end=t_end />
                            <Playhead playhead_x=ph_x />
                        </svg>
                    </div>
                }.into_view()
            }}
        </div>
    }
}
