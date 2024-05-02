use std::path::PathBuf;
use tokio::io;

#[derive(Debug, thiserror::Error)]
pub enum SplitterError {
    #[error("Could not load config")]
    LoadConfig(#[source] anyhow::Error),

    #[error("Some error with the database")]
    OpenDatabase(#[from] twba_local_db::re_exports::sea_orm::DbErr),

    #[error("File or Folder not found or invalid: {0:?}")]
    NotFound(PathBuf),
    #[error("Input File was not a valid input: {0:?}")]
    InvalidInputFile(PathBuf),

    #[error("Could not create folder: {0:?}")]
    CreateFolder(#[source] io::Error),
    #[error("Could not read from filesystem: {0:?}")]
    Read(#[source] io::Error),
    #[error("Could not write to filesystem: {0:?}")]
    Write(String, #[source] io::Error),

    #[error("Path could not be canonicalized: {0:?}")]
    Canonicalize(#[source] io::Error),
    #[error("Could not convert path to string: {0:?}")]
    PathToString(PathBuf),

    #[error("Something went wrong during the ffmpeg command")]
    FfmpegCommand(#[source] io::Error),

    #[error("Could not parse the playlist")]
    PlaylistParse(#[source] anyhow::Error),
    #[error("Playlist was empty/did not contain any parts")]
    PlaylistEmpty,

    #[error("Joining two parts requires at least two parts in the list")]
    JoinRequiresAtLeastTwoParts,
}
