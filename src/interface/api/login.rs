use actix_web::{post, web, HttpResponse, Result};
use application::auth::login_and_signup::LoginAndSignup;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[post("/api/auth/login")] // login_and_signup: web::data<LoginAndSignup<>> ??
pub async fn login(
    &self,
    request: web::Json<LoginRequest>,
    login_and_signup: web::data<LoginAndSignup>,
) -> Result<HttpResponse> {
    let (access_token, refresh_token) = login_and_signup
        .login(&body.email, &body.pass)
        .await
        .map_err(|_| actixweb::error::ErrorUnauthorized("Invalid Credentials"))?;

    Ok(HttpResponse::Ok()
        .cookie(
            actix_web::cookie::Cookie::build("refresh_token", refresh_token)
                .http_only(true)
                .secure(true)
                .same_site(actix_web::cookie::SameSite::Strict)
                .max_age(actix_web::cookie::time::Duration::days(7))
                .path("api/auth")
                .finish(),
        ) // when returned, stored in client side?
        .json(serde::Json!({
            "access_token": access_token,
            "token_type": "Bearer"
            "expires_in": 900
        })))
}
