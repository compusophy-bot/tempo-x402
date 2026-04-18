use crate::{api, wallet, WalletMode, WalletState};
use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
pub fn WalletButtons(
    wallet: ReadSignal<WalletState>,
    set_wallet: WriteSignal<WalletState>,
) -> impl IntoView {
    let (funding, set_funding) = create_signal(false);

    let connect_metamask = move |_| {
        spawn_local(async move {
            match wallet::connect_wallet().await {
                Ok(state) => set_wallet.set(state),
                Err(e) => {
                    web_sys::console::error_1(&format!("MetaMask error: {}", e).into());
                }
            }
        });
    };

    let use_demo = move |_| match wallet::use_demo_key() {
        Ok(state) => {
            let pk = state.private_key.clone();
            set_wallet.set(state);
            // Auto-setup: fund via faucet + approve facilitator
            if let Some(key) = pk {
                spawn_local(async move {
                    match api::setup_wallet(&key).await {
                        Ok(resp) => {
                            web_sys::console::log_1(&format!("Wallet setup: {:?}", resp).into());
                        }
                        Err(e) => {
                            web_sys::console::warn_1(&format!("Wallet setup failed: {}", e).into());
                        }
                    }
                });
            }
        }
        Err(e) => {
            web_sys::console::error_1(&format!("Demo key error: {}", e).into());
        }
    };

    let create_wallet = move |_| {
        set_funding.set(true);
        match wallet::load_or_create_embedded_wallet() {
            Ok((state, is_new)) => {
                let address = state.address.clone().unwrap_or_default();
                let pk = state.private_key.clone();
                set_wallet.set(state);
                spawn_local(async move {
                    if is_new {
                        match wallet::fund_address(&address).await {
                            Ok(_) => {
                                web_sys::console::log_1(
                                    &format!("Funded new wallet: {}", address).into(),
                                );
                            }
                            Err(e) => {
                                web_sys::console::error_1(&format!("Funding failed: {}", e).into());
                            }
                        }
                    }
                    // Auto-setup: fund via faucet + approve facilitator
                    if let Some(key) = pk {
                        match api::setup_wallet(&key).await {
                            Ok(resp) => {
                                web_sys::console::log_1(
                                    &format!("Wallet setup: {:?}", resp).into(),
                                );
                            }
                            Err(e) => {
                                web_sys::console::warn_1(
                                    &format!("Wallet setup failed: {}", e).into(),
                                );
                            }
                        }
                    }
                    set_funding.set(false);
                });
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Create wallet error: {}", e).into());
                set_funding.set(false);
            }
        }
    };

    let disconnect = move |_| {
        set_wallet.set(WalletState::default());
    };

    view! {
        <Show
            when=move || wallet.get().connected
            fallback=move || view! {
                <div class="wallet-dropdown">
                    <button class="btn btn-secondary wallet-dropdown-trigger"
                        on:click=move |_| {
                            let el = web_sys::window().unwrap()
                                .document().unwrap()
                                .query_selector(".wallet-dropdown-menu").unwrap();
                            if let Some(el) = el {
                                let display = el.dyn_ref::<web_sys::HtmlElement>().unwrap().style().get_property_value("display").unwrap_or_default();
                                el.dyn_ref::<web_sys::HtmlElement>().unwrap().style().set_property("display",
                                    if display == "block" { "none" } else { "block" }
                                ).ok();
                            }
                        }
                    >
                        "Account"
                    </button>
                    <div class="wallet-dropdown-menu" style="display:none">
                        <button class="wallet-dropdown-item" on:click=connect_metamask>
                            "Connect Wallet"
                        </button>
                        <button class="wallet-dropdown-item" on:click=use_demo>
                            "Demo Key"
                        </button>
                        <button
                            class="wallet-dropdown-item"
                            on:click=create_wallet
                            disabled=move || funding.get()
                        >
                            {move || {
                                if funding.get() {
                                    "Creating..."
                                } else if wallet::has_stored_wallet() {
                                    "Restore Wallet"
                                } else {
                                    "Create Wallet"
                                }
                            }}
                        </button>
                    </div>
                </div>
            }
        >
            {move || {
                let w = wallet.get();
                let addr = w.address.unwrap_or_default();
                let short = if addr.len() > 10 {
                    format!("{}...{}", &addr[..6], &addr[addr.len()-4..])
                } else {
                    addr
                };
                let mode_label = w.mode.label();
                view! {
                    <div class="wallet-info">
                        <span class="wallet-mode-badge">{mode_label}</span>
                        <span class="wallet-address">{short}</span>
                        <button class="btn btn-secondary btn-sm" on:click=disconnect>
                            "Disconnect"
                        </button>
                    </div>
                }
            }}
        </Show>
    }
}

