//! Invite tracking

use std::{collections::HashMap, iter::once};

use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude::{
    Context, Guild, GuildId, InviteCreateEvent, InviteDeleteEvent, RichInvite, TypeMapKey,
    UnavailableGuild, UserId,
};
use tokio::sync::RwLock;
use tracing::Level;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Invite {
    /// When the invite was created
    pub created_at: DateTime<Utc>,
    /// The point in time until this invite is valid
    pub max_age: Option<DateTime<Utc>>,
    pub max_uses: Option<u64>,
    pub temporary: bool,
    pub uses: u64,
    pub inviter: UserId,
}

impl From<RichInvite> for Invite {
    fn from(v: RichInvite) -> Self {
        let created_at = *v.created_at;
        Self {
            // If an invite does not expire, the max age is 0
            max_age: if v.max_age != 0 {
                Some(
                    created_at
                        + Duration::from_std(std::time::Duration::from_secs(v.max_age)).unwrap(),
                )
            } else {
                None
            },
            created_at,
            // If an invite max_use is 0, it is permanent and has no limited
            max_uses: if v.max_uses != 0 {
                Some(v.max_uses)
            } else {
                None
            },
            temporary: v.temporary,
            uses: v.uses,
            inviter: v.inviter.unwrap().id,
        }
    }
}

impl From<InviteCreateEvent> for Invite {
    fn from(v: InviteCreateEvent) -> Self {
        let created_at = Utc::now();
        Self {
            // TODO: ensure that this assumption is correct: If an invite does not expire, the max
            // age is 0
            max_age: if v.max_age != 0 {
                Some(
                    created_at
                        + Duration::from_std(std::time::Duration::from_secs(v.max_age)).unwrap(),
                )
            } else {
                None
            },
            created_at,
            // If an invite max_use is 0, it is permanent and has no limited
            max_uses: if v.max_uses != 0 {
                Some(v.max_uses)
            } else {
                None
            },
            temporary: v.temporary,
            // the value returned for this will always be 0
            uses: 0,
            inviter: v.inviter.unwrap().id,
        }
    }
}

#[derive(Debug)]
pub struct InviteStore;

impl TypeMapKey for InviteStore {
    type Value = RwLock<HashMap<GuildId, HashMap<String, Invite>>>;
}

impl InviteStore {
    #[instrument(skip_all, name = "add_invites_created_guild", level = "debug")]
    pub async fn invite_guild_created(ctx: Context, guild: &Guild) {
        match guild.invites(ctx.http).await {
            Ok(invites) => {
                event!(
                    Level::DEBUG,
                    guild = guild.id.0,
                    invites = invites.len(),
                    "loaded {} invite(s) for guild {}",
                    invites.len(),
                    guild.id.0
                );
                let invites = invites
                    .into_iter()
                    .map(|i| (i.code.clone(), Invite::from(i)));
                let writer = ctx.data.write().await;

                // this can be thought of as the get_or_insert
                match writer.get::<InviteStore>() {
                    Some(store) => {
                        store.write().await.insert(guild.id, invites.collect());
                    }
                    None => {
                        // "inital" store with the invites that we loaded for this guild
                        // This branch should be called exactly once for the first guild the bot
                        // joined
                        let store: RwLock<HashMap<GuildId, HashMap<String, Invite>>> = RwLock::new(
                            HashMap::from_iter(once((guild.id, HashMap::from_iter(invites)))),
                        );
                        ctx.data.write().await.insert::<InviteStore>(store);
                    }
                };
            }
            Err(e) => {
                event!(Level::WARN, error = ?e, "failed to load invites for guild {}: {}", guild.id.0, e);
            }
        }
    }

    #[instrument(skip_all, name = "remove_invites_deleted_guild")]
    pub async fn invite_guild_deleted(ctx: &Context, guild: &UnavailableGuild) {
        ctx.data
            .read()
            .await
            .get::<InviteStore>()
            .unwrap()
            .write()
            .await
            .remove(&guild.id);
    }

    //#[instrument(skip_all, name = "add_invite", level = "debug")]
    pub async fn invite_created(ctx: &Context, invite: InviteCreateEvent) {
        let guild = invite.guild_id.expect("guild is set");
        let code = invite.code.clone();
        event!(
            Level::DEBUG,
            code,
            guild = guild.0,
            "new invite created in guild {}",
            guild.0,
        );

        ctx.data
            .read()
            .await
            .get::<InviteStore>()
            .unwrap()
            .write()
            .await
            .get_mut(&invite.guild_id.unwrap())
            .unwrap()
            .insert(code, Invite::from(invite));
    }

    #[instrument(skip_all, name = "remove_invite", level = "debug")]
    pub async fn invite_deleted(ctx: &Context, invite: &InviteDeleteEvent) {
        event!(
            Level::DEBUG,
            invite = invite.code,
            "invite deleted: {}",
            invite.code
        );
        ctx.data
            .read()
            .await
            .get::<InviteStore>()
            .unwrap()
            .write()
            .await
            .get_mut(&invite.guild_id.unwrap())
            .expect("guild has been inserted on ready or in guild create event")
            .remove(&invite.code);
    }
}
