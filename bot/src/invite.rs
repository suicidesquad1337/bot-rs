//! Invite tracking

use std::{
    collections::{HashMap, HashSet},
    iter::once,
};

use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude::{
    Context, Guild, GuildId, InviteCreateEvent, InviteDeleteEvent, Member, RichInvite, TypeMapKey,
    UnavailableGuild, UserId,
};
use tokio::sync::RwLock;
use tracing::{Instrument, Level};

use crate::Data;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct Invite {
    /// When the invite was created
    pub created_at: DateTime<Utc>,
    /// The point in time until this invite is valid
    pub max_age: Option<DateTime<Utc>>,
    pub max_uses: Option<u64>,
    pub temporary: bool,
    pub uses: u64,
    pub guild: GuildId,
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
            guild: v.guild.unwrap().id,
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
            guild: v.guild_id.unwrap(),
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

// FIXME: listen for permission update in case the bot didnt have the permission
// to see invites but now has
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
                let mut writer = ctx.data.write().await;

                // this can be thought of as the get_or_insert
                match writer.get::<InviteStore>() {
                    Some(store) => {
                        store.write().await.insert(guild.id, invites.collect());
                    }
                    None => {
                        // "inital" store with the invites that we loaded for this guild
                        // This branch should be called exactly once for the first guild the bot
                        // joined
                        let store: HashMap<GuildId, HashMap<String, Invite>> =
                            HashMap::from_iter(once((guild.id, HashMap::from_iter(invites))));
                        writer.insert::<InviteStore>(RwLock::new(store));
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
        event!(
            Level::INFO,
            "Guild deleted, deleting all invites from guild {}",
            guild.id.0
        );
        ctx.data
            .read()
            .await
            .get::<InviteStore>()
            .unwrap()
            .write()
            .await
            .remove(&guild.id);
    }

    #[instrument(skip_all, name = "add_invite", level = "debug")]
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
            Level::INFO,
            invite = invite.code,
            guild = invite.guild_id.unwrap().0,
            "invite {} deleted in guild {}",
            invite.code,
            invite.guild_id.unwrap().0
        );
        ctx.data
            .read()
            .await
            .get::<InviteStore>()
            .unwrap()
            .write()
            .await
            .get_mut(&invite.guild_id.unwrap())
            .expect("guild has been inserted in guild create event")
            .remove(&invite.code);
    }
}

pub struct InviteTracker;

