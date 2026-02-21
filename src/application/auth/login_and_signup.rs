use crate::domain::entities::{AuthResponse, User};
use crate::domain::port::{TokenError, TokenService, UserRepository};

pub struct LoginAndSignup<U: UserRepository, T: TokenService> {
    user_repo: U,
    token: T,
}

impl<U: UserRepository, T: TokenService> LoginAndSignup<U, T> {
    pub fn new(user_repo: U, token: T) -> Self {
        Self(user_repo, token)
    }

    pub async fn login(&self, email: &str, pass: &str) -> Result<(String, String), LoginError> {
        // first find the user by email
        let user = &self
            .user_repo
            .find_by_email(email)
            .await
            .map_err(|_| actixweb::error::ErrorUnauthorized("Invalid Credentials"))?;
        // hash the password and confirm it
        let hashed_pass = &self.user_repo.hash_password(pass);
        let confirm_pass = &self
            .user_repo
            .verify_password(&user, &hashed_pass)
            .map_err(|_| LoginError::InvalidCredentials)?;

        if !confirm_pass {
            return Err(LoginError::InvalidCredentials);
        }

        if confirm_pass {
            let access_token = &self
                .token
                .generate_access_token(user)
                .map_err(|_| LoginError::TokenGenerationFailed)?;
            let refresh_token = &self
                .token
                .generate_refresh_token(user)
                .map_err(|_| LoginError::TokenGenerationFailed)?;
            Ok(access_token, refresh_token)
        }
    }
}

#[derive(Debug)]
pub enum LoginError {
    InvalidCredentials,
    TokenGenertionFailed,
}
