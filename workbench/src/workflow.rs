use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskSpec {
    pub version: u32,
    pub project_path: String,
    pub operation_id: String,
    pub expected_revision: u64,
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub max_generation_calls: u32,
    #[serde(default = "voice")]
    pub voice: i32,
    #[serde(default = "enabled")]
    pub subtitles: bool,
    #[serde(default = "stop")]
    pub stop_at: String,
    #[serde(default)]
    pub output: Option<concat_project::model::VideoSettings>,
    #[serde(default)]
    pub bgm: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scene {
    pub id: String,
    pub source: Source,
    #[serde(default)]
    pub narration: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub source_start: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Source {
    Local {
        path: String,
    },
    Generate {
        model: String,
        prompt: String,
        media_kind: String,
        #[serde(default)]
        references: Vec<String>,
    },
    Bilibili {
        url: String,
        rights: String,
        #[serde(default = "part")]
        parts: String,
        #[serde(default = "quality")]
        quality: String,
    },
}

fn voice() -> i32 {
    45
}
fn enabled() -> bool {
    true
}
fn stop() -> String {
    "draft".into()
}
fn part() -> String {
    "1".into()
}
fn quality() -> String {
    "120".into()
}

impl TaskSpec {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.stop_at != "draft"
            || self.scenes.is_empty()
            || self.scenes.len() > 64
        {
            return Err(Error::new(
                "invalid_request",
                "TaskSpec v1 需要 1–64 个分镜，stopAt 必须是 draft",
            ));
        }
        if self.project_path.trim().is_empty()
            || self.operation_id.trim().is_empty()
            || self.operation_id.len() > 96
        {
            return Err(Error::new(
                "invalid_request",
                "工程路径和 operationId 必须有效，流程操作 ID 最长 96 字节",
            ));
        }
        if let Some(video) = self.output {
            if video.width == 0
                || video.height == 0
                || video.width > 16384
                || video.height > 16384
                || video.rate_num <= 0
                || video.rate_den <= 0
            {
                return Err(Error::new("invalid_request", "输出尺寸或帧率无效"));
            }
        }
        let mut ids = std::collections::HashSet::new();
        let mut generations = 0;
        for scene in &self.scenes {
            if scene.id.trim().is_empty()
                || !ids.insert(&scene.id)
                || !scene.source_start.is_finite()
                || scene.source_start < 0.0
                || scene.duration.is_some_and(|duration| {
                    !duration.is_finite() || duration <= 0.0 || duration > 3600.0
                })
            {
                return Err(Error::new(
                    "invalid_request",
                    "分镜 ID 必须唯一，时长与入点必须有效",
                ));
            }
            if let Source::Generate {
                media_kind,
                model,
                prompt,
                ..
            } = &scene.source
            {
                generations += 1;
                if !["image", "video"].contains(&media_kind.as_str())
                    || model.trim().is_empty()
                    || prompt.trim().is_empty()
                {
                    return Err(Error::new(
                        "invalid_request",
                        "生成分镜缺少有效模型、提示词或 mediaKind",
                    ));
                }
            }
            if let Source::Bilibili {
                url,
                rights,
                parts,
                quality,
            } = &scene.source
            {
                cutflow_download::validate_input(url)
                    .map_err(|message| Error::new("invalid_request", message))?;
                cutflow_download::validate_parts(parts)
                    .map_err(|message| Error::new("invalid_request", message))?;
                if rights.trim().is_empty()
                    || parts.contains(',')
                    || !["120", "80", "best"].contains(&quality.as_str())
                {
                    return Err(Error::new(
                        "invalid_request",
                        "B站分镜需授权说明、单个分 P 和有效画质",
                    ));
                }
            }
        }
        if generations > self.max_generation_calls {
            return Err(Error::new(
                "budget_exceeded",
                "分镜生成次数超过 maxGenerationCalls",
            ));
        }
        Ok(())
    }
}

pub fn sentences(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    for sentence in text.split_inclusive(['。', '！', '？', '!', '?', '\n']) {
        let mut chunk = String::new();
        for character in sentence.trim().chars() {
            chunk.push(character);
            if chunk.chars().count() >= 60 {
                result.push(std::mem::take(&mut chunk));
            }
        }
        if !chunk.is_empty() {
            result.push(chunk);
        }
    }
    result
}
