use oauth2::TokenResponse;
use poe_api::{PoEApi, PoEApiAccountScope, PoEApiConfigBuilder};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  dotenvy::dotenv().unwrap();

  let client_id = dotenvy::var("CLIENT_ID")?;
  let version = dotenvy::var("VERSION")?;
  let contact_email = dotenvy::var("CONTACT_EMAIL")?;

  let config = PoEApiConfigBuilder::default()
    .client_id(client_id)
    .version(version)
    .contact_email(contact_email)
    .redirect_url("http://localhost:8088")?
    .redirect_addr("127.0.0.1:8088")?
    .build()
    .unwrap();

  let api = PoEApi::new(config).unwrap();

  let scopes = [
    PoEApiAccountScope::Profile,
    PoEApiAccountScope::Leagues,
    PoEApiAccountScope::Stashes,
    PoEApiAccountScope::Characters,
  ];

  let token = api.get_token(scopes, |url| {
    println!("{url}");
    Ok(())
  }).await?;

  dbg!(token.access_token().secret());
  dbg!(token);

  Ok(())
}
