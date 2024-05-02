use crate::client::utils::ffmpeg::run_ffmpeg_concat;
use crate::errors::SplitterError;
use crate::prelude::*;
use anyhow::Context;
use chrono::Duration;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::instrument;

/// Converts a duration to a string that is usable for example in an ffmpeg command
///
/// Example:
///
/// ```
/// use chrono::Duration;
/// let duration: Duration = Duration::seconds(20);
/// let s = downloader::duration_to_string(&duration);
/// assert_eq!(s, "00:00:20");
/// ```
pub fn duration_to_string(duration: &Duration) -> String {
    trace!("duration to string for duration: {:?}", duration);
    let seconds = duration.num_seconds();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
#[instrument(skip(input_parts, duration_cap),
    fields(input_parts_amount = input_parts.parts.len(),
        total_duration = input_parts.total_duration.num_seconds(),
        duration_cap = duration_cap.num_seconds()))]
pub(super) async fn join_last_parts_if_needed(
    mut input_parts: PlaylistInfo,
    base_folder: &Path,
    duration_cap: Duration,
) -> Result<Vec<PathBuf>> {
    info!("joining last parts if needed");
    let last_part = input_parts.last_part();
    let second_last_part = input_parts.second_last_part();
    if let Some(last_part) = last_part {
        if let Some(second_last_path) = second_last_part {
            let joined_duration = last_part.duration + second_last_path.duration;
            if joined_duration <= duration_cap {
                //join together

                join_last_two_parts(&mut input_parts, base_folder).await?;
                info!("joined last two parts together");
            } else {
                info!("last two parts are too long to join together");
            }
        } else {
            info!("there is only one part, so we can't join anything");
        }
    } else {
        warn!("there are no parts, so we can't join anything");
    }

    input_parts
        .parts
        .iter()
        .map(|part| Ok(base_folder.join(&part.path)))
        .collect()
}

async fn join_last_two_parts(input_parts: &mut PlaylistInfo, base_folder: &Path) -> Result<()> {
    let last_part = input_parts
        .parts
        .pop()
        .ok_or(SplitterError::JoinRequiresAtLeastTwoParts)?;
    let second_last_part = input_parts
        .parts
        .last_mut()
        .ok_or(SplitterError::JoinRequiresAtLeastTwoParts)?;
    second_last_part.duration += last_part.duration;
    let second_last_part_path = combine_path_as_string(base_folder, &second_last_part.path)?;
    let last_part_path = combine_path_as_string(base_folder, &last_part.path)?;
    let join_txt_path = base_folder.join("join.txt");
    let join_out_tmp_path = base_folder.join("join_out_tmp.mp4");
    tokio::fs::write(
        &join_txt_path,
        format!(
            "file '{}'\nfile '{}'",
            second_last_part_path, last_part_path
        ),
    )
    .await
    .map_err(SplitterError::Write)?;

    run_ffmpeg_concat(
        join_txt_path
            .to_str()
            .ok_or_else(|| SplitterError::PathToString(join_txt_path.clone()))?
            .to_string(),
        join_out_tmp_path
            .to_str()
            .ok_or_else(|| SplitterError::PathToString(join_out_tmp_path.clone()))?
            .to_string(),
    )
    .await?;
    debug!(
        "removing files: {:?}, {:?}, {:?}",
        second_last_part.path, last_part.path, join_txt_path
    );
    tokio::fs::remove_file(last_part.path)
        .await
        .map_err(SplitterError::Write)?;
    tokio::fs::remove_file(&second_last_part.path)
        .await
        .map_err(SplitterError::Write)?;
    tokio::fs::remove_file(join_txt_path)
        .await
        .map_err(SplitterError::Write)?;
    debug!(
        "renaming file: {:?} to {:?}",
        join_out_tmp_path, second_last_part.path
    );
    tokio::fs::rename(join_out_tmp_path, &second_last_part.path)
        .await
        .map_err(SplitterError::Write)?;
    Ok(())
}

pub(crate) async fn get_playlist_info(playlist_path: &PathBuf) -> Result<PlaylistInfo> {
    let mut total_duration = Duration::zero();
    let mut parts: Vec<PartInfo> = vec![];

    let lines = tokio::fs::read_to_string(playlist_path)
        .await
        .map_err(SplitterError::Read)?;

    let mut last_duration = None;
    for line in lines.lines() {
        if line.starts_with("#EXTINF:") {
            let time_str = line
                .strip_prefix("#EXTINF:")
                .context("could not strip prefix")
                .map_err(SplitterError::PlaylistParse)?;
            let time_str = time_str.split(',').next().unwrap_or(time_str);
            let time_str = time_str.trim();
            let duration = Duration::milliseconds(
                (1000.0
                    * time_str
                        .parse::<f64>()
                        .context("could not parse the part duration")
                        .map_err(SplitterError::PlaylistParse)?) as u64 as i64,
            );
            last_duration = Some(duration);
            total_duration += duration;
        } else if line.starts_with("#EXT-X-ENDLIST") {
            break;
        } else if line.starts_with("#EXT") {
            trace!("unknown line in playlist: {}", line);
            continue;
        } else if let Some(duration) = last_duration {
            let path = PathBuf::from(line.trim().to_string());
            parts.push(PartInfo { duration, path });
            last_duration = None;
        }
    }
    if parts.is_empty() {
        return Err(SplitterError::PlaylistEmpty);
    }

    Ok(PlaylistInfo {
        total_duration,
        parts,
    })
}
impl PlaylistInfo {
    pub(crate) fn last_part(&self) -> Option<&PartInfo> {
        self.parts.last()
    }
    pub(crate) fn second_last_part(&self) -> Option<&PartInfo> {
        if self.parts.len() < 2 {
            return None;
        }
        self.parts.get(self.parts.len() - 2)
    }
}
#[derive(Debug)]
pub(crate) struct PlaylistInfo {
    pub total_duration: Duration,
    pub parts: Vec<PartInfo>,
}
#[derive(Debug)]
pub(crate) struct PartInfo {
    pub duration: Duration,
    pub path: PathBuf,
}

/// joins two paths together, canonicalizes them and returns them as a string
fn combine_path_as_string(base: &Path, path: &Path) -> Result<String> {
    let path = base.join(path);
    let path = path.canonicalize().map_err(SplitterError::Canonicalize)?;
    let path = path
        .to_str()
        .ok_or_else(|| SplitterError::PathToString(path.clone()))?
        .to_string();
    Ok(path)
}

pub mod ffmpeg;
