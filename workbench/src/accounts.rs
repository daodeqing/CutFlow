use crate::{Error, Result};
use bdl_core::account::{self, AccountLibraryFolderKind, QrLoginStatus};
use serde_json::{Value, json};
use std::path::Path;

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Runtime::new().map_err(Error::from)
}

fn account_error(error: impl std::fmt::Display) -> Error {
    Error::new(
        "account_error",
        account::redact_sensitive(&error.to_string()),
    )
}

pub fn start() -> Result<Value> {
    let session = runtime()?
        .block_on(account::start_qr_login())
        .map_err(account_error)?;
    Ok(
        json!({"sessionId": session.qrcode_key, "svg": session.qr_image_svg,
              "expiresIn": session.expires_in_seconds, "status": "waiting"}),
    )
}

pub fn poll(root: &Path, session: &str) -> Result<Value> {
    if session.len() > 256 || session.is_empty() {
        return Err(Error::new("invalid_request", "二维码会话无效"));
    }
    let outcome = runtime()?
        .block_on(account::poll_qr_login(session))
        .map_err(account_error)?;
    if outcome.status == QrLoginStatus::Confirmed {
        let cookie = outcome
            .cookie_header
            .ok_or_else(|| Error::new("account_error", "未收到登录凭据"))?;
        cutflow_download::credentials::save(root, &cookie).map_err(account_error)?;
        return Ok(json!({"status":"confirmed", "account": status(root)?}));
    }
    Ok(json!({"status": outcome.status, "message": account::redact_sensitive(&outcome.message)}))
}

pub fn status(root: &Path) -> Result<Value> {
    let Some(cookie) = cutflow_download::credentials::load(root).map_err(account_error)? else {
        return Ok(json!({"loggedIn":false}));
    };
    runtime()?.block_on(async {
        let summary = account::verify_cookie_session(&cookie)
            .await
            .map_err(account_error)?;
        if !summary.logged_in {
            return Ok(json!({"loggedIn":false,"expired":true}));
        }
        let client = bpi_rs::BpiClient::builder()
            .cookie(&cookie)
            .build()
            .map_err(account_error)?;
        let membership = client
            .login()
            .vip_info()
            .await
            .ok()
            .map(|value| {
                if value.vip_status == 1 {
                    "有效大会员"
                } else {
                    "非大会员"
                }
            })
            .unwrap_or("查询失败，状态未知");
        Ok(
            json!({"loggedIn":true,"name":summary.name,"mid":summary.mid,
                  "avatarUrl":summary.avatar_url,"membership":membership}),
        )
    })
}

pub fn favorites(root: &Path, page: u32, folder: Option<u64>) -> Result<Value> {
    let cookie = cutflow_download::credentials::load(root)
        .map_err(account_error)?
        .ok_or_else(|| Error::new("login_required", "请先扫码登录"))?;
    runtime()?.block_on(async {
        let summary = account::verify_cookie_session(&cookie).await.map_err(account_error)?;
        let mid = summary.mid.and_then(|mid| mid.parse().ok())
            .ok_or_else(|| Error::new("login_required", "登录已失效，请重新扫码"))?;
        if let Some(folder)=folder {
            let client=bpi_rs::BpiClient::builder().cookie(&cookie).build().map_err(account_error)?;
            let params=bpi_rs::fav::FavListDetailParams::new(bpi_rs::ids::MediaId::new(folder).map_err(account_error)?)
                .page(page.max(1)).map_err(account_error)?;
            let listing=client.fav().list_detail(params).await.map_err(account_error)?;
            return Ok(json!({"folderId":folder,"page":page.max(1),"hasMore":listing.has_more,
                "items":listing.medias.iter().filter_map(|item|{
                    let bvid=item.bvid.as_ref().or(item.bv_id.as_ref())?;
                    Some(json!({"title":item.title,"bvid":bvid,"source_url":format!("https://www.bilibili.com/video/{bvid}"),"duration":item.duration}))
                }).collect::<Vec<_>>()}));
        }
        let listing = account::account_library_page(&cookie, mid, AccountLibraryFolderKind::CreatedFavorite, page.max(1), 20)
            .await.map_err(account_error)?;
        serde_json::to_value(listing).map_err(Error::from)
    })
}
