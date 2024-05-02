use super::*;
use tracing::instrument;
#[instrument(skip(join_txt_path, join_out_path))]
pub(crate) async fn run_ffmpeg_concat(
    join_txt_path: impl Into<String>,
    join_out_path: impl Into<String>,
) -> Result<()> {
    let join_txt_path = join_txt_path.into();
    let join_out_path = join_out_path.into();

    debug!(
        "Running ffmpeg command: ffmpeg -f concat -safe 0 -i {:?} -c copy {:?}",
        join_txt_path, join_out_path
    );
    Command::new("ffmpeg")
        .args([
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            &join_txt_path,
            "-c",
            "copy",
            &join_out_path,
        ])
        .output()
        .await
        .map_err(SplitterError::FfmpegCommand)?;
    debug!("Finished running ffmpeg command");
    Ok(())
}
#[instrument(skip(input, target_duration, output_playlist, output_pattern))]
pub(crate) async fn run_ffmpeg_split(
    input: &Path,
    output_pattern: &String,
    output_playlist: &Path,
    target_duration: &Duration,
) -> Result<()> {
    let split_duration_str = duration_to_string(target_duration);
    debug!(
    "Running ffmpeg command: ffmpeg -i {:?} -c copy -map 0 -segment_time {} -reset_timestamps 1 \
     -segment_list {} -segment_list_type m3u8 -avoid_negative_ts 1 -f segment {}",
    input,
    split_duration_str,
    output_playlist.display(),
    output_pattern
);
    Command::new("ffmpeg")
        .args([
            "-i",
            input
                .to_str()
                .ok_or_else(|| SplitterError::PathToString(input.to_path_buf()))?,
            "-c",
            "copy",
            "-map",
            "0",
            "-segment_time",
            &split_duration_str,
            "-reset_timestamps",
            "1",
            "-segment_list",
            output_playlist
                .to_str()
                .ok_or_else(|| SplitterError::PathToString(output_playlist.to_path_buf()))?,
            "-segment_list_type",
            "m3u8",
            "-avoid_negative_ts",
            "1",
            "-f",
            "segment",
            output_pattern,
        ])
        .output()
        .await
        .map_err(SplitterError::FfmpegCommand)?;
    debug!("Finished running ffmpeg command");
    Ok(())
}
