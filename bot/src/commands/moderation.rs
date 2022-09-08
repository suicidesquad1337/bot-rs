use poise::serenity_prelude::{Color, UserId};

use crate::{Context, Result};

/// Moderate stuff
#[command(
    slash_command,
    guild_only,
    required_permissions = "BAN_MEMBERS",
    required_bot_permissions = "BAN_MEMBERS"
)]
pub async fn hackban(
    ctx: Context<'_>,
    #[description = "The member you want to ban"] user: UserId,
    reason: Option<String>,
) -> Result<()> {
    if let Some(ref reason) = reason {
        ctx.guild()
            .unwrap()
            .ban_with_reason(&ctx.discord().http, user, 0, reason)
            .await?;
    } else {
        ctx.guild()
            .unwrap()
            .ban(&ctx.discord().http, user, 0)
            .await?;
    }
    ctx.send(|b| {
        b.ephemeral(true);
        b.embed(|e| {
            e.color(Color::DARK_GREEN);
            e.title("Banned 🚫");
            if let Some(reason) = reason {
                e.description(format!(
                    "User `{}` got banned for reason `{}`",
                    user, reason
                ));
            } else {
                e.description(format!("User `{}` got banned", user));
            }
            e
        });
        b
    })
    .await?;
    Ok(())
}
