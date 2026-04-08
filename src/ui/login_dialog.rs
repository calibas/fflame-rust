//! Account panel — Sign In, Register, and Account Info
//!
//! Shows different content based on auth state:
//! - Not signed in: tabbed Sign In / Register forms
//! - Signed in: account info with sign-out button

use std::sync::{Arc, Mutex};
use egui;
use rust_i18n::t;

/// Which form is shown when not signed in
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthTab {
    SignIn,
    Register,
}

/// State for the login/register/account panel
pub struct LoginDialogState {
    /// Which tab is active (only when not signed in)
    pub tab: AuthTab,

    // Sign In fields
    pub email: String,
    pub password: String,
    pub loading: bool,
    pub error: Option<String>,
    pub login_result: Arc<Mutex<Option<Result<LoginSuccess, String>>>>,

    // Register fields
    pub reg_display_name: String,
    pub reg_email: String,
    pub reg_password: String,
    pub reg_confirm: String,
    pub reg_loading: bool,
    pub reg_error: Option<String>,
    pub reg_result: Arc<Mutex<Option<Result<LoginSuccess, String>>>>,

    // Remember me (desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    pub remember_me: bool,

    // Profile info (fetched from /api/users/me when account panel is open)
    pub profile: Option<crate::api::types::ApiUser>,
    pub profile_loading: bool,
    pub profile_result: Arc<Mutex<Option<Result<crate::api::types::ApiUser, String>>>>,
}

/// Successful login/register response data
pub struct LoginSuccess {
    pub token: String,
    pub email: String,
}

impl Default for LoginDialogState {
    fn default() -> Self {
        Self {
            tab: AuthTab::SignIn,
            email: String::new(),
            password: String::new(),
            loading: false,
            error: None,
            login_result: Arc::new(Mutex::new(None)),
            reg_display_name: String::new(),
            reg_email: String::new(),
            reg_password: String::new(),
            reg_confirm: String::new(),
            reg_loading: false,
            reg_error: None,
            reg_result: Arc::new(Mutex::new(None)),
            #[cfg(not(target_arch = "wasm32"))]
            remember_me: false,
            profile: None,
            profile_loading: false,
            profile_result: Arc::new(Mutex::new(None)),
        }
    }
}

/// Render the account panel.
/// Returns `Some(LoginSuccess)` when a login or register succeeds.
/// `sign_out` is set to true when the user clicks Sign Out.
pub fn render_login_dialog(
    ui: &mut egui::Ui,
    state: &mut LoginDialogState,
    config_manager: &mut crate::config::ConfigManager,
    sign_out: &mut bool,
) -> Option<LoginSuccess> {
    let mut completed: Option<LoginSuccess> = None;

    // Poll for completed async login
    completed = completed.or_else(|| poll_result(state, config_manager, false));
    // Poll for completed async register
    completed = completed.or_else(|| poll_result(state, config_manager, true));

    let is_signed_in = config_manager.system_settings().is_signed_in();

    if is_signed_in {
        render_account_info(ui, state, config_manager, sign_out);
    } else {
        // Tab bar: Sign In / Register
        ui.horizontal(|ui| {
            if ui.selectable_label(state.tab == AuthTab::SignIn, t!("login.sign_in")).clicked() {
                state.tab = AuthTab::SignIn;
            }
            if ui.selectable_label(state.tab == AuthTab::Register, t!("login.register")).clicked() {
                state.tab = AuthTab::Register;
            }
        });
        ui.separator();

        match state.tab {
            AuthTab::SignIn => {
                render_sign_in_form(ui, state, config_manager);
            }
            AuthTab::Register => {
                render_register_form(ui, state, config_manager);
            }
        }
    }

    completed
}

