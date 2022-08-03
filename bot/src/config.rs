use std::collections::HashSet;

use poise::serenity_prelude::UserId;
use secrecy::SecretString;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use tracing::Level;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database: Database,
    pub tracing: Tracing,
    pub discord: Discord,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Database {
    pub url: SecretString,
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct Tracing {
    #[serde_as(as = "DisplayFromStr")]
    #[serde(default = "default_log_level")]
    pub level: Level,
}

fn default_log_level() -> Level {
    match cfg!(debug_asserations) {
        true => Level::DEBUG,
        false => Level::WARN,
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct Discord {
    pub token: SecretString,
    pub prefix: String,
    #[serde_as(as = "HashSet<DisplayFromStr>")]
    #[serde(default)]
    pub bot_owners: HashSet<UserId>,
}
