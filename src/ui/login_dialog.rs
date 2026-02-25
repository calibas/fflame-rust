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
    pub reg_email: String,
    pub reg_password: String,
    pub reg_confirm: String,
    pub reg_loading: bool,
    pub reg_error: Option<String>,
    pub reg_result: Arc<Mutex<Option<Result<LoginSuccess, String>>>>,
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
            reg_email: String::new(),
            reg_password: String::new(),
            reg_confirm: String::new(),
            reg_loading: false,
            reg_error: None,
            reg_result: Arc::new(Mutex::new(None)),
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

    let is_signed_in = config_manager.system_settings().auth_token.is_some();

    if is_signed_in {
        render_account_info(ui, config_manager, sign_out);
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
    config_manager: &crate::config::ConfigManager,
    sign_out: &mut bool,
) {
    let settings = config_manager.system_settings();
    let email = settings.auth_email.clone().unwrap_or_else(|| "Unknown".to_string());

    ui.vertical_centered(|ui| {
        ui.add_space(12.0);

        ui.heading(t!("login.account_heading"));
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("login.email"));
            ui.strong(&email);
        });

        ui.add_space(16.0);

        if ui.button(t!("auth.sign_out")).clicked() {
            *sign_out = true;
        }
    });
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
            let email_response = ui.add(
                egui::TextEdit::singleline(&mut state.email)
                    .desired_width(200.0)
                    .hint_text("user@example.com")
                    .interactive(!state.loading),
            );

            // Auto-focus email field when dialog opens
            if state.email.is_empty() && state.password.is_empty() && state.error.is_none() {
                email_response.request_focus();
            }
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

            // Submit on Enter in password field
            if pw_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if !state.email.is_empty() && !state.password.is_empty() && !state.loading {
                    trigger_login(state, config_manager);
                }
            }
        });

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

        // Email field
        ui.horizontal(|ui| {
            ui.label(t!("login.email"));
            let email_response = ui.add(
                egui::TextEdit::singleline(&mut state.reg_email)
                    .desired_width(200.0)
                    .hint_text("user@example.com")
                    .interactive(!state.reg_loading),
            );

            if state.reg_email.is_empty() && state.reg_password.is_empty() && state.reg_error.is_none() {
                email_response.request_focus();
            }
        });

        ui.add_space(4.0);

        // Password field
        ui.horizontal(|ui| {
            ui.label(t!("login.password"));
            ui.add(
                egui::TextEdit::singleline(&mut state.reg_password)
                    .desired_width(200.0)
                    .password(true)
                    .interactive(!state.reg_loading),
            );
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

            // Submit on Enter
            if confirm_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if can_register(state) {
                    trigger_register(state, config_manager);
                }
            }
        });

        // Password mismatch warning
        if !state.reg_confirm.is_empty() && state.reg_password != state.reg_confirm {
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
    !state.reg_email.is_empty()
        && !state.reg_password.is_empty()
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
                settings.auth_token = Some(success.token.clone());
                settings.auth_email = Some(success.email.clone());
                let _ = settings.save();

                // On WASM, also save to localStorage for async helper compatibility
                #[cfg(target_arch = "wasm32")]
                {
                    if let Some(storage) = web_sys::window()
                        .and_then(|w| w.local_storage().ok().flatten())
                    {
                        let _ = storage.set_item("fflame_auth_token", &success.token);
                        let _ = storage.set_item("fflame_auth_email", &success.email);
                    }
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
                    state.reg_error = Some(err);
                } else {
                    state.error = Some(err);
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
    let base_url = config_manager.system_settings().api_base_url.clone();
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

    let email = state.reg_email.clone();
    let password = state.reg_password.clone();
    let base_url = config_manager.system_settings().api_base_url.clone();
    let result_slot = state.reg_result.clone();

    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let result = do_register(&base_url, &email, &password).await;
            *result_slot.lock().unwrap() = Some(result);
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::spawn(move || {
            let result = pollster::block_on(do_register(&base_url, &email, &password));
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

/// Perform the actual register API call
async fn do_register(base_url: &str, email: &str, password: &str) -> Result<LoginSuccess, String> {
    use crate::api::client::{build_url, api_post_unauth};
    use crate::api::types::{RegisterRequest, AuthResponse};

    let url = build_url(base_url, "/api/auth/register");
    let req = RegisterRequest {
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
