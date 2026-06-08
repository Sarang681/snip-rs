use std::time::Duration;

use fred::{
    clients::Client,
    interfaces::{ClientLike, KeysInterface},
    types::{
        Builder,
        config::{Config, TcpConfig},
    },
};

pub async fn get_redis_client(url: &str) -> Result<Client, fred::error::Error> {
    let config = Config::from_url(url)?;

    let client = Builder::from_config(config)
        .with_connection_config(|config| {
            config.connection_timeout = Duration::from_secs(5);
            config.tcp = TcpConfig {
                nodelay: Some(true),
                ..Default::default()
            };
        })
        .build()?;

    client.init().await?;

    Ok(client)
}

pub async fn put_key(
    client: &Client,
    short_code: &str,
    long_url: &str,
) -> Result<(), fred::error::Error> {
    client
        .set::<String, _, _>(short_code, long_url, None, None, false)
        .await?;
    Ok(())
}

pub async fn get_key(client: &Client, short_code: &str) -> Result<String, fred::error::Error> {
    let long_url: String = client.get(short_code).await?;

    Ok(long_url)
}
