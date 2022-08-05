use chrono::{Duration, Utc};
use comfy_table::{presets::NOTHING, Cells, Table};
use poise::serenity_prelude::{Member, UserId};

use crate::{
    invite::{Invite, InviteStore},
    Context, Result,
};

#[command(
    slash_command,
    guild_only,
    required_bot_permissions = "MANAGE_GUILD",
    subcommands("list", "list_of")
)]
pub async fn invite(_: Context<'_>) -> Result<()> {
    Ok(())
}

// List invites created by you
#[command(slash_command, ephemeral, required_bot_permissions = "MANAGE_GUILD")]
pub async fn list(ctx: Context<'_>) -> Result<()> {
    list_invites(ctx, ctx.author().id).await
}

/// List invites created by a member of this guild
#[command(
    slash_command,
    ephemeral,
    required_bot_permissions = "MANAGE_GUILD",
    required_permissions = "MANAGE_GUILD",
    rename = "list-of"
)]
pub async fn list_of(ctx: Context<'_>, member: Member) -> Result<()> {
    list_invites(ctx, member.user.id).await
}

/// Helper function to list the invites of an user
pub async fn list_invites(ctx: Context<'_>, user: UserId) -> Result<()> {
    let reader = ctx.discord().data.read().await;
    let reader = reader
        .get::<InviteStore>()
        .ok_or_else(|| anyhow!("invite store missing"))?
        .read()
        .await;
    let invites = reader
        .get(&ctx.guild_id().unwrap())
        .ok_or_else(|| anyhow!("no invites stored for this guild"))?
        .iter()
        .filter(|(_, invite)| invite.inviter == user);
    let table = generate_invite_table(invites, user != ctx.author().id, user);
    ctx.send(|reply| {
        reply.content(table);
        reply.ephemeral(true)
    })
    .await?;
    Ok(())
}

fn generate_invite_table<'a>(
    invites: impl Iterator<Item = (&'a String, &'a Invite)>,
    display_inviter: bool,
    member: UserId,
) -> String {
    let mut invites = invites.peekable();

    if invites.peek().is_none() {
        return match display_inviter {
            true => format!("<@{}> has no invites in this guild.", member),
            false => "You have no invites in this guild.".to_string(),
        };
    }
    let mut table = Table::new();
    let mut headers = vec!["Invite", "Uses", "Expires"];
    if display_inviter {
        headers.insert(0, "Inviter");
    }

    table.set_header(headers);
    table.load_preset(NOTHING);
    invites
        .map::<Cells, _>(|(code, meta)| {
            let uses = format!(
                "{}/{}",
                meta.uses,
                meta.max_uses
                    .map(|u| u.to_string())
                    // \u{221E} = infinity symbol
                    .unwrap_or_else(|| "\u{221E}".to_string())
            );
            let expires = match meta.max_age {
                Some(t) => {
                    let mut expires_in = t - Utc::now();
                    let days = match expires_in.num_days() {
                        0 => None,
                        x => {
                            expires_in = expires_in - Duration::days(x);
                            Some(format!("{}d", x))
                        }
                    };
                    let hours = match expires_in.num_hours() {
                        0 => None,
                        x => {
                            expires_in = expires_in - Duration::hours(x);
                            Some(format!("{}h", x))
                        }
                    };
                    let minutes = match expires_in.num_minutes() {
                        0 => None,
                        x => {
                            expires_in = expires_in - Duration::minutes(x);
                            Some(format!("{}min", x))
                        }
                    };
                    let seconds = match expires_in.num_seconds() {
                        0 => None,
                        x => Some(format!("{}s", x)),
                    };

                    format!(
                        "{} {} {} {}",
                        days.unwrap_or_default(),
                        hours.unwrap_or_default(),
                        minutes.unwrap_or_default(),
                        seconds.unwrap_or_default(),
                    )
                    .trim()
                    .to_string()
                }
                None => "\u{221E}".to_string(),
            };
            match display_inviter {
                true => [meta.inviter.0.to_string(), code.to_string(), uses, expires]
                    .into_iter()
                    .into(),
                false => [code.to_string(), uses, expires].into_iter().into(),
            }
        })
        .for_each(|row| {
            table.add_row(row);
        });
    format!("```\n{}\n```", table)
}
