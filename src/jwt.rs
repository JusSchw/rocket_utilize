use chrono::{DateTime, Duration, Utc};
use cookie::{CookieBuilder, time::OffsetDateTime};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rocket::http::Cookie;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, convert::TryFrom, env};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Jwt<T> {
    pub exp: Option<DateTime<Utc>>,
    pub claims: T,
}

impl<T> Jwt<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(claims: T) -> Self {
        Self { exp: None, claims }
    }

    pub fn new_with_exp(claims: T, duration: Duration) -> Self {
        Self {
            exp: Some(Utc::now() + duration),
            claims,
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.exp {
            Some(exp) => Utc::now() > exp,
            None => false,
        }
    }

    pub fn validate(token: &str) -> anyhow::Result<Self> {
        let secret = env::var("JWT_SECRET")?;
        let data = decode::<T>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(Self {
            exp: None,
            claims: data.claims,
        })
    }

    pub fn sign(&self) -> anyhow::Result<String> {
        let secret = env::var("JWT_SECRET")?;
        Ok(encode(
            &Header::default(),
            &self.claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )?)
    }

    pub fn as_cookie<'c>(
        &self,
        name: impl Into<Cow<'static, str>>,
        client_exp: bool,
    ) -> anyhow::Result<CookieBuilder<'c>> {
        let builder = CookieBuilder::new(name, self.sign()?);
        match self.exp {
            Some(exp) if client_exp => {
                Ok(builder.expires(OffsetDateTime::from_unix_timestamp(exp.timestamp())?))
            }
            _ => Ok(builder),
        }
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
