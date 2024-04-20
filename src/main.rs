use crate::errors::SplitterError;
use twba_backup_config::prelude::Config;
use twba_backup_config::Conf;
use prelude::*;

pub mod client;
pub mod errors;
pub mod prelude;
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_env_filter(
            "sea_orm=warn,sea_orm_migration=warn,sqlx=warn,twba_splitter=trace,twba_local_db=warn,other=warn",
        )
        .init();
    info!("Hello, world!");

    run().await?;

    info!("Bye");
    Ok(())
}
async fn run() -> Result<()> {
    let conf = Conf::builder()
        .env()
        .file("./settings.toml")
        .file(shellexpand::tilde("~/twba/config.toml").into_owned())
        .load()
        .map_err(|e| SplitterError::LoadConfig(e.into()))?;

    let db = twba_local_db::open_database(Some(&conf.db_url)).await?;
    twba_local_db::migrate_db(&db).await?;

    let client = client::SplitterClient::new(conf, db);
    client.split_videos().await?;
    Ok(())
}