/// Render account info when signed in
fn render_account_info(
    ui: &mut egui::Ui,
    state: &mut LoginDialogState,
    config_manager: &crate::config::ConfigManager,
    sign_out: &mut bool,
) {
    // Trigger profile fetch if not yet loaded
    if state.profile.is_none() && !state.profile_loading {
        state.profile_loading = true;
        let result_slot = state.profile_result.clone();
        let settings = config_manager.system_settings();
        let token = settings.get_auth_token().unwrap_or_default();

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = fetch_profile(&token).await;
                *result_slot.lock().unwrap() = Some(result);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::spawn(move || {
                let result = pollster::block_on(fetch_profile(&token));
                *result_slot.lock().unwrap() = Some(result);
            });
        }
    }

    // Poll for completed profile fetch
    if state.profile_loading {
        if let Some(result) = state.profile_result.lock().ok().and_then(|mut r| r.take()) {
            state.profile_loading = false;
            match result {
                Ok(user) => { state.profile = Some(user); }
                Err(e) => { log::error!("Failed to fetch profile: {}", e); }
            }
        }
    }

    let settings = config_manager.system_settings();
    let email = settings.auth_email.clone().unwrap_or_else(|| "Unknown".to_string());

    ui.vertical_centered(|ui| {
        ui.add_space(12.0);

        ui.heading(t!("login.account_heading"));
        ui.add_space(8.0);

        if let Some(ref profile) = state.profile {
            ui.horizontal(|ui| {
                ui.label(t!("login.display_name"));
                ui.strong(&profile.display_name);
            });
        }

        ui.horizontal(|ui| {
            ui.label(t!("login.email"));
            ui.strong(&email);
        });

        ui.add_space(16.0);

        if ui.button(t!("auth.sign_out")).clicked() {
            state.profile = None; // Clear cached profile on sign out
            *sign_out = true;
        }
    });
}

/// Map raw API error strings to user-friendly messages for registration.
fn friendly_register_error(err: &str) -> String {
    if err.contains("display_name_taken") {
        t!("login.error_display_name_taken").to_string()
    } else if err.contains("email_taken") {
        t!("login.error_email_taken").to_string()
    } else {
        err.to_string()
    }
}

/// Fetch the current user's profile from the API.
async fn fetch_profile(token: &str) -> Result<crate::api::types::ApiUser, String> {
    use crate::api::client::{build_url, api_get};
    use crate::api::API_BASE_URL;

    let url = build_url(API_BASE_URL, "/api/users/me");
    api_get(&url, token).await.map_err(|e| e.to_string())
}

/// Render the sign-in form
fn render_sign_in_form(
    ui: &mut egui::Ui,
    state: &mut LoginDialogState,
    config_manager: &crate::config::ConfigManager,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(8.0);

        // Error display
        if let Some(ref error) = state.error {
            ui.colored_label(egui::Color32::RED, error);
            ui.add_space(4.0);
        }

        // Email field
        ui.horizontal(|ui| {
            ui.label(t!("login.email"));
            let r = ui.add(
                egui::TextEdit::singleline(&mut state.email)
                    .desired_width(200.0)
                    .hint_text("user@example.com")
                    .interactive(!state.loading),
            );
            super::vkb_sync(ui, &r, &state.email);
        });

        ui.add_space(4.0);

        // Password field
        ui.horizontal(|ui| {
            ui.label(t!("login.password"));
            let pw_response = ui.add(
                egui::TextEdit::singleline(&mut state.password)
                    .desired_width(200.0)
                    .password(true)
                    .interactive(!state.loading),
            );
            super::vkb_sync(ui, &pw_response, &state.password);

            // Submit on Enter in password field
            if pw_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if !state.email.is_empty() && !state.password.is_empty() && !state.loading {
                    trigger_login(state, config_manager);
                }
            }
        });

        // Remember me checkbox (desktop only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            ui.add_space(4.0);
            ui.checkbox(&mut state.remember_me, t!("login.remember_me"));
        }

        ui.add_space(8.0);

        // Buttons
        ui.horizontal(|ui| {
            let sign_in_enabled = !state.email.is_empty() && !state.password.is_empty() && !state.loading;

            if state.loading {
                ui.spinner();
                ui.label(t!("login.signing_in"));
            } else {
                if ui.add_enabled(sign_in_enabled, egui::Button::new(t!("login.sign_in"))).clicked() {
                    trigger_login(state, config_manager);
                }
            }
        });

        ui.add_space(4.0);

        // Link to register tab
        ui.horizontal(|ui| {
            ui.weak(t!("login.no_account"));
            if ui.small_button(t!("login.register")).clicked() {
                state.tab = AuthTab::Register;
            }
        });
    });
}

