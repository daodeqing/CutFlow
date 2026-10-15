pub mod bangumi;
pub mod cheese;
pub mod collection;
pub mod favorite;
pub mod http_guard;
pub mod pacing;
pub mod paged;
pub mod source;
pub mod uploader;
pub mod video;

use async_trait::async_trait;
use chrono::{FixedOffset, TimeZone};

use crate::BdlResult;
use crate::input::ClassifiedInput;
use crate::model::NormalizedSourceTree;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResolveOptions {
    pub fetch_streams: bool,
}

#[async_trait]
pub trait Resolver {
    async fn resolve(
        &self,
        input: ClassifiedInput,
        options: ResolveOptions,
    ) -> BdlResult<NormalizedSourceTree>;
}

pub(crate) fn bilibili_publish_date(timestamp: u64) -> Option<String> {
    if timestamp == 0 {
        return None;
    }
    let timestamp = i64::try_from(timestamp).ok()?;
    let china_standard_time = FixedOffset::east_opt(8 * 60 * 60)?;
    china_standard_time
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|value| value.date_naive().to_string())
}

/// Prefer CDN URLs supplied by the play API while retaining every original URL
/// as a fallback. Each URL carries its own query parameters and signature.
pub(crate) fn media_urls(base_url: String, backup_urls: Vec<String>) -> Vec<String> {
    let mut urls = Vec::with_capacity(1 + backup_urls.len());
    for url in std::iter::once(base_url).chain(backup_urls) {
        if !url.trim().is_empty() && !urls.contains(&url) {
            urls.push(url);
        }
    }
    urls.sort_by_key(|url| is_pcdn_host(url));
    urls
}

fn is_pcdn_host(raw_url: &str) -> bool {
    let Ok(url) = url::Url::parse(raw_url) else {
        return false;
    };
    url.host_str()
        .is_some_and(|host| host.contains("mcdn") || host.contains("szbdyd"))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{bilibili_publish_date, media_urls};

    #[test]
    fn media_urls_prefer_complete_cdn_backups_and_keep_pcdn_fallbacks() {
        let pcdn = "https://xy.mcdn.bilivideo.cn:4483/upgcxcode/video.m4s?os=mcdn&upsig=pcdn";
        let another_pcdn = "https://xy.v1d.szbdyd.com:8997/upgcxcode/video.m4s?upsig=other";
        let cdn = "https://upos-sz-mirrorcos.bilivideo.com/upgcxcode/video.m4s?os=cos&upsig=cdn";
        let cdn_with_keyword_in_query =
            "https://upos-sz-mirrorali.bilivideo.com/upgcxcode/video.m4s?note=mcdn";

        assert_eq!(
            media_urls(
                pcdn.to_owned(),
                vec![
                    another_pcdn.into(),
                    cdn.into(),
                    cdn_with_keyword_in_query.into(),
                    cdn.into()
                ]
            ),
            vec![cdn, cdn_with_keyword_in_query, pcdn, another_pcdn]
        );
        assert_eq!(
            media_urls(pcdn.into(), vec![another_pcdn.into()]),
            vec![pcdn, another_pcdn]
        );
    }

    #[test]
    fn publication_date_uses_bilibili_china_date_at_utc_boundary() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 1, 1, 16, 30, 0)
            .single()
            .expect("fixture timestamp")
            .timestamp() as u64;

        assert_eq!(
            bilibili_publish_date(timestamp).as_deref(),
            Some("2026-01-02")
        );
        assert_eq!(bilibili_publish_date(0), None);
    }
}
