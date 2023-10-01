use std::net::{SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use derivative::Derivative;
use derive_builder::Builder;
use derive_more::From;
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{
  AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope, TokenUrl,
};
use reqwest::redirect::Policy;
use reqwest::{Client, ClientBuilder, Method, RequestBuilder, Response, Url};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const API_URL: &str = "https://api.pathofexile.com";
pub const AUTH_URL: &str = "https://www.pathofexile.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://www.pathofexile.com/oauth/token";
pub const CLOSE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Better PoE Redirect</title>
</head>
<body>
You can close this page.
</body>
</html>"#;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoEApiError {
  error: String,
  error_description: String,
}

#[derive(Debug, Error)]
pub enum Error {
  #[error(transparent)]
  IoError(#[from] std::io::Error),
  #[error(transparent)]
  ReqwestError(#[from] reqwest::Error),
  #[error(transparent)]
  ReqwestInvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
  #[error(transparent)]
  UrlParseError(#[from] url::ParseError),
  #[error(transparent)]
  UninitializedFieldError(#[from] derive_builder::UninitializedFieldError),
  #[error("{error}: {error_description}")]
  PoEApiError {
    error: String,
    error_description: String,
  },
  #[error("Failed to get authorization code")]
  FailedToGetAuthorizationCode,
  #[error("{0}")]
  Custom(String),
  #[error(transparent)]
  BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct PoEApiConfig {
  client_id: String,
  version: String,
  contact_email: String,
  #[builder(setter(custom))]
  redirect_url: Url,
  #[builder(setter(custom))]
  redirect_addr: Vec<SocketAddr>,
  #[builder(default = "CLOSE_HTML.to_string()")]
  close_html: String,
}

impl PoEApiConfigBuilder {
  pub fn redirect_url<T>(self, value: T) -> Result<Self, T::Error>
  where
    T: TryInto<Url>,
  {
    Ok(Self {
      redirect_url: Some(value.try_into()?),
      ..self
    })
  }

  pub fn redirect_addr<T>(self, value: T) -> Result<Self, std::io::Error>
  where
    T: ToSocketAddrs,
  {
    Ok(Self {
      redirect_addr: Some(value.to_socket_addrs()?.collect()),
      ..self
    })
  }
}

#[derive(Debug, Copy, Clone, From)]
pub enum PoEApiScope {
  Account(PoEApiAccountScope),
}

impl FromStr for PoEApiScope {
  type Err = Error;

  fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
    PoEApiAccountScope::from_str(s).map(Into::<Self>::into)
  }
}

impl ToString for PoEApiScope {
  fn to_string(&self) -> String {
    match self {
      PoEApiScope::Account(scope) => scope.to_string(),
    }
  }
}

#[derive(Debug, Copy, Clone)]
pub enum PoEApiAccountScope {
  Profile,
  Leagues,
  Stashes,
  Characters,
  LeagueAccounts,
  ItemFilter,
}

impl PoEApiAccountScope {
  pub const fn name(&'_ self) -> &'static str {
    match self {
      Self::Profile => "account:profile",
      Self::Leagues => "account:leagues",
      Self::Stashes => "account:stashes",
      Self::Characters => "account:characters",
      Self::LeagueAccounts => "account:league_accounts",
      Self::ItemFilter => "account:item_filter",
    }
  }
}

impl FromStr for PoEApiAccountScope {
  type Err = Error;

  fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
    match s.trim_start_matches("account::") {
      "profile" => Ok(Self::Profile),
      "leagues" => Ok(Self::Leagues),
      "stashes" => Ok(Self::Stashes),
      "characters" => Ok(Self::Characters),
      "league_accounts" => Ok(Self::LeagueAccounts),
      "item_filter" => Ok(Self::ItemFilter),
      _ => Err(Error::Custom("Failed to parse account scope".into())),
    }
  }
}

impl AsRef<str> for PoEApiAccountScope {
  fn as_ref(&self) -> &'static str {
    self.name()
  }
}

#[allow(clippy::from_over_into)]
impl Into<&str> for PoEApiAccountScope {
  fn into(self) -> &'static str {
    self.name()
  }
}

impl ToString for PoEApiAccountScope {
  fn to_string(&self) -> String {
    self.as_ref().into()
  }
}

#[derive(Debug)]
pub struct PoEApi {
  config: PoEApiConfig,
  server: AuthorizationServer,
  client: Client,
}

impl PoEApi {
  pub fn new(config: PoEApiConfig) -> Result<Self> {
    Self::new_with_builder(config, Client::builder())
  }

  pub fn new_with_builder(config: PoEApiConfig, builder: ClientBuilder) -> Result<Self> {
    let PoEApiConfig {
      client_id,
      version,
      contact_email,
      redirect_addr,
      ..
    } = &config;

    let user_agent = format!("OAuth {client_id}/{version} (contact: {contact_email})");

    let server = AuthorizationServer::new(redirect_addr.as_slice())?;
    let client = builder
      .user_agent(user_agent)
      .redirect(Policy::none())
      .build()?;

    Ok(Self {
      config,
      server,
      client,
    })
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

  pub fn close_authorization_server(&self) {
    self.server.close_handle.store(true, Ordering::SeqCst)
  }

  pub async fn get_token<S, F, T, R>(&self, scopes: S, callback: F) -> Result<BasicTokenResponse>
  where
    S::Item: Into<PoEApiScope>,
    S: IntoIterator,
    F: FnOnce(Url) -> R,
    R: Into<Result<T, Error>>,
  {
    let client = BasicClient::new(
      ClientId::new(self.config.client_id.to_string()),
      None,
      AuthUrl::new(AUTH_URL.into())?,
      Some(TokenUrl::new(TOKEN_URL.to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(self.config.redirect_url.to_string())?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf_token) = client
      .authorize_url(CsrfToken::new_random)
      .add_scopes(
        scopes //
          .into_iter()
          .map(|s| s.into().to_string())
          .map(Scope::new),
      )
      .set_pkce_challenge(pkce_challenge)
      .url();

    callback(auth_url).into()?;

    let authorization_code = self
      .server
      .get_authorization_code(&self.config, csrf_token)?;

    client
      .exchange_code(AuthorizationCode::new(authorization_code))
      .set_pkce_verifier(pkce_verifier)
      .request_async(oauth2::reqwest::async_http_client)
      .await
      .map_err(|_| Error::FailedToGetAuthorizationCode)
  }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub(crate) struct AuthorizationServer {
  #[derivative(Debug = "ignore")]
  server: tiny_http::Server,
  close_handle: AtomicBool,
}

impl AuthorizationServer {
  pub fn new(addr: impl ToSocketAddrs) -> Result<Self> {
    Ok(Self {
      server: tiny_http::Server::http(addr)?,
      close_handle: AtomicBool::new(false),
    })
  }

  pub fn get_authorization_code(&self, config: &PoEApiConfig, state: CsrfToken) -> Result<String> {
    while let Ok(request) = self.server.recv_timeout(Duration::from_millis(100)) {
      let Some(request) = request else {
        if self.close_handle.load(Ordering::SeqCst) {
          return Err(Error::FailedToGetAuthorizationCode);
        }

        continue;
      };

      let url = format!("{}{}", config.redirect_url, request.url());
      let url = Url::parse(&url)?;
      let query = url.query_pairs().collect::<Vec<_>>();

      let query_state = query
        .iter()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value);

      let query_code = query
        .iter()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value);

      match (query_state, query_code) {
        (Some(query_state), Some(query_code)) if state.secret() == query_state => {
          request.respond(tiny_http::Response::from_data(config.close_html.as_bytes()))?;

          return Ok(query_code.to_string());
        }
        _ => {
          request
            .respond(tiny_http::Response::from_string("Invalid Query").with_status_code(422))?;
        }
      }
    }

    Err(Error::FailedToGetAuthorizationCode)
  }
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
