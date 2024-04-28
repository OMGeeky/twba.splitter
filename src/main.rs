use crate::errors::SplitterError;
use prelude::*;

pub mod client;
pub mod errors;
pub mod prelude;
#[tokio::main]
async fn main() -> Result<()> {
    let _guard = twba_common::init_tracing("twba_splitter");
    info!("Hello, world!");

    run().await?;

    info!("Bye");
    Ok(())
}
async fn run() -> Result<()> {
    let conf = twba_backup_config::get_default_builder()
        .load()
        .map_err(|e| SplitterError::LoadConfig(e.into()))?;

    let db = twba_local_db::open_database(Some(&conf.db_url)).await?;
    twba_local_db::migrate_db(&db).await?;

    let client = client::SplitterClient::new(conf, db);
    client.split_videos().await?;
    Ok(())
}
