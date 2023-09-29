use std::borrow::Cow;

use derivative::Derivative;
use derive_builder::Builder;
use reqwest::redirect::Policy;
use reqwest::{Client, ClientBuilder, Method, RequestBuilder, Response, Url};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const API_URL: &str = "https://api.pathofexile.com";

#[derive(Derivative, Builder)]
#[derivative(Debug)]
#[builder(pattern = "owned")]
/// PoE Api Config
pub struct PoEApiConfig<'a> {
  /// Client ID
  ///
  /// **Required**
  #[builder(setter(into))]
  client_id: Cow<'a, str>,
  /// Version
  ///
  /// **Required**
  #[builder(setter(into))]
  version: Cow<'a, str>,
  /// Contact Email
  ///
  /// **Required**
  #[builder(setter(into))]
  contact_email: Cow<'a, str>,

  #[builder(setter(custom), default = "ClientBuilder::new()")]
  // #[derivative(Debug = "ignore")]
  client_builder: ClientBuilder,
}

impl<'a> PoEApiConfigBuilder<'a> {
  pub fn client_builder<F>(mut self, f: F) -> Self
  where
    F: FnOnce(ClientBuilder) -> ClientBuilder + 'a,
  {
    self.client_builder = self.client_builder.map(f);
    self
  }
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
  pub fn new(config: PoEApiConfig<'_>) -> Result<Self> {
    let PoEApiConfig {
      client_id,
      version,
      contact_email,
      client_builder,
    } = config;

    let user_agent = format!("OAuth {client_id}/{version} (contact: {contact_email})");

    let client = client_builder
      .user_agent(user_agent)
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

  pub async fn get_profile(&self, token: &str) -> Result<Profile> {
    self
      .get("/profile")?
      .bearer_auth(token)
      .send_checked()
      .await?
      .json()
      .await
      .map_err(Into::into)
  }
}

pub(crate) fn api_url(endpoint: &str) -> Result<Url> {
  format!("{API_URL}{endpoint}").parse().map_err(Into::into)
}

#[async_trait::async_trait]
pub(crate) trait RequestBuilderExt2 {
  type Error;

  async fn send_checked(self) -> Result<Response, Self::Error>;
}

#[async_trait::async_trait]
impl RequestBuilderExt2 for RequestBuilder {
  type Error = Error;

  async fn send_checked(self) -> Result<Response, Self::Error> {
    let response = self.send().await?;
    let status = response.status();

    if status.is_client_error() || status.is_server_error() {
      let error = response.json::<PoEApiError>().await?;

      return Err(Error::PoEApiError {
        error: error.error,
        error_description: error.error_description,
      });
    }

    Ok(response)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test() {
    let builder = PoEApiConfigBuilder::create_empty()
      .client_id("client")
      .version("0.0.0")
      .client_builder(|builder| builder.connection_verbose(true))
      .contact_email("email@email.com")
      .build();

    assert!(builder.is_ok());

    if let Ok(config) = builder {
      assert_eq!(config.client_id, "client");
      assert_eq!(config.version, "0.0.0");
      assert_eq!(config.contact_email, "email@email.com");
    }
  }
}
