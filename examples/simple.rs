use poe_api::{PoEApi, PoEApiConfigBuilder};

#[tokio::main]
async fn main() {
  dotenvy::dotenv().unwrap();

  let config = PoEApiConfigBuilder::default()
    .client_id(dotenvy::var("CLIENT_ID").unwrap())
    .version(dotenvy::var("VERSION").unwrap())
    .contact_email(dotenvy::var("CONTACT_EMAIL").unwrap())
    .access_token(dotenvy::var("ACCESS_TOKEN").unwrap())
    .build()
    .unwrap();

  let api = PoEApi::new(config).unwrap();

  let profile = api.get_profile().await.unwrap();

  dbg!(profile);
}
