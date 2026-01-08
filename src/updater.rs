//! 规则自动更新器
//! 通过 GitHub API 检测 KazumiRules 仓库变动并同步规则

use crate::config::CONFIG;
use crate::http_client::HTTP_CLIENT;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

/// 规则目录
const RULES_DIR: &str = "rules";
/// 存储上次 commit SHA 的文件
const LAST_COMMIT_FILE: &str = "rules/.last_commit";

/// 带代理重试的 GET 请求
async fn get_with_retry(url: &str) -> anyhow::Result<reqwest::Response> {
    // 第一次直接请求
    let result = HTTP_CLIENT
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "anime-search-api")
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => Ok(resp),
        Ok(resp) => {
            // 状态码错误，尝试代理
            let status = resp.status();
            debug!("请求失败 ({}), 尝试代理: {}", status, url);
            get_via_proxy(url).await
        }
        Err(e) => {
            // 网络错误，尝试代理
            debug!("请求失败 ({}), 尝试代理: {}", e, url);
            get_via_proxy(url).await
        }
    }
}

/// 通过代理请求
async fn get_via_proxy(url: &str) -> anyhow::Result<reqwest::Response> {
    let proxy_url = format!("{}{}", CONFIG.github_proxy, url);
    debug!("使用代理: {}", proxy_url);

    let response = HTTP_CLIENT
        .get(&proxy_url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "anime-search-api")
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("代理请求失败: HTTP {}", response.status());
    }

    Ok(response)
}

/// GitHub Commit 响应
#[derive(Debug, Deserialize)]
struct GitHubCommit {
    sha: String,
}

/// GitHub Contents 响应 (文件列表)
#[derive(Debug, Deserialize)]
struct GitHubContent {
    name: String,
    #[serde(rename = "type")]
    content_type: String,
}

/// 更新结果
#[derive(Debug, Clone, Serialize)]
pub struct UpdateResult {
    pub total: usize,
    pub updated: usize,
    pub added: usize,
    pub failed: usize,
    pub details: Vec<UpdateDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateDetail {
    pub name: String,
    pub action: String, // "added", "updated", "failed"
    pub message: String,
}

/// 检查本地是否有规则文件
pub fn has_local_rules() -> bool {
    let rules_path = Path::new(RULES_DIR);
    if !rules_path.exists() {
        return false;
    }

    match fs::read_dir(rules_path) {
        Ok(entries) => entries
            .flatten()
            .any(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.ends_with(".json") && name != "index.json"
            }),
        Err(_) => false,
    }
}

/// 读取上次的 commit SHA
fn read_last_commit() -> Option<String> {
    fs::read_to_string(LAST_COMMIT_FILE).ok().map(|s| s.trim().to_string())
}

/// 保存当前 commit SHA
fn save_last_commit(sha: &str) -> anyhow::Result<()> {
    let _ = fs::create_dir_all(RULES_DIR);
    fs::write(LAST_COMMIT_FILE, sha)?;
    Ok(())
}

/// 获取仓库最新 commit SHA
async fn fetch_latest_commit() -> anyhow::Result<String> {
    let url = CONFIG.github_api_commits();
    let response = get_with_retry(&url).await?;
    let commit: GitHubCommit = response.json().await?;
    Ok(commit.sha)
}

/// 获取仓库中的所有规则文件名
async fn fetch_rule_files() -> anyhow::Result<Vec<String>> {
    let url = CONFIG.github_api_contents();
    let response = get_with_retry(&url).await?;
    let contents: Vec<GitHubContent> = response.json().await?;

    // 过滤出 .json 文件，排除 index.json
    let rule_files: Vec<String> = contents
        .into_iter()
        .filter(|c| {
            c.content_type == "file" && c.name.ends_with(".json") && c.name != "index.json"
        })
        .map(|c| c.name.trim_end_matches(".json").to_string())
        .collect();

    Ok(rule_files)
}

/// 下载单个规则
async fn download_rule(name: &str) -> anyhow::Result<String> {
    let url = format!("{}{}.json", CONFIG.github_raw_base(), name);
    let response = get_with_retry(&url).await?;
    let content = response.text().await?;

    // 验证 JSON 格式
    serde_json::from_str::<serde_json::Value>(&content)?;

    Ok(content)
}

