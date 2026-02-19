use crate::domain::entities::{AccessTokenPayload, RefreshTokenPayload, User};
use crate::domain::port::{TokenError, TokenService};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
//A key thing i noted is that the infrastructure layer jwt.rs doesn't creates the secret
// but collects it
pub struct JwtServiceImpl {
    secret: String, // the main stuff
    issuer: String, // who generated the secret
}

impl JwtServiceImpl {
    pub fn new(secret: String, issuer: String) -> Self {
        Self(secret, issuer)
    }
}

impl TokenService for JwtServiceImpl {
    fn generate_access_token(&self, user: &User) -> Result<String, TokenError> {
        let current_time = Utc::now();
        let expiration_time = current_time + Duration::minutes(30);
        let payload = TokenClaim {
            // this will be used as the payload for the jwt
            id: user.id,
            email: user.email.clone(),
            entity_type: user.entity_type.clone(),
            role: user.role.clone(),
            exp: expiration_time.timestamp() as usize,
            iat: current_time.timestamp() as usize,
            iss: &self.issuer.clone(),
        };

        // the main generation now
        encode(
            &Header::new(Algorithm::HS256),
            &payload,
            &EncodingKey::from_secret(&self.secret.as_bytes()),
        )
        .map_err(|e| TokenError::GenerationError(e.to_string()))
    }

    fn generate_refresh_token(&self, user: &User) -> Result<String, TokenError> {
        let current_time = Utc::now();
        let expiration_time = current_time + Duration::days(7);

        let payload = TokenClaim {
            id: user.id,
            role: user.role.clone(),
            exp: expiration_time.timestamp() as usize,
            iat: current_time.timestamp() as usize,
            iss: &self.issuer.clone(),
        };

        encode(
            &Header::new(Algorithm::HS256),
            &payload,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| TokenError::GenerationError(e.to_string()))
    }

    fn validate_access_token(&self, token: &str) -> Result<AccessTokenPayload, TokenError> {
        // this is not creating a new signature, It's a configuration object that tells the decoder
        // to expect HS256. then apply some validation checks
        let mut validation = Validation::new(Algorithm::SH256);
        validation.set_issuer(&[&self.issuer]);
        validation.validate_exp = true;
        validation.leeway = 60;

        let decoded = decode::<AccessTokenPayload>(
            // this is where validation happens
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenError::Expired,
            _ => TokenError::InvalidToken,
        });

        Ok(decoded.claims) // this claims is basically the AccessTokenPayload, after validation pass, with the data it originally contained
    }

    fn validate_refresh_token(&self, token: &str) -> Result<RefreshTokenPayload, TokenError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.issuer]);
        validation.validate_exp = true;
        validation.leeway = 60;

        let decoded = decode::<RefreshTokenPayload>(
            token,
            &Decodingkey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenError::Expired,
            _ => TokenError::InvalidToken,
        });

        Ok(decoded.claims)
    }
}
