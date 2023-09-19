use derive_builder::Builder;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::redirect::Policy;
use reqwest::{Client, ClientBuilder, Method, RequestBuilder, Response, Url};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use thiserror::Error;

pub const API_URL: &str = "https://api.pathofexile.com";

#[derive(Debug, Builder)]
#[builder(setter(into))]
/// PoE Api Config
pub struct PoEApiConfig<'a> {
  /// Client ID
  ///
  /// **Required**
  client_id: Cow<'a, str>,
  /// Version
  ///
  /// **Required**
  version: Cow<'a, str>,
  /// Contact Email
  ///
  /// **Required**
  contact_email: Cow<'a, str>,
  /// Access Token
  ///
  /// **Required**
  access_token: Cow<'a, str>,
  #[builder(default)]
  /// User Agent Extra
  ///
  /// **Optional**
  user_agent_extra: Cow<'a, str>,
  #[builder(default)]
  /// Custom headers
  ///
  /// **Optional**
  custom_headers: HeaderMap,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoEApiError {
  error: String,
  error_description: String,
}

#[derive(Debug, Error)]
pub enum Error {
  #[error(transparent)]
  ReqwestError(#[from] reqwest::Error),
  #[error(transparent)]
  ReqwestInvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
  #[error(transparent)]
  UrlParseError(#[from] url::ParseError),
  #[error("{error}: {error_description}")]
  PoEApiError {
    error: String,
    error_description: String,
  },
}

#[derive(Debug)]
pub struct PoEApi {
  client: Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
  uuid: String,
  name: String,
  realm: Option<String>,
  locale: Option<String>,
  guild: Option<ProfileGuildOrTwitch>,
  twitch: Option<ProfileGuildOrTwitch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileGuildOrTwitch {
  name: String,
}

impl PoEApi {
  pub fn new(
    PoEApiConfig {
      client_id,
      version,
      contact_email,
      access_token,
      user_agent_extra,
      custom_headers,
      ..
    }: PoEApiConfig<'_>,
  ) -> Result<Self> {
    let mut headers = custom_headers;
    let user_agent =
      format!("OAuth {client_id}/{version} (contact: {contact_email}){user_agent_extra}");
    let authorization = format!("Bearer {access_token}");
    let mut authorization = HeaderValue::from_str(&authorization)?;

    authorization.set_sensitive(true);

    headers.insert(AUTHORIZATION, authorization);

    let client = ClientBuilder::new()
      .user_agent(user_agent)
      .default_headers(headers)
      .redirect(Policy::none())
      .build()?;

    Ok(Self { client })
  }

  fn request(&self, method: Method, endpoint: &str) -> Result<RequestBuilder> {
    let url = api_url(endpoint)?;

    Ok(self.client.request(method, url))
  }

  fn get(&self, endpoint: &str) -> Result<RequestBuilder> {
    self.request(Method::GET, endpoint)
  }

  pub async fn get_profile(&self) -> Result<Profile> {
    let response: Response = self.get("/profile")?.send().await?;

    let status = response.status();

    if status.is_client_error() || status.is_server_error() {
      let error = response.json::<PoEApiError>().await?;

      return Err(Error::PoEApiError {
        error: error.error,
        error_description: error.error_description,
      })
    }

    response.json().await.map_err(Into::into)
  }
}

pub(crate) fn api_url(endpoint: &str) -> Result<Url> {
  format!("{API_URL}{endpoint}").parse().map_err(Into::into)
}

#[cfg(test)]
mod tests {
  use crate::PoEApiConfigBuilder;

  #[test]
  fn test() {
    let builder = PoEApiConfigBuilder::create_empty()
      .client_id("client")
      .version("0.0.0")
      .access_token("token")
      .contact_email("email@email.com")
      .build();

    assert!(builder.is_ok());

    if let Ok(config) = builder {
      assert_eq!(config.client_id, "client");
      assert_eq!(config.version, "0.0.0");
      assert_eq!(config.access_token, "token");
      assert_eq!(config.contact_email, "email@email.com");
    }
  }
}
