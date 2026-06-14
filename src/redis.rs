use std::time::Duration;

use fred::{
    clients::Client,
    interfaces::{ClientLike, KeysInterface},
    types::{
        Builder, Expiration,
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

pub async fn put_url_key(
    client: &Client,
    short_code: &str,
    long_url: &str,
    ttl: i64,
) -> Result<(), fred::error::Error> {
    let expiration = Expiration::EX(ttl);
    client
        .set::<String, _, _>(short_code, long_url, Some(expiration), None, false)
        .await?;
    Ok(())
}

pub async fn get_url_key(client: &Client, short_code: &str) -> Result<String, fred::error::Error> {
    let long_url: String = client.get(short_code).await?;

    Ok(long_url)
}

pub async fn put_rate_limit_key(
    client: &Client,
    ip_addr: &str,
    action: &str,
) -> Result<i64, fred::error::Error> {
    let key = format!("ratelimit:{}:{}", ip_addr, action);

    let value = client.incr::<i64, &String>(&key).await?;

    if value == 1 {
        client.expire::<bool, _>(&key, 60, None).await?;
    }

    Ok(value)
}
