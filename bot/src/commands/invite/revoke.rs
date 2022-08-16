use std::collections::HashSet;

use futures::{future, stream, Stream, StreamExt, TryStreamExt};
use poise::{
    serenity_prelude::{
        AutocompleteInteraction, CacheHttp, Invite as SerenityInvite, Member, Permissions, UserId,
    },
    ApplicationCommandOrAutocompleteInteraction, ApplicationContext,
};
use sqlx::PgPool;
use tracing::{Instrument, Level};

use crate::{invite::InviteStore, Context, Result};

/// Revoke a single or all invites created by a you or an other member
#[instrument(skip(ctx))]
#[command(slash_command, ephemeral)]
pub async fn revoke(
    ctx: Context<'_>,
    #[description = "The code of the invite you want to revoke"]
    #[autocomplete = "autocomplete_invite"]
    invite: Option<String>,
    #[description = "If set to true, kick all members you used this invite to join the server"]
    kick: Option<bool>,
    #[description = "Only required if you want to revoke all invites from this member"]
    member: Option<Member>,
) -> Result<()> {
    let privileged = ctx
        .guild()
        .unwrap()
        .member_permissions(ctx.discord().http(), ctx.author().id)
        .await
        .unwrap_or(Permissions::empty())
        .manage_guild();

    if member.is_some() && !privileged {
        return Err(anyhow!(
            "You don't have the permission to revoke the invites of other members"
        )
        .into());
    }

    match member {
        Some(member) if privileged => {
            let reader = ctx.discord().data.read().await;
            let mut revoked = Vec::new();
            for (invite, _) in reader
                .get::<InviteStore>()
                .unwrap()
                .read()
                .await
                .get(&ctx.guild().unwrap().id)
                .ok_or_else(|| anyhow!("No invites stored for this guild"))?
                .iter()
                .filter(|(_, meta)| meta.inviter == member.user.id)
            {
                revoked.push(delete_invite(ctx, invite, kick.unwrap_or(false)).await?);
            }
            match revoked.len() {
                0 => ctx.say("No invites revoked.").await?,
                x => ctx.say(format!("Revoked {} invites.", x)).await?,
            };
            Ok(())
        }
        None => match invite {
            Some(invite) => {
                delete_invite(ctx, &invite, kick.unwrap_or(false))
                    .await
                    .map_err(|_| anyhow!("Failed to delete invite {}", invite))?;
                ctx.say(format!("Successfully revoked invite `{}`.", invite))
                    .await?;
                Ok(())
            }
            None => {
                // revoke all invites from this member
                // FIXME: this is complete code duplication
                let reader = ctx.discord().data.read().await;
                let mut revoked = Vec::new();
                for (invite, _) in reader
                    .get::<InviteStore>()
                    .unwrap()
                    .read()
                    .await
                    .get(&ctx.guild().unwrap().id)
                    .ok_or_else(|| anyhow!("No invites stored for this guild"))?
                    .iter()
                    .filter(|(_, meta)| meta.inviter == ctx.author().id)
                {
                    revoked.push(delete_invite(ctx, invite, kick.unwrap_or(false)).await?);
                }
                match revoked.len() {
                    0 => ctx.say("No invites revoked.").await?,
                    x => ctx.say(format!("Revoked {} invites.", x)).await?,
                };
                Ok(())
            }
        },
        _ => {
            Err(anyhow!("You don't have the permission to revoke invites of other members.").into())
        }
    }
}

#[instrument(skip(ctx))]
async fn delete_invite(ctx: Context<'_>, invite: &str, kick: bool) -> Result<String> {
    let invite = SerenityInvite::get(ctx.discord().http(), invite, false, false, None)
        .await
        .map_err(|_| anyhow!("Invalid invite."))?;

    invite
        .delete(ctx.discord().http())
        .await
        .map_err(|e| anyhow!("Cannot delete invite: {}", e))?;
    if kick {
        sqlx::query!(
                r#"SELECT "user", "used_at" FROM invited_members WHERE invite = $1"#,
                &invite.code
            )
            .fetch(&ctx.data().pool)
            .try_for_each_concurrent(None, |row| async move {
                // todo: check for independence limit
                match ctx
                    .guild()
                    .unwrap()
                    .kick_with_reason(
                        ctx.discord().http(),
                        UserId(row.user.parse().unwrap()),
                        &format!("Invite revoked by {}#{} ({})", ctx.author().name, ctx.author().discriminator, ctx.author().id),
                    )
                    .await
                {
                    Ok(_) => event!(Level::INFO, "Kicked member {}", row.user),
                    // FIXME: this will warn even for members that simply aren't in the guild anymore
                    Err(e) => event!(Level::WARN, member = row.user, error = ?e, "Failed to kick member {}: {}", row.user, e),
                }
                Ok(())
            })
            .instrument(info_span!("invite_revoke_kick_members"))
            .await?;
    }
    Ok(invite.code)
}

/// Returns all invites that are from `inviter` and were used at least once
async fn db_invites<'a>(pool: &'a PgPool, inviter: &str) -> impl Stream<Item = String> + 'a {
    sqlx::query!(
        r#"SELECT DISTINCT ON (invite) invite FROM invited_members WHERE inviter = $1"#,
        inviter,
    )
    .fetch(pool)
    .map_ok(|r| r.invite)
    // just ignore rows that returned an error
    .then(|r| future::ready(stream::iter(r.into_iter())))
    .flatten()
}
#[instrument(skip(ctx))]
async fn autocomplete_invite<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    let interaction: &AutocompleteInteraction = match ctx {
        Context::Application(ApplicationContext {
            interaction: ApplicationCommandOrAutocompleteInteraction::Autocomplete(interaction),
            ..
        }) => interaction,
        _ => unreachable!("non-autocomplete interaction in autocomplete callback"),
    };

    let privileged = ctx
        .guild()
        .unwrap()
        .member_permissions(ctx.discord().http(), ctx.author().id)
        .await
        .unwrap_or(Permissions::empty())
        .manage_guild();

    let member = match privileged {
        true => interaction
            .data
            .options
            .iter()
            .find(|parent| parent.name == "revoke")
            .and_then(|cmd| cmd.options.iter().find(|c| c.name == "member"))
            .and_then(|c| c.value.as_ref())
            .and_then(|v| v.as_str())
            .map(|v| v.to_owned())
            .unwrap_or_else(|| ctx.author().id.0.to_string()),
        false => ctx.author().id.0.to_string(),
    };

    info!(
        member = member,
        "Autocompleting invites for member {}", &member,
    );

    let reader = ctx.discord().data.read().await;

    // TODO: consider caching this since for each newly typed character (each new
    // autocomplete request), a call to the database is performed. This is (mostly)
    // needless however, since the invites probably won't change in this short time
    // frame. A crate for this would be `moka`.
    let db_invites = db_invites(&ctx.data().pool, member.as_str()).await;

    let reader = reader.get::<InviteStore>().unwrap().read().await;

    let local_invites: HashSet<String> = reader
        .get(&ctx.guild_id().unwrap())
        .unwrap()
        .iter()
        .filter(move |(_, invite)| invite.inviter.to_string() == member)
        .map(|(code, _)| code.to_owned())
        .collect();
    // This can probably be optimized somehow. Since this optimization is trivial,
    // it is left as an exercise to the reader.
    let filter = local_invites.clone();
    // first push all the local invites out and then add (old) invites from the
    // database
    stream::iter(local_invites)
        .chain(db_invites.filter(move |i| future::ready(!filter.contains(i))))
        .filter(move |i| future::ready(i.starts_with(partial)))
}
