use futures::TryStreamExt;
use poise::serenity_prelude::{CacheHttp, Invite as SerenityInvite, Member, Permissions, UserId};
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
    if invite.is_some() && member.is_some() {
        return Err(anyhow!(
            "You can't use both `invite` and `member`. Either delete the `invite` or all invites \
             of a `member`"
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

// TODO: also list invites that aren't valid anymore but are contained in the
// database
#[instrument(skip(ctx))]
async fn autocomplete_invite(ctx: Context<'_>, partial: &str) -> Vec<String> {
    let reader = ctx.discord().data.read().await;
    let reader = reader.get::<InviteStore>().unwrap().read().await;
    let privileged = ctx
        .guild()
        .unwrap()
        .member_permissions(ctx.discord().http(), ctx.author().id)
        .await
        .unwrap_or(Permissions::empty())
        .manage_guild();

    reader
        .get(&ctx.guild_id().unwrap())
        .unwrap()
        .iter()
        .filter(move |(code, invite)| {
            // privileged/admin members are able to revoke any invite and therefore it
            // should autocomplete for all invites
            ((invite.inviter == ctx.author().id) || privileged)
                && code.to_lowercase().starts_with(&partial.to_lowercase())
        })
        .map(|(code, _)| code.to_owned())
        .collect()
}
