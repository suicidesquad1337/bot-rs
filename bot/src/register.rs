use crate::{Context, Result};

#[command(prefix_command)]
#[instrument(name = "register_slash_commands", level = "info")]
pub async fn register(ctx: Context<'_>) -> Result<()> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}
