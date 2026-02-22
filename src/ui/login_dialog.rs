//! Login dialog — native egui email/password form for API authentication

use std::sync::{Arc, Mutex};
use egui;
use rust_i18n::t;

/// State for the login dialog
pub struct LoginDialogState {
    pub email: String,
    pub password: String,
    pub loading: bool,
    pub error: Option<String>,
    pub login_result: Arc<Mutex<Option<Result<LoginSuccess, String>>>>,
}

/// Successful login response data
pub struct LoginSuccess {
    pub token: String,
    pub email: String,
}

impl Default for LoginDialogState {
    fn default() -> Self {
        Self {
            email: String::new(),
            password: String::new(),
            loading: false,
            error: None,
            login_result: Arc::new(Mutex::new(None)),
        }
    }
}

/// Render the login dialog contents
pub fn render_login_dialog(
    ui: &mut egui::Ui,
    state: &mut LoginDialogState,
    config_manager: &mut crate::config::ConfigManager,
) -> Option<LoginSuccess> {
    let mut completed_login: Option<LoginSuccess> = None;

    // Poll for completed async login
    if state.loading {
        if let Ok(mut result) = state.login_result.lock() {
            if let Some(login_result) = result.take() {
                state.loading = false;
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

                        state.error = None;
                        state.email.clear();
                        state.password.clear();
                        completed_login = Some(success);
                    }
                    Err(err) => {
                        state.error = Some(err);
                    }
                }
            }
        }
    }

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

        // Register link
        ui.horizontal(|ui| {
            ui.weak(t!("login.no_account"));
            if ui.small_button(t!("login.register")).clicked() {
                let base_url = config_manager.system_settings().api_base_url.clone();
                let register_url = base_url.replace("api.", "").trim_end_matches('/').to_string();
                let _ = webbrowser::open(&format!("{}/register", register_url));
            }
        });
    });

    completed_login
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

/// Perform the actual login API call
async fn do_login(base_url: &str, email: &str, password: &str) -> Result<LoginSuccess, String> {
    use crate::api::client::{build_url, api_post_unauth};
    use crate::api::types::{LoginRequest, AuthResponse};

    let url = build_url(base_url, "/api/auth/login");
    let req = LoginRequest {
        email: email.to_string(),
        password: password.to_string(),
    };

    let _resp: AuthResponse = api_post_unauth(&url, &req)
        .await
        .map_err(|e| e.to_string())?;

    // AuthResponse contains the token but not user info.
    // We use the email from the login request for display.
    Ok(LoginSuccess {
        token: _resp.token,
        email: email.to_string(),
    })
}
