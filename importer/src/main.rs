use anyhow::{Context, Result};
use core_db::{create_pool};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool = create_pool().await.context("Failed to connect to database")?;

    sqlx::query(include_str!("../../core/schema.sql")).execute(&pool).await?;

    Ok(())
}
