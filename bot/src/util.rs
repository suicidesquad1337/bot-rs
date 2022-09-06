use chrono::{DateTime, Utc};
use poise::serenity_prelude::{Color, Context, Message, Result, User};

#[allow(dead_code)]
pub enum Penalty {
    Timeout(DateTime<Utc>),
    Ban(Option<DateTime<Utc>>),
}

pub async fn send_sanction_notification<S>(
    ctx: &Context,
    user: &User,
    reason: S,
    penalty: Penalty,
) -> Result<Message>
where
    S: ToString,
{
    user.direct_message(ctx, |f| {
        f.add_embed(|e| {
            e.title("You've been sanctioned");
            e.color(Color::RED);
            e.description(format!(
                "Hey {username},\n
                I'm sorry to tell you, but you've been sanctioned for {reason}.
                Your penalty is {penalty}
            ",
                username = user.name,
                reason = reason.to_string(),
                penalty = gen_penalty_string(penalty)
            ));
            e.footer(|f| f.text("you probably deserved it"));
            e
        });
        f
    })
    .await
}

/// Generate a human readable penalty
///
/// Formulate the penalty in human readable words.<br>
/// This puts out a string like `timeout until <t:1543392060:R>` or `permanent
/// ban`
fn gen_penalty_string(penalty: Penalty) -> String {
    let mut sb = String::new();
    sb.push_str("a ");
    match penalty {
        Penalty::Timeout(x) => {
            sb.push_str(format!("timeout until <t:{}:R>", x.timestamp()).as_str())
        }
        Penalty::Ban(x) => {
            if let Some(time) = x {
                sb.push_str(format!("ban until <t:{}:R>", time.timestamp()).as_str())
            } else {
                sb.push_str("permanent ban");
            }
        }
    };
    sb
}
