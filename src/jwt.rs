use chrono::{Duration, Utc};
use cookie::{CookieBuilder, time::OffsetDateTime};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use once_cell::sync::OnceCell;
use rocket::http::Cookie;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, convert::TryFrom};

#[derive(Default)]
pub struct JWTConfig {
    secret: String,
}

impl JWTConfig {
    pub fn set_static<'a>(secret: impl Into<Cow<'a, str>>) {
        JWTCONFIG.get_or_init(|| Self {
            secret: secret.into().to_string(),
        });
    }
}

const JWTCONFIG: OnceCell<JWTConfig> = OnceCell::new();

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Jwt<T> {
    pub exp: i64,
    pub claims: T,
}

impl<T> Jwt<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(claims: T) -> Self {
        Self {
            exp: 4856112000,
            claims,
        }
    }

    pub fn new_with_exp(claims: T, duration: Duration) -> Self {
        let exp = (Utc::now() + duration).timestamp();
        Self { exp, claims }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }

    pub fn validate(token: &str) -> anyhow::Result<Self> {
        let binding = JWTCONFIG;
        let secret = &binding.get_or_init(|| JWTConfig::default()).secret;
        let data = decode::<Self>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(data.claims)
    }

    pub fn sign(&self) -> anyhow::Result<String> {
        let binding = JWTCONFIG;
        let secret = &binding.get_or_init(|| JWTConfig::default()).secret;
        Ok(encode(
            &Header::default(),
            &self,
            &EncodingKey::from_secret(secret.as_bytes()),
        )?)
    }

    pub fn as_cookie<'c>(
        &self,
        name: impl Into<Cow<'static, str>>,
        client_exp: bool,
    ) -> anyhow::Result<CookieBuilder<'c>> {
        let mut builder = CookieBuilder::new(name, self.sign()?);

        if client_exp {
            builder = builder.expires(OffsetDateTime::from_unix_timestamp(self.exp)?);
        }
        Ok(builder)
    }
}

impl<T> TryFrom<Cookie<'_>> for Jwt<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    type Error = anyhow::Error;
    fn try_from(value: Cookie<'_>) -> Result<Self, Self::Error> {
        Self::validate(value.value())
    }
}
