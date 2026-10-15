use crate::{Error, Result};
use concat_project::{
    Command, Editor,
    commands::{ClipMove, NewMedia, TrimEdge},
};
use serde_json::json;

fn failure(message: impl Into<String>) -> Error {
    Error::new("editor_error", message)
}

pub fn append_media(
    trial: &mut Editor,
    commands: &mut Vec<Command>,
    media: NewMedia,
    start: f64,
    duration: f64,
    source_start: f64,
    place: bool,
    mute: bool,
) -> Result<Option<String>> {
    let existing = trial
        .project()
        .media
        .iter()
        .find(|item| item.path == media.path)
        .map(|item| item.id.clone());
    let add = Command::AddMedia { item: media };
    let outcome = trial
        .apply(add.clone())
        .map_err(|error| failure(error.to_string()))?;
    commands.push(add);
    let media_id = outcome
        .created_id
        .or(existing)
        .ok_or_else(|| failure("无法导入媒体"))?;
    if !place {
        return Ok(None);
    }
    let add = Command::AddClipAtFirstFree { media_id, start };
    let clip_id = trial
        .apply(add.clone())
        .map_err(|error| failure(error.to_string()))?
        .created_id
        .ok_or_else(|| failure("无法放置片段"))?;
    commands.push(add);
    if source_start > 0.0 {
        let command = Command::TrimClip {
            clip_id: clip_id.clone(),
            edge: TrimEdge::Start,
            delta: source_start,
            ripple: false,
        };
        trial
            .apply(command.clone())
            .map_err(|error| failure(error.to_string()))?;
        commands.push(command);
        let track_id = trial
            .project()
            .active()
            .clip(&clip_id)
            .ok_or_else(|| failure("片段丢失"))?
            .track_id
            .clone();
        let command = Command::MoveClips {
            moves: vec![ClipMove {
                clip_id: clip_id.clone(),
                start,
                track_id,
            }],
        };
        trial
            .apply(command.clone())
            .map_err(|error| failure(error.to_string()))?;
        commands.push(command);
    }
    let current = trial
        .project()
        .active()
        .clip(&clip_id)
        .ok_or_else(|| failure("片段丢失"))?
        .duration;
    let command = Command::TrimClip {
        clip_id: clip_id.clone(),
        edge: TrimEdge::End,
        delta: duration - current,
        ripple: false,
    };
    trial
        .apply(command.clone())
        .map_err(|error| failure(error.to_string()))?;
    commands.push(command);
    let command: Command =
        serde_json::from_value(json!({"op":"updateClip","clipId":clip_id,"patch":{"muted":mute}}))?;
    trial
        .apply(command.clone())
        .map_err(|error| failure(error.to_string()))?;
    commands.push(command);
    Ok(Some(clip_id))
}
