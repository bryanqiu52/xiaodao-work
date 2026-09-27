//! GitHub 热榜：拉 search API，内存缓存 10 分钟。
//!
//! **为什么请求走 Rust 而不是前端 fetch**：
//!   - GitHub API 对无 UA 的请求直接 403，Rust 侧统一带上 UA；
//!   - 没有浏览器 CORS 那套变数，失败原因能原样带回界面显示；
//!   - 匿名限额 60 次/小时 —— 结果缓存 10 分钟，怎么点都不会把限额点爆。

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// 缓存有效期。10 分钟一次，一小时最多 6 次，离 60 次的限额很远
const TTL: Duration = Duration::from_secs(10 * 60);

/// 单次请求超时：15 秒。GitHub search 偶尔慢，再长用户就该以为卡死了
const TIMEOUT: Duration = Duration::from_secs(15);

static CACHE: Mutex<Option<CacheEntry>> = Mutex::new(None);

struct CacheEntry {
    /// 缓存键：语言 + 时间范围 + 计算出的日期（日期变了缓存自动失效）
    key: String,
    items: Vec<GithubRepo>,
    fetched_at: Instant,
    /// 显示用的时间标签（HH:mm）
    label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GithubRepo {
    pub id: i64,
    /// "owner/repo"
    pub full_name: String,
    pub html_url: String,
    pub description: String,
    pub language: String,
    pub stargazers_count: u64,
    /// 只取日期部分（YYYY-MM-DD），原始是 ISO 时间戳
    pub pushed_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrendingResp {
    pub items: Vec<GithubRepo>,
    /// "HH:mm"，界面显示"更新于"
    pub fetched_at: String,
}

#[derive(Deserialize)]
struct SearchResp {
    items: Vec<ApiRepo>,
}

#[derive(Deserialize)]
struct ApiRepo {
    id: i64,
    full_name: String,
    html_url: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    language: Option<String>,
    stargazers_count: u64,
    #[serde(default)]
    pushed_at: String,
}

pub async fn trending(lang: &str, since: &str) -> Result<TrendingResp, String> {
    // since → 天数 → **本地**日期。不能用 UTC：中国时区晚上 8 点后 UTC 日期会偏一天，
    // `created:>` 的窗口就多算/少算一整天 —— 参考实现踩过的坑
    let days = match since {
        "7d" => 7,
        "90d" => 90,
        _ => 30,
    };
    let date = (chrono::Local::now() - chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string();
    let key = format!("{}|{}|{}", lang.trim().to_lowercase(), since, date);

    // 命中缓存直接回，不发请求
    if let Ok(guard) = CACHE.lock() {
        if let Some(c) = guard.as_ref() {
            if c.key == key && c.fetched_at.elapsed() < TTL {
                log::info!("[热榜] 命中缓存（{}）", key);
                return Ok(TrendingResp {
                    items: c.items.clone(),
                    fetched_at: c.label.clone(),
                });
            }
        }
    }

    let q = if lang.trim().is_empty() {
        format!("created:>{}", date)
    } else {
        format!("created:>{} language:{}", date, lang.trim())
    };

    let client = reqwest::Client::builder()
        // GitHub API 对无 UA 请求一律 403
        .user_agent("xiaodao-work")
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("初始化网络失败：{}", e))?;

    let resp = client
        .get("https://api.github.com/search/repositories")
        .query(&[
            ("q", q.as_str()),
            ("sort", "stars"),
            ("order", "desc"),
            ("per_page", "20"),
        ])
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("网络请求失败：{}", e))?;

    let status = resp.status();
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        // 403 在 GitHub 这儿多半是限流（匿名 60 次/小时），给个能懂的说法
        log::warn!("[热榜] 被限流（HTTP {}）", status.as_u16());
        return Err("请求太频繁，GitHub 限流了，过一会儿再试".to_string());
    }
    if !status.is_success() {
        return Err(format!("请求失败（HTTP {}）", status.as_u16()));
    }

    let body: SearchResp = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败：{}", e))?;

    let items: Vec<GithubRepo> = body
        .items
        .into_iter()
        .map(|r| GithubRepo {
            id: r.id,
            full_name: r.full_name,
            html_url: r.html_url,
            description: r.description.unwrap_or_default(),
            language: r.language.unwrap_or_default(),
            stargazers_count: r.stargazers_count,
            pushed_at: r.pushed_at.chars().take(10).collect(),
        })
        .collect();

    let label = chrono::Local::now().format("%H:%M").to_string();
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(CacheEntry {
            key,
            items: items.clone(),
            fetched_at: Instant::now(),
            label: label.clone(),
        });
    }
    log::info!("[热榜] 已拉取 {} 条（lang={} since={}）", items.len(), lang, since);
    Ok(TrendingResp { items, fetched_at: label })
}

/// 用系统浏览器打开仓库页。
///
/// **只放行 GitHub 域的 https 链接**：数据来自 API 风险不大，但"打开"这个动作
/// 的语义是交给系统默认程序，不能让它变成"打开任意 URL"的后门。
pub fn open_repo(url: &str) -> Result<(), String> {
    let ok = url.starts_with("https://github.com/")
        || url.starts_with("https://gist.github.com/")
        || url.starts_with("https://raw.githubusercontent.com/");
    if !ok {
        log::warn!("[热榜] 拒绝非 GitHub 链接: {}", url);
        return Err("只允许打开 GitHub 的链接".to_string());
    }
    opener::open(url).map_err(|e| format!("打开失败：{}", e))
}