impl InviteTracker {
    #[instrument(skip_all, name = "guild_member_add", level = "debug")]
    pub async fn on_join(ctx: Context, member: Member) {
        event!(
            Level::INFO,
            member = member.user.id.0,
            guild = member.guild_id.0,
            "member {} is trying to join guild {}",
            member.user.id.0,
            member.guild_id.0
        );

        if member.user.bot {
            // bots dont use normal invite sto join
            event!(
                Level::INFO,
                member = member.user.id.0,
                "member {} is a bot, invite not tracked",
                member.user.id.0
            );
            return;
        }

        // this event is kinda hacky, because it assumes that 1) the join event
        // is always called BEFORE invite_delete (required for one time use
        // invites) and 2) that the InviteStore has the state of the
        // invites before the join and not after.

        let reader = ctx
            .data
            .read()
            .instrument(info_span!("read_invites_wait"))
            .await;
        let store = reader.get::<InviteStore>().unwrap();

        // wrtier only used at the end to update the local cache
        let mut store_reader = store.write().await;
        let old_state_store = store_reader.get_mut(&member.guild_id).unwrap();
        let current_state_store: HashMap<String, Invite> = match member
            .guild_id
            .invites(ctx.http.clone())
            .await
        {
            Ok(invites) => {
                event!(
                    Level::DEBUG,
                    "loaded {} invites for comparison",
                    invites.len()
                );
                invites
                    .into_iter()
                    .map(|i| (i.code.clone(), i.into()))
                    .collect()
            }
            Err(e) => {
                event!(Level::WARN, error = ?e, "cannot fetch invites for comparison: {}", e);
                match member
                    .kick_with_reason(
                        ctx.http,
                        &format!("Cannot fetch invites for comparison: {}", e),
                    )
                    .await
                {
                    Ok(_) => (),
                    Err(e) => {
                        event!(Level::WARN, member = member.user.id.0, error = ?e, "cannot kick member {}: {}", member.user.id.0, e)
                    }
                }
                return;
            }
        };

        let old_state: HashSet<_> = old_state_store.keys().into_iter().collect();
        let current_state: HashSet<_> = current_state_store.keys().into_iter().collect();

        // if were is one invite missing, we know it is a invite which had only one use
        // left
        let (member, invite, code) = if current_state.len() == (old_state.len() - 1) {
            // in this case, we just need to find the invite that is in
            // old_state but not in current_state
            debug_assert!(old_state.is_superset(&current_state));
            let invite_code = old_state.difference(&current_state).next().unwrap();
            let invite = old_state_store.get(*invite_code).unwrap();
            event!(
                Level::INFO,
                inviter = invite.inviter.0,
                member = member.user.id.0,
                guild = member.guild_id.0,
                "member {} joined on guild {} invited by {}",
                member.user.id.0,
                member.guild_id.0,
                invite.inviter.0
            );
            // This will be handled by the invite delete event
            // old_state_store.remove(&code);
            (member, invite, *invite_code)
        } else {
            // all element in old_state are still present in current_state BUT their
            // metadata (which is only stored in the two *_store variants) is different.
            // However, since these two HashSets are only over the keys, they are identical.
            debug_assert!(old_state == current_state);

            match current_state_store
                .iter()
                .find(|(code, new)| match old_state_store.get(*code) {
                    Some(old_meta) => (old_meta.uses + 1) == new.uses,
                    None => false,
                }) {
                Some((code, new_invite)) => {
                    // update _this_ invite in the local invite cache. This is needed, because the
                    // `use` count has changed, because this invite was used.
                    old_state_store.insert(code.to_owned(), new_invite.to_owned());
                    (member, new_invite, code)
                }
                None => {
                    event!(
                        Level::WARN,
                        member = member.user.id.0,
                        guild = member.guild_id.0,
                        "failed to associate an invite with member {} on guild {}",
                        member.user.id.0,
                        member.guild_id.0
                    );
                    match member
                        .kick_with_reason(
                            ctx.http,
                            "failed to associate an invite with this member",
                        )
                        .await
                    {
                        Ok(_) => (),
                        Err(e) => {
                            event!(Level::WARN, member = member.user.id.0, error = ?e, "cannot kick member {}: {}", member.user.id.0, e)
                        }
                    }
                    return;
                }
            }
        };

        let data = reader.get::<Data>().unwrap();
        match sqlx::query!(
            r#"
        INSERT INTO invited_members ("user", inviter, invite, guild)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT("user", "guild") DO UPDATE
        SET inviter = EXCLUDED.inviter,
        invite = EXCLUDED.invite,
        used_at = EXCLUDED.used_at
        "#,
            member.user.id.0.to_string(),
            invite.inviter.0.to_string(),
            code,
            invite.guild.0.to_string(),
        )
        .execute(&data.pool)
        .await
        {
            Ok(_) => event!(
                Level::INFO,
                "{} is the inviter of {} on guild {}",
                invite.inviter.0,
                member.user.id.0,
                invite.guild.0
            ),
            Err(e) => {
                event!(Level::ERROR, error = ?e, "failed to insert into database: {}", e);
                match member
                    .kick_with_reason(ctx.http, "error inserting user in database")
                    .await
                {
                    Ok(_) => (),
                    Err(e) => event!(
                        Level::WARN,
                        error = ?e,
                        member = member.user.id.0,
                        guild = member.guild_id.0,
                        "failed to kick member {} from guild {}: {}",
                        member.user.id.0,
                        member.guild_id.0,
                        e
                    ),
                }
            }
        }

        event!(Level::DEBUG, "invite_store at end: {:#?}", old_state_store);
    }
}
