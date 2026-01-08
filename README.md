# 🎬 AnimeSearch API

基于 Rust + Axum 的在线动漫聚合搜索后端，兼容 [Kazumi](https://github.com/Predidit/Kazumi) 规则格式。

## ✨ 特性

- 🚀 **高性能** - Rust + Tokio 异步运行时
- 📡 **流式响应** - SSE 实时返回搜索结果
- 🔧 **规则驱动** - 兼容 Kazumi 规则格式 (XPath)，自动转换为 CSS 选择器
- 🌐 **多平台** - 支持 70+ 动漫资源站点
- 📺 **集数获取** - 自动获取每个结果的集数列表
- 🔄 **智能重试** - 网络失败时自动使用反代重试
- 🖥️ **内置前端** - 自带简洁的搜索页面
- 📺 **Bangumi API** - 完整代理 Bangumi API，自动添加 CORS
- ⚡ **纯 Rust** - 无 C 依赖，支持跨平台编译

## 📦 技术栈

| 类别 | 技术 |
|------|------|
| 语言 | Rust 1.75+ |
| 框架 | Axum 0.8 |
| 运行时 | Tokio |
| HTTP 客户端 | Reqwest |
| HTML 解析 | scraper (html5ever) |
| XPath 支持 | 自研 XPath→CSS 转换器 |
| 元数据 | Bangumi API |

## 🚀 快速开始

### 编译运行

```bash
cd anime-search-api

# 开发运行
cargo run

# 生产构建
cargo build --release
./target/release/anime-search-api
```

访问 http://localhost:3000 即可使用搜索页面。

### 预编译二进制

从 [Releases](https://github.com/AdingApkgg/anime-search-api/releases) 下载预编译版本：

| 平台 | 文件 |
|------|------|
| Linux x64 | `anime-search-api-x86_64-unknown-linux-gnu.tar.gz` |
| Linux x64 (静态) | `anime-search-api-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `anime-search-api-aarch64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 (静态) | `anime-search-api-aarch64-unknown-linux-musl.tar.gz` |
| Linux ARMv7 (树莓派) | `anime-search-api-armv7-unknown-linux-gnueabihf.tar.gz` |
| macOS Intel | `anime-search-api-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `anime-search-api-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `anime-search-api-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `anime-search-api-aarch64-pc-windows-msvc.zip` |

## 📡 API 接口

### 核心接口

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/` | 搜索页面 |
| POST | `/api` | 搜索动漫 (FormData: `anime=关键词, rules=规则名, episodes=1`) |
| GET | `/info` | API 信息 |
| GET | `/rules` | 获取规则列表 |
| GET | `/update` | 从 KazumiRules 更新规则 |
| GET | `/health` | 健康检查 |

> 💡 设置 `episodes=1` 可获取每个结果的集数列表

### Bangumi API 代理

通用代理，自动添加 CORS 头，前端可直接调用：

| 方法 | 路径 | 说明 |
|------|------|------|
| ANY | `/bgm/*` | 透传到 `api.bgm.tv/*` |

**示例：**

```javascript
// 搜索动漫
fetch('/bgm/v0/search/subjects?limit=10', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ keyword: '葬送的芙莉莲', filter: { type: [2] } })
})

// 获取条目详情
fetch('/bgm/v0/subjects/425249')

// 每日放送
fetch('/bgm/calendar')

// 需要认证的 API
fetch('/bgm/v0/me', {
  headers: { 'Authorization': 'Bearer YOUR_TOKEN' }
})
```

> 🔐 需要认证的 API 请在请求头传入 `Authorization: Bearer <token>`
> 
> 获取 Token: https://next.bgm.tv/demo/access-token

### 搜索请求示例

```javascript
const formData = new FormData()
formData.append('anime', '葬送的芙莉莲')
formData.append('rules', 'AGE,MXdm,NT')
formData.append('episodes', '1')  // 可选：获取集数列表

const response = await fetch('/api', {
  method: 'POST',
  body: formData,
})

const reader = response.body.getReader()
// 读取 SSE 流...
```

### 响应格式 (每行一个 JSON)

```json
{"total": 3}
{"progress": {"completed": 1, "total": 3}, "result": {"name": "AGE动漫", "color": "orange", "tags": ["在线"], "items": [{"name": "葬送的芙莉莲", "url": "...", "episodes": [{"name": null, "episodes": [{"name": "01", "url": "..."}, {"name": "02", "url": "..."}]}]}]}}
{"progress": {"completed": 2, "total": 3}}
{"done": true}
```

## 📝 规则格式

规则文件放在 `rules/` 目录，每个 `.json` 文件是一个规则。

**完全兼容 [Kazumi 规则](https://github.com/Predidit/KazumiRules)**，XPath 表达式会自动转换为 CSS 选择器！

### 规则示例

```json
{
  "api": "1",
  "type": "anime",
  "name": "AGE动漫",
  "version": "1.5",
  "baseURL": "https://www.agedm.io/",
  "searchURL": "https://www.agedm.io/search?query=@keyword",
  "searchList": "//div[2]/div/section/div/div/div/div",
  "searchName": "//div/div[2]/h5/a",
  "searchResult": "//div/div[2]/h5/a",
  "chapterRoads": "//div[2]/div/section/div/div[2]/div[2]/div[2]/div",
  "chapterResult": "//ul/li/a",
  "color": "orange",
  "tags": ["在线"]
}
```

### XPath → CSS 自动转换

| XPath | CSS |
|-------|-----|
| `//div[1]` | `div:nth-of-type(1)` |
| `//div[@class='item']` | `div.item` |
| `//div[@id='main']` | `div#main` |
| `//div[contains(@class, 'btn')]` | `div[class*="btn"]` |
| `//div/a` | `div > a` |
| `//div//a` | `div a` |

### 导入 Kazumi 规则

```bash
# 下载 Kazumi 规则
curl -o rules/gugu3.json https://raw.githubusercontent.com/Predidit/KazumiRules/main/gugu3.json

# 或使用内置更新功能
curl http://localhost:3000/update
```

## 📁 项目结构

```
anime-search-api/
├── Cargo.toml
├── LICENSE
├── README.md
├── rules/              # 规则文件目录 (兼容 Kazumi)
│   ├── AGE.json
│   ├── MXdm.json
│   └── ...             # 70+ 规则
├── static/
│   └── index.html      # 前端页面
└── src/
    ├── main.rs         # 入口 + 路由
    ├── core.rs         # 核心搜索逻辑 (SSE 流)
    ├── engine.rs       # 规则引擎 (scraper)
    ├── xpath_to_css.rs # XPath → CSS 转换器
    ├── rules.rs        # 规则加载器
    ├── types.rs        # 类型定义
    ├── http_client.rs  # HTTP 客户端 (自动反代重试)
    ├── updater.rs      # 规则自动更新
    └── bangumi.rs      # Bangumi API
```

## 🔧 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PORT` | 3000 | 服务端口 |
| `RUST_LOG` | info | 日志级别 |
| `AUTO_UPDATE` | 0 | 启动时自动更新规则 (1=启用) |
| `BANGUMI_ACCESS_TOKEN` | - | Bangumi API 默认 access token |

## 🔄 Nginx 反向代理

```nginx
server {
    listen 443 ssl http2;
    server_name anime.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # SSE 流式响应
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400s;
    }
}
```

> ⚠️ `proxy_buffering off` 确保 SSE 流式响应正常工作

## 🙏 致谢

- [Kazumi](https://github.com/Predidit/Kazumi) - 规则格式参考
- [KazumiRules](https://github.com/Predidit/KazumiRules) - 规则仓库
- [scraper](https://github.com/causal-agent/scraper) - HTML 解析

## 📄 License

MIT
