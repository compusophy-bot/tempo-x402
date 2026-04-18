use crate::{api, WalletMode, WalletState};
use leptos::*;

/// Instance info panel — shows identity, peers, clone button
#[component]
pub fn InstancePanel() -> impl IntoView {
    let (info, set_info) = create_signal(None::<serde_json::Value>);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal(None::<String>);

    // Clone action state
    let (clone_loading, set_clone_loading) = create_signal(false);
    let (clone_result, set_clone_result) =
        create_signal(None::<Result<api::CloneResponse, String>>);

    spawn_local(async move {
        let base = api::gateway_base_url();
        let url = format!("{}/instance/info", base);
        match gloo_net::http::Request::get(&url).send().await {
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
        set_loading.set(false);
    });

    view! {
        <div class="instance-panel">
            <h3>"Instance Info"</h3>

            <Show when=move || loading.get() fallback=|| ()>
                <p class="loading">"Loading instance info..."</p>
            </Show>

            <Show when=move || error.get().is_some() fallback=|| ()>
                <p class="error-text">"Instance info unavailable"</p>
            </Show>

            <Show when=move || info.get().is_some() fallback=|| ()>
                {move || {
                    let data = info.get().unwrap_or_default();

                    let identity = data.get("identity").cloned();
                    let children_count = data.get("peer_count")
                        .or_else(|| data.get("children_count"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let clone_available = data.get("clone_available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let clone_price = data.get("clone_price")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();
                    let version = data.get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let uptime = data.get("uptime_seconds")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    let address = identity.as_ref()
                        .and_then(|id| id.get("address"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();
                    let instance_id = identity.as_ref()
                        .and_then(|id| id.get("instance_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();
                    let parent_url = identity.as_ref()
                        .and_then(|id| id.get("parent_url"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let wallet_balance = data.get("wallet_balance")
                        .and_then(|v| v.get("formatted"))
                        .and_then(|v| v.as_str())
                        .map(|s| format!("{} pathUSD", s));

                    let children = data.get("peers")
                        .or_else(|| data.get("children"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();

                    let clone_price_btn = clone_price.clone();

                    view! {
                        <div class="instance-details">
                            <div class="instance-identity">
                                <p><strong>"Address: "</strong><code>{address}</code></p>
                                {wallet_balance.map(|bal| view! {
                                    <p><strong>"Balance: "</strong>{bal}</p>
                                })}
                                <p><strong>"Instance: "</strong><code>{instance_id}</code></p>
                                <p><strong>"Version: "</strong>{version}</p>
                                <p><strong>"Uptime: "</strong>{format!("{}s", uptime)}</p>
                                {parent_url.map(|url| view! {
                                    <p><strong>"Parent: "</strong>
                                        <a href=url.clone() target="_blank">{url}</a>
                                    </p>
                                })}
                            </div>

                            <div class="clone-section">
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
                                    {let clone_price_label = clone_price_btn.clone(); move || if clone_loading.get() {
                                        "Cloning...".to_string()
                                    } else if clone_available {
                                        format!("Clone ({})", clone_price_label)
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

                            <Show when=move || { children_count > 0 } fallback=|| ()>
                                <div class="children-list">
                                    <h4>{format!("Peers ({})", children_count)}</h4>
                                    <ul>
                                        {children.iter().map(|child| {
                                            let child_id = child.get("instance_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            let child_url = child.get("url")
                                                .and_then(|v| v.as_str())
                                                .map(String::from);
                                            let child_status = child.get("status")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            view! {
                                                <li>
                                                    <code>{child_id}</code>
                                                    " — "
                                                    <span class="status-badge">{child_status}</span>
                                                    {child_url.map(|url| view! {
                                                        " "
                                                        <a href=url.clone() target="_blank">{url}</a>
                                                    })}
                                                </li>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </ul>
                                </div>
                            </Show>
                        </div>
                    }
                }}
            </Show>
        </div>
    }
}