/// Render the register form
fn render_register_form(
    ui: &mut egui::Ui,
    state: &mut LoginDialogState,
    config_manager: &crate::config::ConfigManager,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(8.0);

        // Error display
        if let Some(ref error) = state.reg_error {
            ui.colored_label(egui::Color32::RED, error);
            ui.add_space(4.0);
        }

        // Display name field
        ui.horizontal(|ui| {
            ui.label(t!("login.display_name"));
            let r = ui.add(
                egui::TextEdit::singleline(&mut state.reg_display_name)
                    .desired_width(200.0)
                    .hint_text("User")
                    .interactive(!state.reg_loading),
            );
            super::vkb_sync(ui, &r, &state.reg_display_name);
        });

        // Email field
        ui.horizontal(|ui| {
            ui.label(t!("login.email"));
            let r = ui.add(
                egui::TextEdit::singleline(&mut state.reg_email)
                    .desired_width(200.0)
                    .hint_text("user@example.com")
                    .interactive(!state.reg_loading),
            );
            super::vkb_sync(ui, &r, &state.reg_email);
        });

        ui.add_space(4.0);

        // Password field
        ui.horizontal(|ui| {
            ui.label(t!("login.password"));
            let r = ui.add(
                egui::TextEdit::singleline(&mut state.reg_password)
                    .desired_width(200.0)
                    .password(true)
                    .interactive(!state.reg_loading),
            );
            super::vkb_sync(ui, &r, &state.reg_password);
        });

        ui.add_space(4.0);

        // Confirm password field
        ui.horizontal(|ui| {
            ui.label(t!("login.confirm_password"));
            let confirm_response = ui.add(
                egui::TextEdit::singleline(&mut state.reg_confirm)
                    .desired_width(200.0)
                    .password(true)
                    .interactive(!state.reg_loading),
            );
            super::vkb_sync(ui, &confirm_response, &state.reg_confirm);

            // Submit on Enter
            if confirm_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if can_register(state) {
                    trigger_register(state, config_manager);
                }
            }
        });

        // Password validation warnings
        if !state.reg_password.is_empty() && state.reg_password.len() < 8 {
            ui.add_space(2.0);
            ui.colored_label(egui::Color32::from_rgb(220, 160, 60), t!("login.password_too_short"));
        } else if !state.reg_confirm.is_empty() && state.reg_password != state.reg_confirm {
            ui.add_space(2.0);
            ui.colored_label(egui::Color32::from_rgb(220, 160, 60), t!("login.passwords_mismatch"));
        }

        ui.add_space(8.0);

        // Buttons
        ui.horizontal(|ui| {
            if state.reg_loading {
                ui.spinner();
                ui.label(t!("login.registering"));
            } else {
                let enabled = can_register(state);
                if ui.add_enabled(enabled, egui::Button::new(t!("login.register"))).clicked() {
                    trigger_register(state, config_manager);
                }
            }
        });

        ui.add_space(4.0);

        // Link to sign in tab
        ui.horizontal(|ui| {
            ui.weak(t!("login.have_account"));
            if ui.small_button(t!("login.sign_in")).clicked() {
                state.tab = AuthTab::SignIn;
            }
        });
    });
}

/// Check if register form is ready to submit
fn can_register(state: &LoginDialogState) -> bool {
    !state.reg_display_name.is_empty()
        && !state.reg_email.is_empty()
        && state.reg_password.len() >= 8
        && state.reg_password == state.reg_confirm
        && !state.reg_loading
}

/// Poll async results for login or register
fn poll_result(
    state: &mut LoginDialogState,
    config_manager: &mut crate::config::ConfigManager,
    is_register: bool,
) -> Option<LoginSuccess> {
    let loading = if is_register { state.reg_loading } else { state.loading };
    if !loading {
        return None;
    }

    let result_slot = if is_register {
        &state.reg_result
    } else {
        &state.login_result
    };

    let result = {
        if let Ok(mut slot) = result_slot.lock() {
            slot.take()
        } else {
            None
        }
    };

    if let Some(login_result) = result {
        if is_register {
            state.reg_loading = false;
        } else {
            state.loading = false;
        }

        match login_result {
            Ok(success) => {
                // Save to SystemSettings
                let settings = config_manager.system_settings_mut();
                settings.auth_email = Some(success.email.clone());
                // On desktop, save token and persist; on WASM, cookies handle auth
                #[cfg(not(target_arch = "wasm32"))]
                {
                    settings.auth_token = Some(success.token.clone());

                    // Save or clear encrypted credentials based on "Remember me"
                    if !is_register && state.remember_me {
                        match crate::storage::credentials::save_credentials(
                            &state.email,
                            &state.password,
                        ) {
                            Ok(saved) => {
                                settings.saved_credentials = Some(saved);
                                log::info!("Saved encrypted credentials for auto-login");
                            }
                            Err(e) => {
                                log::error!("Failed to encrypt credentials: {}", e);
                                settings.saved_credentials = None;
                            }
                        }
                    } else if !is_register {
                        // Explicitly unchecked — clear any previously saved credentials
                        settings.saved_credentials = None;
                    }

                    let _ = settings.save();
                }

                // Clear form fields
                if is_register {
                    state.reg_error = None;
                    state.reg_email.clear();
                    state.reg_password.clear();
                    state.reg_confirm.clear();
                } else {
                    state.error = None;
                    state.email.clear();
                    state.password.clear();
                }

                return Some(success);
            }
            Err(err) => {
                if is_register {
                    state.reg_error = Some(friendly_register_error(&err));
                } else {
                    state.error = Some(err);
                    state.password.clear();
                }
            }
        }
    }

    None
}