/// Wallet management panel — export, import, delete for embedded wallets.
#[component]
pub fn WalletManagement() -> impl IntoView {
    let (wallet, set_wallet) =
        expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();

    let (show_key, set_show_key) = create_signal(false);
    let (show_import, set_show_import) = create_signal(false);
    let (import_value, set_import_value) = create_signal(String::new());
    let (import_error, set_import_error) = create_signal(None::<String>);
    let (confirm_delete, set_confirm_delete) = create_signal(false);
    let (copied, set_copied) = create_signal(false);

    let toggle_reveal = move |_| {
        set_show_key.update(|v| *v = !*v);
        set_copied.set(false);
    };

    let copy_key = move |_| {
        if let Some(key) = wallet.get().private_key {
            wallet::copy_to_clipboard(&key);
            set_copied.set(true);
        }
    };

    let download_json = move |_| {
        let w = wallet.get();
        if let (Some(key), Some(addr)) = (w.private_key, w.address) {
            let json = wallet::export_wallet_json(&key, &addr);
            let filename = format!("x402-wallet-{}.json", &addr[..8]);
            if let Err(e) = wallet::trigger_download(&filename, &json) {
                web_sys::console::error_1(&format!("Download failed: {}", e).into());
            }
        }
    };

    let toggle_import = move |_| {
        set_show_import.update(|v| *v = !*v);
        set_import_error.set(None);
        set_import_value.set(String::new());
    };

    let do_import = move |_| {
        let key = import_value.get();
        match wallet::import_embedded_wallet(&key) {
            Ok(state) => {
                set_wallet.set(state);
                set_show_import.set(false);
                set_import_error.set(None);
            }
            Err(e) => {
                set_import_error.set(Some(e));
            }
        }
    };

    let do_delete = move |_| {
        wallet::delete_embedded_wallet();
        set_wallet.set(WalletState::default());
        set_confirm_delete.set(false);
    };

    view! {
        <Show when=move || wallet.get().mode == WalletMode::Embedded fallback=|| ()>
            <div class="wallet-management">
                <h4>"Wallet Management"</h4>

                <div class="wallet-actions">
                    <button class="btn btn-secondary btn-sm" on:click=toggle_reveal>
                        {move || if show_key.get() { "Hide Key" } else { "Reveal Key" }}
                    </button>

                    <Show when=move || show_key.get() fallback=|| ()>
                        <div class="key-reveal">
                            <code class="private-key">{move || wallet.get().private_key.unwrap_or_default()}</code>
                            <button class="btn btn-secondary btn-sm" on:click=copy_key>
                                {move || if copied.get() { "Copied!" } else { "Copy" }}
                            </button>
                        </div>
                    </Show>

                    <button class="btn btn-secondary btn-sm" on:click=download_json>
                        "Download Backup"
                    </button>

                    <button class="btn btn-secondary btn-sm" on:click=toggle_import>
                        {move || if show_import.get() { "Cancel Import" } else { "Import Key" }}
                    </button>

                    <Show when=move || show_import.get() fallback=|| ()>
                        <div class="import-form">
                            <input
                                type="password"
                                class="input"
                                placeholder="Paste private key (0x...)"
                                prop:value=move || import_value.get()
                                on:input=move |ev| {
                                    set_import_value.set(event_target_value(&ev));
                                    set_import_error.set(None);
                                }
                            />
                            <button class="btn btn-primary btn-sm" on:click=do_import>
                                "Import"
                            </button>
                            <Show when=move || import_error.get().is_some() fallback=|| ()>
                                <p class="error-text">{move || import_error.get().unwrap_or_default()}</p>
                            </Show>
                        </div>
                    </Show>

                    <Show
                        when=move || confirm_delete.get()
                        fallback=move || view! {
                            <button
                                class="btn btn-danger btn-sm"
                                on:click=move |_| set_confirm_delete.set(true)
                            >
                                "Delete Wallet"
                            </button>
                        }
                    >
                        <div class="delete-confirm">
                            <p class="warning-text">
                                "This permanently deletes your private key. Make sure you have a backup!"
                            </p>
                            <button class="btn btn-danger btn-sm" on:click=do_delete>
                                "Yes, Delete Forever"
                            </button>
                            <button
                                class="btn btn-secondary btn-sm"
                                on:click=move |_| set_confirm_delete.set(false)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </Show>
                </div>
            </div>
        </Show>
    }
}
