//! Login and registration UI state models.
//!
//! These types represent the client-side state for authentication pages,
//! including form data and validation logic.

use serde::{Deserialize, Serialize};

/// Which authentication page is currently shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthPageState {
    Login,
    Register,
    ForgotPassword,
    ResetPassword,
}

impl Default for AuthPageState {
    fn default() -> Self {
        Self::Login
    }
}

/// State for the login form.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    pub remember_me: bool,
    /// Validation or server error to display, if any.
    pub error: Option<String>,
}

impl LoginForm {
    /// Create a new empty login form.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate the login form fields.
    ///
    /// Returns `Ok(())` if valid, or an error message describing the problem.
    pub fn validate(&self) -> Result<(), String> {
        if self.email.trim().is_empty() {
            return Err("Email is required".into());
        }
        if !self.email.contains('@') {
            return Err("Invalid email address".into());
        }
        if self.password.is_empty() {
            return Err("Password is required".into());
        }
        if self.password.len() < 8 {
            return Err("Password must be at least 8 characters".into());
        }
        Ok(())
    }

    /// Clear the form to its default state.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// State for the registration form.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegisterForm {
    pub username: String,
    pub email: String,
    pub password: String,
    pub confirm_password: String,
    /// Validation or server error to display, if any.
    pub error: Option<String>,
}

impl RegisterForm {
    /// Create a new empty registration form.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate the registration form fields.
    ///
    /// Returns `Ok(())` if valid, or an error message describing the problem.
    pub fn validate(&self) -> Result<(), String> {
        if self.username.trim().is_empty() {
            return Err("Username is required".into());
        }
        if self.username.len() < 3 {
            return Err("Username must be at least 3 characters".into());
        }
        if self.email.trim().is_empty() {
            return Err("Email is required".into());
        }
        if !self.email.contains('@') {
            return Err("Invalid email address".into());
        }
        if self.password.is_empty() {
            return Err("Password is required".into());
        }
        if self.password.len() < 8 {
            return Err("Password must be at least 8 characters".into());
        }
        if self.password != self.confirm_password {
            return Err("Passwords do not match".into());
        }
        Ok(())
    }

    /// Clear the form to its default state.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_auth_page_is_login() {
        assert_eq!(AuthPageState::default(), AuthPageState::Login);
    }

    #[test]
    fn login_form_validates_email_required() {
        let form = LoginForm {
            email: "".into(),
            password: "password123".into(),
            ..Default::default()
        };
        let err = form.validate().unwrap_err();
        assert!(err.contains("Email"));
    }

    #[test]
    fn login_form_validates_email_format() {
        let form = LoginForm {
            email: "not-an-email".into(),
            password: "password123".into(),
            ..Default::default()
        };
        let err = form.validate().unwrap_err();
        assert!(err.contains("Invalid email"));
    }

    #[test]
    fn login_form_validates_password_required() {
        let form = LoginForm {
            email: "user@example.com".into(),
            password: "".into(),
            ..Default::default()
        };
        let err = form.validate().unwrap_err();
        assert!(err.contains("Password is required"));
    }

    #[test]
    fn login_form_validates_password_length() {
        let form = LoginForm {
            email: "user@example.com".into(),
            password: "short".into(),
            ..Default::default()
        };
        let err = form.validate().unwrap_err();
        assert!(err.contains("at least 8"));
    }

    #[test]
    fn login_form_accepts_valid_input() {
        let form = LoginForm {
            email: "user@example.com".into(),
            password: "secure-password".into(),
            remember_me: true,
            error: None,
        };
        assert!(form.validate().is_ok());
    }

    #[test]
    fn login_form_clear() {
        let mut form = LoginForm {
            email: "user@example.com".into(),
            password: "pass".into(),
            remember_me: true,
            error: Some("error".into()),
        };
        form.clear();
        assert!(form.email.is_empty());
        assert!(form.password.is_empty());
        assert!(!form.remember_me);
        assert!(form.error.is_none());
    }

    #[test]
    fn register_form_validates_username_required() {
        let form = RegisterForm {
            username: "".into(),
            email: "user@example.com".into(),
            password: "password123".into(),
            confirm_password: "password123".into(),
            error: None,
        };
        let err = form.validate().unwrap_err();
        assert!(err.contains("Username is required"));
    }

    #[test]
    fn register_form_validates_username_length() {
        let form = RegisterForm {
            username: "ab".into(),
            email: "user@example.com".into(),
            password: "password123".into(),
            confirm_password: "password123".into(),
            error: None,
        };
        let err = form.validate().unwrap_err();
        assert!(err.contains("at least 3"));
    }

    #[test]
    fn register_form_validates_password_mismatch() {
        let form = RegisterForm {
            username: "alice".into(),
            email: "alice@example.com".into(),
            password: "password123".into(),
            confirm_password: "different456".into(),
            error: None,
        };
        let err = form.validate().unwrap_err();
        assert!(err.contains("do not match"));
    }

    #[test]
    fn register_form_accepts_valid_input() {
        let form = RegisterForm {
            username: "alice".into(),
            email: "alice@example.com".into(),
            password: "password123".into(),
            confirm_password: "password123".into(),
            error: None,
        };
        assert!(form.validate().is_ok());
    }

    #[test]
    fn register_form_clear() {
        let mut form = RegisterForm {
            username: "alice".into(),
            email: "alice@example.com".into(),
            password: "pass".into(),
            confirm_password: "pass".into(),
            error: Some("err".into()),
        };
        form.clear();
        assert!(form.username.is_empty());
        assert!(form.email.is_empty());
        assert!(form.error.is_none());
    }
}
