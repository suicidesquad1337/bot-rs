use std::{fmt::Debug, sync::Arc};

use chrono::{Duration, Utc};
use poise::{
    dispatch_event,
    serenity_prelude::{
        Context, EventHandler, Guild, Interaction, InviteCreateEvent, InviteDeleteEvent, Member,
        Message, Ready, ShardManager, StickerFormatType, UnavailableGuild, UserId,
    },
    Event, FrameworkContext, FrameworkOptions,
};
use tokio::sync::{Mutex, RwLock};
use tracing::{Instrument, Level};

use crate::{
    invite::{InviteStore, InviteTracker},
    util::send_sanction_notification,
};

#[derive(Debug)]
pub struct GlobalEventHandler<D, E> {
    pub options: FrameworkOptions<D, E>,
    pub data: D,
    pub shard_manager: RwLock<Option<Arc<Mutex<ShardManager>>>>,
    pub whoami: RwLock<Option<UserId>>,
}

impl<D, E> GlobalEventHandler<D, E>
where
    D: Send + Sync + Debug,
    E: Send + Sync + Debug,
{
    #[instrument(skip(ctx), level = Level::DEBUG)]
    pub async fn dispatch_event(&self, ctx: Context, event: Event<'_>) {
        // building this should be cheap
        let framework = FrameworkContext {
            bot_id: self.whoami.read().await.unwrap(),
            options: &self.options,
            user_data: &self.data,
            shard_manager: &self.shard_manager.read().await.as_ref().unwrap().clone(),
        };
        dispatch_event(framework, &ctx, &event).await;
    }
}

#[async_trait]
impl<D, E> EventHandler for GlobalEventHandler<D, E>
where
    D: Send + Sync + Debug,
    E: Send + Sync + Debug,
{
    #[instrument(skip_all)]
    async fn ready(&self, ctx: Context, ready: Ready) {
        // this is only executed once at start up
        *self
            .whoami
            .write()
            .instrument(debug_span!("ready_load_self"))
            .await = Some(ready.user.id);

        event!(
            Level::INFO,
            guilds = ready.guilds.len(),
            "I'm on {} guilds!",
            ready.guilds.len(),
        );

        self.dispatch_event(
            ctx,
            Event::Ready {
                data_about_bot: ready,
            },
        )
        .instrument(debug_span!("dispatch_ready_event"))
        .await;
    }

    #[instrument(skip_all)]
    #[allow(unused_must_use)]
    async fn message(&self, ctx: Context, new_message: Message) {
        // prevent sending of default stickers
        if !new_message.is_private() {
            for sticker in &new_message.sticker_items {
                if sticker.format_type == StickerFormatType::Lottie {
                    let comms_disabled_until = Utc::now() + Duration::minutes(1);
                    send_sanction_notification(
                        &ctx,
                        &new_message.author,
                        "sending a default sticker",
                        crate::util::Penalty::Timeout(comms_disabled_until),
                    )
                    .await;
                    new_message
                        .member(&ctx)
                        .await
                        .unwrap()
                        .disable_communication_until_datetime(&ctx, comms_disabled_until.into())
                        .await;
                    new_message.delete(&ctx).await;
                }
            }
        }

        self.dispatch_event(ctx, Event::Message { new_message })
            .instrument(debug_span!("dispatch_message_event"))
            .await;
    }

    #[instrument(skip_all)]
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        self.dispatch_event(ctx, Event::InteractionCreate { interaction })
            .instrument(debug_span!("dispatch_interaction_create_event"))
            .await;
    }

    #[instrument(skip_all)]
    async fn invite_create(&self, ctx: Context, invite: InviteCreateEvent) {
        InviteStore::invite_created(&ctx, invite).await;
    }

    #[instrument(skip_all)]
    async fn guild_create(&self, ctx: Context, guild: Guild, _: bool) {
        InviteStore::invite_guild_created(ctx, &guild).await;
    }

    #[instrument(skip_all)]
    async fn guild_delete(&self, ctx: Context, guild: UnavailableGuild, _: Option<Guild>) {
        InviteStore::invite_guild_deleted(&ctx, &guild).await;
    }

    #[instrument(skip_all)]
    async fn invite_delete(&self, ctx: Context, invite: InviteDeleteEvent) {
        InviteStore::invite_deleted(&ctx, &invite).await;
    }

    #[instrument(skip_all)]
    async fn guild_member_addition(&self, ctx: Context, member: Member) {
        InviteTracker::on_join(ctx, member).await;
    }
}
