use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DanmakuFormat {
    #[default]
    Xml,
    Html,
    Srt,
    Ass,
}

impl DanmakuFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Xml => "xml",
            Self::Html => "html",
            Self::Srt => "srt",
            Self::Ass => "ass",
        }
    }
}

#[derive(Deserialize)]
struct XmlComments {
    #[serde(rename = "d", default)]
    comments: Vec<XmlComment>,
}

#[derive(Deserialize)]
struct XmlComment {
    #[serde(rename = "@p")]
    properties: String,
    #[serde(rename = "$text", default)]
    text: String,
}

struct Comment {
    start: u64,
    mode: u8,
    color: u32,
    text: String,
}

/// Bilibili timestamps are seconds relative to the video, not wall-clock time.
pub fn xml_to_subtitle(xml: &str, format: DanmakuFormat) -> crate::BdlResult<String> {
    let invalid = |message: String| crate::BdlError::Planning { message };
    let parsed: XmlComments = quick_xml::de::from_str(xml.trim_start_matches('\u{feff}'))
        .map_err(|error| invalid(format!("弹幕 XML 无效：{error}")))?;
    let mut comments = Vec::new();
    for item in parsed.comments {
        let fields: Vec<_> = item.properties.split(',').collect();
        let seconds = fields.first().and_then(|s| s.parse::<f64>().ok());
        let mode = fields.get(1).and_then(|s| s.parse::<u8>().ok());
        let color = fields.get(3).and_then(|s| s.parse::<u32>().ok());
        let (Some(seconds), Some(mode), Some(color)) = (seconds, mode, color) else {
            continue;
        };
        // Advanced/script/BAS comments contain structured payloads, not plain text.
        if !seconds.is_finite()
            || !(0.0..=315_360_000.0).contains(&seconds)
            || !matches!(mode, 1..=6)
            || color > 0xffffff
        {
            continue;
        }
        let text = item
            .text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            comments.push(Comment {
                start: (seconds * 1000.0).round() as u64,
                mode,
                color,
                text,
            });
        }
    }
    comments.sort_by_key(|comment| comment.start);
    if comments.is_empty() {
        return Err(invalid("弹幕没有可转换的普通文字内容。".into()));
    }
    match format {
        DanmakuFormat::Srt => {
            let mut output = String::new();
            for (index, comment) in comments.iter().enumerate() {
                // Plain SRT has no motion or positioning; show each comment for four seconds.
                let text = comment.text.replace('<', "＜").replace('>', "＞");
                output.push_str(&format!(
                    "{}\n{} --> {}\n{text}\n\n",
                    index + 1,
                    timestamp(comment.start, false),
                    timestamp(comment.start + 4000, false)
                ));
            }
            Ok(output)
        }
        DanmakuFormat::Ass => Ok(comments_to_ass(&comments)),
        _ => Err(invalid("弹幕字幕格式需要选择 SRT 或 ASS。".into())),
    }
}

fn timestamp(ms: u64, ass: bool) -> String {
    if ass {
        format!(
            "{}:{:02}:{:02}.{:02}",
            ms / 3_600_000,
            ms / 60_000 % 60,
            ms / 1000 % 60,
            ms % 1000 / 10
        )
    } else {
        format!(
            "{:02}:{:02}:{:02},{:03}",
            ms / 3_600_000,
            ms / 60_000 % 60,
            ms / 1000 % 60,
            ms % 1000
        )
    }
}

