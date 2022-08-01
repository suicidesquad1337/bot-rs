use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use poise::{
    serenity_prelude::GatewayIntents, Framework, FrameworkOptions, PrefixFrameworkOptions,
};
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use tracing::Instrument;
use tracing_log::LogTracer;
use tracing_subscriber::FmtSubscriber;

mod config;
mod data;
mod register;

#[doc(inline)]
pub use {config::Config, data::Data};

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate poise;

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
    sqlx::migrate!("../migrations/")
        .run(&pool)
        .instrument(info_span!("database_migrations"))
        .await
        .map_err(|e| anyhow!("Failed to run database migrations: {}", e))?;

    // bot setup
    let framework = Framework::builder()
        .options(FrameworkOptions {
            prefix_options: PrefixFrameworkOptions {
                prefix: Some((&config.discord.prefix).into()),
                ..Default::default()
            },
            commands: vec![register::register()],
            ..Default::default()
        })
        .token(config.discord.token.expose_secret())
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .user_data_setup(move |_, _, _| Box::pin(async move { Ok(Data::new(pool, config)) }));

    framework.run().await?;

    Ok(())
}
