use std::collections::HashSet;

use poise::serenity_prelude::UserId;
use secrecy::SecretString;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use tracing::Level;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database: Database,
    #[serde(default)]
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

impl Default for Tracing {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

fn default_log_level() -> Level {
    match cfg!(debug_assertions) {
        true => Level::DEBUG,
        false => Level::WARN,
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct Discord {
    pub token: SecretString,
    #[serde(default = "default_prefix")]
    pub prefix: String,
    #[serde_as(as = "HashSet<DisplayFromStr>")]
    // rename to `owners` because underscores dont work in env variables
    #[serde(default, rename = "owners")]
    pub bot_owners: HashSet<UserId>,
}

fn default_prefix() -> String {
    "?".to_string()
}