fn comments_to_ass(comments: &[Comment]) -> String {
    let mut output = String::from(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\nWrapStyle: 2\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,40,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,7,20,20,20,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );
    let mut lanes = [[0_u64; 20]; 3];
    for comment in comments {
        let group = match comment.mode {
            4 => 2,
            5 => 1,
            _ => 0,
        };
        let lane = lanes[group]
            .iter()
            .position(|end| *end <= comment.start)
            .unwrap_or_else(|| {
                lanes[group]
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, end)| *end)
                    .unwrap()
                    .0
            });
        let duration = if group == 0 { 8000 } else { 4000 };
        lanes[group][lane] = comment.start + duration;
        let y = if group == 2 {
            1040 - lane * 50
        } else {
            20 + lane * 50
        };
        let text = comment
            .text
            .replace('\\', "\\\u{2060}")
            .replace('{', "｛")
            .replace('}', "｝")
            .replace('\n', "\\N");
        let width = comment
            .text
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
            * 40;
        let position = match comment.mode {
            4 => format!("\\an2\\pos(960,{y})"),
            5 => format!("\\an8\\pos(960,{y})"),
            6 => format!("\\an7\\move(-{width},{y},1920,{y})"),
            _ => format!("\\an7\\move(1920,{y},-{width},{y})"),
        };
        let rgb = comment.color;
        let bgr = ((rgb & 0xff) << 16) | (rgb & 0xff00) | (rgb >> 16);
        output.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{{{position}\\c&H{bgr:06X}&}}{text}\n",
            timestamp(comment.start, true),
            timestamp(comment.start + duration, true)
        ));
    }
    output
}

/// Standalone, offline viewer. Untrusted XML stays data, never executable HTML.
pub fn xml_to_html(xml: &str) -> Result<String, serde_json::Error> {
    let data = serde_json::to_string(xml)?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    Ok(include_str!("danmaku-viewer.html").replace("__BDL_XML__", &data))
}

#[cfg(test)]
mod tests {
    use super::{DanmakuFormat, xml_to_subtitle};

    #[test]
    fn srt_preserves_video_times_sorts_and_decodes_xml_entities() {
        let xml = r#"<i><d p="3661.234,1,25,16777215">后面</d><d p="1.5,5,25,16711680">中文 &amp; &lt;b&gt;&#10;第二行</d><d p="0,7,25,0">[特殊数据]</d><d p="NaN,1,25,0">无效</d></i>"#;
        let srt = xml_to_subtitle(xml, DanmakuFormat::Srt).unwrap();
        assert_eq!(
            srt,
            "1\n00:00:01,500 --> 00:00:05,500\n中文 & ＜b＞\n第二行\n\n2\n01:01:01,234 --> 01:01:05,234\n后面\n\n"
        );
    }

    #[test]
    fn ass_preserves_color_direction_and_fixed_positions_without_text_injection() {
        let xml = r#"<i><d p="1.25,1,25,16711680">{\pos(1,1)}中文\N</d><d p="1.25,6,25,65280">反向</d><d p="2,5,25,16777215">顶部</d><d p="2,4,25,16777215">底部</d></i>"#;
        let ass = xml_to_subtitle(xml, DanmakuFormat::Ass).unwrap();
        assert!(ass.contains("Dialogue: 0,0:00:01.25,0:00:09.25"));
        assert!(ass.contains("\\move(1920,20,-"));
        assert!(ass.contains("\\move(-80,70,1920,70)"));
        assert!(ass.contains("\\c&H0000FF&"));
        assert!(ass.contains("\\an8\\pos(960,20)"));
        assert!(ass.contains("\\an2\\pos(960,1040)"));
        assert!(!ass.contains("{\\pos(1,1)}"));
        assert!(ass.contains("｛\\\u{2060}pos(1,1)｝"));
    }

    #[test]
    fn invalid_or_empty_comments_fail_instead_of_publishing_fake_subtitles() {
        for xml in ["<i><d", "<i></i>", r#"<i><d p="-1,1,25,0">无效</d></i>"#] {
            assert!(xml_to_subtitle(xml, DanmakuFormat::Srt).is_err());
        }
    }

    #[test]
    fn embeds_untrusted_xml_as_data_and_keeps_viewer_offline() {
        let html =
            super::xml_to_html("<i><d p=\"1,1,25,16777215\">&lt;/script&gt;中文</d></i>").unwrap();
        assert!(html.contains("\\u003ci\\u003e"));
        assert!(!html.contains("<i>"));
        assert!(!html.contains("https://"));
        assert!(html.contains("textContent"));
    }
}
