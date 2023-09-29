use poe_api::{PoEApi, PoEApiConfigBuilder};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  dotenvy::dotenv().unwrap();

  let client_id = dotenvy::var("CLIENT_ID")?;
  let version = dotenvy::var("VERSION")?;
  let contact_email = dotenvy::var("CONTACT_EMAIL")?;
  let access_token = dotenvy::var("ACCESS_TOKEN")?;

  let config = PoEApiConfigBuilder::default()
    .client_id(client_id)
    .version(version)
    .contact_email(contact_email)
    .build()
    .unwrap();

  let api = PoEApi::new(config).unwrap();

  let profile = api.get_profile(&access_token).await.unwrap();

  dbg!(profile);

  Ok(())
}
