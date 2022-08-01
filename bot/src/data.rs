use sqlx::PgPool;

use crate::Config;

#[derive(Debug)]
#[non_exhaustive]
pub struct Data {
    pub pool: PgPool,
    pub config: Config,
}

impl Data {
    pub const fn new(pool: PgPool, config: Config) -> Self {
        Self { pool, config }
    }
}
