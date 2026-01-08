//! 配置管理模块
//! 支持从环境变量读取配置，提供默认值

use once_cell::sync::Lazy;
use std::env;

/// 全局配置
pub static CONFIG: Lazy<Config> = Lazy::new(Config::from_env);

/// 应用配置
#[derive(Debug, Clone)]
pub struct Config {
    /// 服务端口
    pub port: u16,

    /// HTTP 请求超时时间 (秒)
    pub timeout_seconds: u64,

    /// 重试请求超时时间 (秒)
    pub retry_timeout_seconds: u64,

    /// HTTP User-Agent
    pub user_agent: String,

    /// 反代前缀 (用于网络问题时重试)
    pub proxy_prefix: String,

    /// GitHub 代理前缀 (用于 GitHub 资源加速)
    pub github_proxy: String,

    /// Bangumi API 地址
    pub bangumi_api_base: String,

    /// Bangumi User-Agent
    pub bangumi_user_agent: String,

    /// 规则仓库 (owner/repo 格式)
    pub rules_repo: String,

    /// 规则仓库分支
    pub rules_branch: String,
}

impl Config {
    /// 从环境变量读取配置
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),

            timeout_seconds: env::var("TIMEOUT_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(15),

            retry_timeout_seconds: env::var("RETRY_TIMEOUT_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),

            user_agent: env::var("USER_AGENT").unwrap_or_else(|_| {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36".to_string()
            }),

            proxy_prefix: env::var("PROXY_PREFIX")
                .unwrap_or_else(|_| "https://rp.30hb.cn/?target=".to_string()),

            github_proxy: env::var("GITHUB_PROXY")
                .unwrap_or_else(|_| "https://gh-proxy.com/".to_string()),

            bangumi_api_base: env::var("BANGUMI_API_BASE")
                .unwrap_or_else(|_| "https://api.bgm.tv".to_string()),

            bangumi_user_agent: env::var("BANGUMI_USER_AGENT")
                .unwrap_or_else(|_| "kirito/anime-search (https://github.com/AdingApkgg/anime-search-api)".to_string()),

            rules_repo: env::var("RULES_REPO")
                .unwrap_or_else(|_| "Predidit/KazumiRules".to_string()),

            rules_branch: env::var("RULES_BRANCH")
                .unwrap_or_else(|_| "main".to_string()),
        }
    }

    /// GitHub API: 获取 commit
    pub fn github_api_commits(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/commits/{}",
            self.rules_repo, self.rules_branch
        )
    }

    /// GitHub API: 获取仓库内容
    pub fn github_api_contents(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/contents",
            self.rules_repo
        )
    }

    /// GitHub Raw: 规则文件基础 URL
    pub fn github_raw_base(&self) -> String {
        format!(
            "https://raw.githubusercontent.com/{}/{}/",
            self.rules_repo, self.rules_branch
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}