/// 保存规则到本地
fn save_rule(name: &str, content: &str) -> anyhow::Result<()> {
    let _ = fs::create_dir_all(RULES_DIR);
    let path = Path::new(RULES_DIR).join(format!("{}.json", name));
    fs::write(path, content)?;
    Ok(())
}

/// 检查本地是否存在该规则
fn rule_exists(name: &str) -> bool {
    Path::new(RULES_DIR).join(format!("{}.json", name)).exists()
}

/// 检测变动并更新规则
pub async fn update_rules() -> UpdateResult {
    let mut result = UpdateResult {
        total: 0,
        updated: 0,
        added: 0,
        failed: 0,
        details: Vec::new(),
    };

    // 检查是否需要强制更新（本地无规则）
    let force_update = !has_local_rules();
    if force_update {
        info!("📦 本地无规则文件，立即拉取...");
    }

    // 获取最新 commit SHA
    let latest_commit = match fetch_latest_commit().await {
        Ok(sha) => sha,
        Err(e) => {
            warn!("获取最新 commit 失败: {}", e);
            result.details.push(UpdateDetail {
                name: "commit".to_string(),
                action: "failed".to_string(),
                message: format!("获取 commit 失败: {}", e),
            });
            return result;
        }
    };

    debug!("最新 commit: {}", &latest_commit[..7]);

    // 检查是否有变动
    let last_commit = read_last_commit();
    let has_changes = force_update || last_commit.as_ref() != Some(&latest_commit);

    if !has_changes {
        info!("📋 规则无变动 (commit: {})", &latest_commit[..7]);
        return result;
    }

    info!(
        "🔄 检测到变动: {} -> {}",
        last_commit.as_ref().map(|s| &s[..7]).unwrap_or("无"),
        &latest_commit[..7]
    );

    // 获取规则文件列表
    let rule_files = match fetch_rule_files().await {
        Ok(files) => files,
        Err(e) => {
            warn!("获取规则列表失败: {}", e);
            result.details.push(UpdateDetail {
                name: "contents".to_string(),
                action: "failed".to_string(),
                message: format!("获取文件列表失败: {}", e),
            });
            return result;
        }
    };

    result.total = rule_files.len();
    info!("📡 发现 {} 个规则文件", rule_files.len());

    // 下载并保存每个规则
    for name in rule_files {
        let is_new = !rule_exists(&name);

        match download_rule(&name).await {
            Ok(content) => {
                if let Err(e) = save_rule(&name, &content) {
                    warn!("保存规则 {} 失败: {}", name, e);
                    result.failed += 1;
                    result.details.push(UpdateDetail {
                        name: name.clone(),
                        action: "failed".to_string(),
                        message: format!("保存失败: {}", e),
                    });
                } else {
                    if is_new {
                        result.added += 1;
                        debug!("➕ 新增规则: {}", name);
                    } else {
                        result.updated += 1;
                        debug!("🔄 更新规则: {}", name);
                    }
                    result.details.push(UpdateDetail {
                        name: name.clone(),
                        action: if is_new { "added" } else { "updated" }.to_string(),
                        message: "ok".to_string(),
                    });
                }
            }
            Err(e) => {
                warn!("下载规则 {} 失败: {}", name, e);
                result.failed += 1;
                result.details.push(UpdateDetail {
                    name: name.clone(),
                    action: "failed".to_string(),
                    message: format!("下载失败: {}", e),
                });
            }
        }
    }

    // 保存当前 commit SHA
    if let Err(e) = save_last_commit(&latest_commit) {
        warn!("保存 commit SHA 失败: {}", e);
    }

    info!(
        "✅ 更新完成: {} 新增, {} 更新, {} 失败",
        result.added, result.updated, result.failed
    );

    result
}

/// 检查是否需要更新（仅检查，不执行更新）
#[allow(dead_code)]
pub async fn check_for_updates() -> bool {
    if !has_local_rules() {
        return true;
    }

    match fetch_latest_commit().await {
        Ok(latest) => {
            let last = read_last_commit();
            last.as_ref() != Some(&latest)
        }
        Err(_) => false,
    }
}