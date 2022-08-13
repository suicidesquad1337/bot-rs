use std::sync::Arc;

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use handler::GlobalEventHandler;
use poise::{serenity_prelude::GatewayIntents, FrameworkOptions, PrefixFrameworkOptions};
use secrecy::ExposeSecret;
use serenity::Client;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::RwLock;
use tracing::Instrument;
use tracing_log::LogTracer;
use tracing_subscriber::FmtSubscriber;

mod commands;
mod config;
mod data;
mod handler;
mod invite;
mod register;

#[doc(inline)]
pub use {config::Config, data::Data};

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate poise;
#[macro_use]
extern crate async_trait;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing compatibilty layer for crates that use `log`
    LogTracer::init()?;

    let config: Config = Figment::new()
        .merge(Toml::file("Bot.toml"))
        .merge(Env::prefixed("PWNHUB_BOT_").map(|k| k.as_str().replace('_', ".").into()))
        .extract()
        .map_err(|e| anyhow!("Failed to load configuration: {}", e))?;

    let subscriber = FmtSubscriber::builder()
        .with_max_level(config.tracing.level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let pool = PgPoolOptions::new()
        .connect(config.database.url.expose_secret())
        .await?;

    // run database migrations
    sqlx::migrate!("./migrations/")
        .run(&pool)
        .instrument(info_span!("database_migrations"))
        .await
        .map_err(|e| anyhow!("Failed to run database migrations: {}", e))?;

    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_INVITES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::all();

    let client = Client::builder(config.discord.token.expose_secret(), intents);

    let owners = config.discord.bot_owners.clone();
    let prefix = config.discord.prefix.clone();
    let data = Data::new(pool, config);
    let mut handler: GlobalEventHandler<Data, Error> = GlobalEventHandler {
        options: FrameworkOptions {
            prefix_options: PrefixFrameworkOptions {
                prefix: Some(prefix),
                ..Default::default()
            },
            owners,
            commands: vec![register::register(), commands::invite()],
            ..Default::default()
        },
        data: data.clone(),
        // set the shard to None since we get it later by the `client`. However, since the client
        // needs ours handler to begin with, we have to do this ugly workaround
        shard_manager: RwLock::const_new(None),
        // this is set in the Ready event
        whoami: RwLock::const_new(None),
    };

    poise::set_qualified_names(&mut handler.options.commands);

    let handler = Arc::new(handler);

    let mut client = client
        .event_handler_arc(handler.clone())
        .type_map_insert::<Data>(data)
        .await?;

    *handler.shard_manager.write().await = Some(client.shard_manager.clone());

    client.start().await?;

    Ok(())
}
