-- `user`: The user who used the `invite`
-- `inviter`: The user who created the `invite`
-- `invite`: The code of the invite (the part after `discord.gg/`)
-- `guild`: The guild this invite belongs to
-- `used_at`: time the user joined the guild
CREATE TABLE invited_members(
    "user" TEXT NOT NULL,
    "inviter" TEXT NOT NULL,
    "invite" TEXT NOT NULL,
    "guild" TEXT NOT NULL,
    "used_at" TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY("user", "guild")
)