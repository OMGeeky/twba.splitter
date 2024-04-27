use crate::errors::SplitterError;
use crate::prelude::*;
use chrono::Duration;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs;
use twba_backup_config::Conf;
use twba_local_db::prelude::{Status, Videos, VideosColumn, VideosModel};
use twba_local_db::re_exports::sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, QuerySelect,
};

mod utils;
use utils::ffmpeg::run_ffmpeg_split;

pub struct SplitterClient {
    conf: Conf,
    db: DatabaseConnection,
}

impl SplitterClient {
    pub fn new(conf: Conf, db: DatabaseConnection) -> Self {
        Self { conf, db }
    }
}

impl SplitterClient {
    #[tracing::instrument(skip(self))]
    async fn split_video(&self, video: VideosModel) -> Result<()> {
        //
        let id = video.id.to_string();
        let mut video = video.into_active_model();
        video.status = ActiveValue::Set(Status::Splitting);
        video.clone().update(&self.db).await?;
        let result = self.inner_split_video(&id).await;

        match result {
            Ok(count) => {
                info!("Split video with id: {} into {} parts", id, count);
                video.status = ActiveValue::Set(Status::Split);
                video.part_count = ActiveValue::Set(count as i32);
                video.clone().update(&self.db).await?;
            }
            Err(err) => {
                error!(
                    "Could not split video with id: {} because of err: {:?}",
                    id, err
                );
                video.status = ActiveValue::Set(Status::SplitFailed);
                video.clone().update(&self.db).await?;
                return Err(err);
            }
        }
        Ok(())
    }
    async fn inner_split_video(&self, id: &str) -> Result<usize> {
        let base_path = Path::new(&self.conf.download_folder_path);
        let input_path = base_path.join(format!("{}.mp4", id));
        let output_folder_path = base_path.join(&id);

        info!("Splitting video with id: {}", id);
        verify_paths(base_path, &input_path, &output_folder_path).await?;
        let output_path_pattern = output_folder_path.join("%03d.mp4");
        let output_path_pattern = output_path_pattern
            .to_str()
            .ok_or_else(|| SplitterError::PathToString(output_path_pattern.clone()))?
            .to_string();

        let split_playlist_path = output_folder_path.join("output.m3u8");
        debug!("output_path_pattern: {}", output_path_pattern);
        let duration_soft_cap = Duration::minutes(
            self.conf
                .google
                .youtube
                .default_video_length_minutes_soft_cap,
        );
        let duration_hard_cap = Duration::minutes(
            self.conf
                .google
                .youtube
                .default_video_length_minutes_hard_cap,
        );
        //todo: get a user specific soft and hard cap
        info!("splitting video at path: {:?}", input_path);
        let start_time = Instant::now();
        run_ffmpeg_split(
            &input_path,
            &output_path_pattern,
            &split_playlist_path,
            &duration_soft_cap,
        )
        .await?;

        let duration = Instant::now().duration_since(start_time);
        info!("FFMPEG-Splitting took: {:?}", duration);
        let split_info = utils::get_playlist_info(&split_playlist_path).await?;
        tokio::fs::remove_file(&split_playlist_path)
            .await
            .map_err(SplitterError::Write)?;
        trace!(
            "total duration: {} in {} parts",
            split_info.total_duration.to_string(),
            split_info.parts.len()
        );
        let paths =
            utils::join_last_parts_if_needed(split_info, &output_folder_path, duration_hard_cap)
                .await?;

        debug!("removing original file: {:?}", input_path);
        tokio::fs::remove_file(&input_path)
            .await
            .map_err(SplitterError::Write)?;

        let duration = Instant::now().duration_since(start_time);
        info!("Done Splitting. Whole operation took: {:?}", duration);
        debug!("paths: {:?}", paths);
        Ok(paths.len())
    }

    #[tracing::instrument(skip(self))]
    pub async fn split_videos(&self) -> Result<()> {
        info!("Splitting videos");
        let videos = Videos::find()
            .filter(VideosColumn::Status.eq(Status::Downloaded))
            .limit(self.conf.max_items_to_process)
            .all(&self.db)
            .await?;

        for video in videos {
            info!("Splitting video: {:?}", video);
            let id = video.id;
            let success = self.split_video(video).await;
            if let Err(err) = success {
                error!(
                    "Could not split video with id: {} because of err: {:?}",
                    id, err
                );
            } else {
                info!("Split video with id: {}", id);
            }
        }

        info!("Finished splitting videos");
        Ok(())
    }
}

async fn verify_paths(
    base_path: &Path,
    input_path: &Path,
    output_folder_path: &PathBuf,
) -> Result<()> {
    if !base_path.exists() || !input_path.exists() {
        return Err(SplitterError::NotFound(input_path.to_path_buf()));
    }
    if !input_path.is_file() {
        return Err(SplitterError::InvalidInputFile(input_path.to_path_buf()));
    }
    fs::create_dir_all(&output_folder_path)
        .await
        .map_err(SplitterError::CreateFolder)?;
    Ok(())
}
