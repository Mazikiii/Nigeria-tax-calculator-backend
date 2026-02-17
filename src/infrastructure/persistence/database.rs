use sqlx::postgres::{PgPool, PgPoolOptions}; // to establish connection
use std::time::Duration;

pub async fn create_pool() -> Result<PgPool, sqlx::Error> {
    // set the database url, read the env variable
    let database_url =
        std::env::var("DATABASE_URL").expect("Database Url(DATABASE_URL) must be set");

    // Then its time to connect based on that url
    let pool = PgPoolOptions::new()
        // managing amount of connections
        .max_connections(10)
        .min_connections(2)
        // i have to handle the timeline of existing connections, 3-5-10 rule
        .acquire_timeout(Duration::from_secs(5)) // Fail fast if connection is busy for more than 3s, somethings wrong
        .idle_timeout(Duration::from_secs(300)) // the connection has no interaction for 5 min, close it
        .max_lifetime(Duration::from_secs(600)) // if the connection lives for 10 mins, refresh it, because of potential issues
        // test the health of the connection before trying to connect, so you know the database is good
        .test_before_acquire(true)
        // then connect to the database
        .connect(&database_url)
        .await?;

    // test pool immediately after startup, if any issues fail fast
    sqlx::query("SELECT 1").fetch_one(&pool).await?;

    // then return the okay pool
    Ok(pool)
}

//always set up a function that monitors connection and know active, idle connections
// it costs nothing
pub struct PoolStats {
    pub active: u32,
    pub idle: usize,
    pub max: u32,
}

pub async fn get_poolstats(pool: &PgPool) -> Option<PoolStats> {
    Some(PoolStats {
        active: pool.size() as u32,
        idle: pool.num_idle(),
        max: pool.options().get_max_connections(),
    })
}