/// Trigger the async login request
fn trigger_login(state: &mut LoginDialogState, config_manager: &crate::config::ConfigManager) {
    state.loading = true;
    state.error = None;

    let email = state.email.clone();
    let password = state.password.clone();
    let base_url = crate::api::API_BASE_URL.to_string();
    let result_slot = state.login_result.clone();

    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let result = do_login(&base_url, &email, &password).await;
            *result_slot.lock().unwrap() = Some(result);
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::spawn(move || {
            let result = pollster::block_on(do_login(&base_url, &email, &password));
            *result_slot.lock().unwrap() = Some(result);
        });
    }
}

/// Trigger the async register request
fn trigger_register(state: &mut LoginDialogState, config_manager: &crate::config::ConfigManager) {
    state.reg_loading = true;
    state.reg_error = None;

    let display_name = state.reg_display_name.clone();
    let email = state.reg_email.clone();
    let password = state.reg_password.clone();
    let base_url = crate::api::API_BASE_URL.to_string();
    let result_slot = state.reg_result.clone();

    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let result = do_register(&base_url, &display_name, &email, &password).await;
            *result_slot.lock().unwrap() = Some(result);
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::spawn(move || {
            let result = pollster::block_on(do_register(&base_url, &display_name, &email, &password));
            *result_slot.lock().unwrap() = Some(result);
        });
    }
}

/// Perform the actual login API call
async fn do_login(base_url: &str, email: &str, password: &str) -> Result<LoginSuccess, String> {
    use crate::api::client::{build_url, api_post_unauth};
    use crate::api::types::{LoginRequest, AuthResponse};

    let url = build_url(base_url, "/api/auth/login");
    let req = LoginRequest {
        email: email.to_string(),
        password: password.to_string(),
    };

    let resp: AuthResponse = api_post_unauth(&url, &req)
        .await
        .map_err(|e| e.to_string())?;

    Ok(LoginSuccess {
        token: resp.token,
        email: email.to_string(),
    })
}

/// Poll for auto-login result (called from main render loop, desktop only).
/// Ensures the async login result is processed even when the Login panel isn't visible.
#[cfg(not(target_arch = "wasm32"))]
pub fn poll_auto_login_result(
    state: &mut LoginDialogState,
    config_manager: &mut crate::config::ConfigManager,
) -> Option<LoginSuccess> {
    poll_result(state, config_manager, false)
}

/// Attempt auto-login from saved encrypted credentials (desktop only).
/// Decrypts saved credentials, fills form fields, and triggers login.
/// Returns true if auto-login was triggered.
#[cfg(not(target_arch = "wasm32"))]
pub fn try_auto_login(
    state: &mut LoginDialogState,
    config_manager: &crate::config::ConfigManager,
) -> bool {
    let settings = config_manager.system_settings();

    // Skip if already signed in or login in progress
    if settings.is_signed_in() || state.loading {
        return false;
    }

    let saved = match &settings.saved_credentials {
        Some(s) => s.clone(),
        None => return false,
    };

    match crate::storage::credentials::load_credentials(&saved) {
        Ok((email, password)) => {
            log::info!("Auto-login: decrypted saved credentials");
            state.email = email;
            state.password = password;
            state.remember_me = true;
            trigger_login(state, config_manager);
            true
        }
        Err(e) => {
            log::error!("Auto-login: failed to decrypt credentials: {}", e);
            false
        }
    }
}

/// Perform the actual register API call
async fn do_register(base_url: &str, display_name: &str, email: &str, password: &str) -> Result<LoginSuccess, String> {
    use crate::api::client::{build_url, api_post_unauth};
    use crate::api::types::{RegisterRequest, AuthResponse};

    let url = build_url(base_url, "/api/auth/register");
    let req = RegisterRequest {
        display_name: display_name.to_string(),
        email: email.to_string(),
        password: password.to_string(),
    };

    let resp: AuthResponse = api_post_unauth(&url, &req)
        .await
        .map_err(|e| e.to_string())?;

    Ok(LoginSuccess {
        token: resp.token,
        email: email.to_string(),
    })
}
