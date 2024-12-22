//! 115网盘自动许愿助力系统
//!
//! 该系统实现了自动化许愿、助力和采纳功能，支持多账号并发处理
//! 主要功能包括：
//! - 自动许愿
//! - 获取待处理愿望
//! - 提供助力
//! - 采纳助力
//! - 多账号处理

use anyhow::Result;
use config::{ConfigError, File};
use log::{error, info, warn, LevelFilter};
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::Path,
};
use log4rs::{
    append::{
        console::ConsoleAppender,
        file::FileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use chrono::Local;

// Constants
const CONFIG_FILE_PATH: &str = "config.yaml";
const DEFAULT_WAIT_TIME: u64 = 60; // 默认等待时间(秒)
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

// API Endpoints
const API_BASE_URL: &str = "https://act.115.com/api/1.0/web/1.0/act2024xys";
const WISH_ENDPOINT: &str = "/wish";
const MY_DESIRE_ENDPOINT: &str = "/my_desire";
const AID_DESIRE_ENDPOINT: &str = "/aid_desire";
const ADOPT_ENDPOINT: &str = "/adopt";
const GET_DESIRE_INFO_ENDPOINT: &str = "/get_desire_info";

/// 设置日志系统
fn setup_logger() -> Result<()> {
    // 创建 logs 目录
    std::fs::create_dir_all("logs")?;

    // 生成日志文件名（使用当前日期）
    let log_file_name = format!(
        "logs/115helper_{}.log",
        Local::now().format("%Y-%m-%d")
    );

    // 控制台输出
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} [{l}] - {m}{n}")))
        .build();

    // 文件输出
    let file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} [{l}] - {m}{n}")))
        .build(log_file_name)?;

    // 创建日志配置
    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(LevelFilter::Info),
        )?;

    // 初始化日志系统
    log4rs::init_config(config)?;

    info!("日志系统初始化完成");
    Ok(())
}
#[derive(Deserialize, Debug)]
struct WishResponse {
    state: i32,
    code: i32,
    message: String,
    data: WishData,
}

#[derive(Deserialize, Debug)]
struct WishData {
    #[serde(default)]
    xys_id: String,
}

#[derive(Deserialize, Debug)]
struct MyDesiresResponse {
    state: i32,
    code: i32,
    message: String,
    data: MyDesiresData,
}

#[derive(Deserialize, Debug)]
struct MyDesiresData {
    list: Vec<DesireItem>,
    count: i32,
}

#[derive(Deserialize, Debug)]
struct DesireInfoResponse {
    state: i32,
    code: i32,
    message: String,
    data: DesireInfo,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    user_name: String,
    face_l: String,
}

#[derive(Debug, Deserialize)]
struct DesireInfo {
    id: String,
    content: String,
    images: String,
    edit_time: i64,
    audit_status: i32,
    status: i32,
    aid: i64,
    reward: i64,
    sj_reward: i64,
    code: String,
    aid_num: i32,
    images_data: Vec<String>,
    user_info: UserInfo,
    is_my_desire: i32,
    button: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    aid_cookie: String,
    wish_cookies: Vec<String>,
}

impl AppConfig {
    /// 加载配置文件，如果不存在则创建默认配置
    pub fn load() -> Result<Self, ConfigError> {
        if !Path::new(CONFIG_FILE_PATH).exists() {
            Self::create_default_config()?;
            println!("已创建默认配置文件 config.yaml，请修改其中的 cookie 值后再运行程序。");
            std::process::exit(1);
        }

        config::Config::builder()
            .add_source(File::with_name(CONFIG_FILE_PATH))
            .build()?
            .try_deserialize()
    }

    /// 创建默认配置文件
    fn create_default_config() -> Result<(), ConfigError> {
        let default_config = AppConfig {
            aid_cookie: String::new(),
            wish_cookies: vec![String::new()],
        };

        let yaml = serde_yaml::to_string(&default_config)
            .map_err(|e| ConfigError::Message(e.to_string()))?;

        fs::write(CONFIG_FILE_PATH, yaml)
            .map_err(|e| ConfigError::Message(e.to_string()))?;

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
struct DesireItem {
    code: String,
    aid_num: i32,
}

#[derive(Deserialize, Debug)]
struct AidResponse {
    state: i32,
    code: i32,
    message: String,
    data: serde_json::Value,
}

#[derive(Deserialize, Debug)]
struct AdoptResponse {
    state: i32,
    code: i32,
    message: String,
    data: serde_json::Value,
}

/// 单账号客户端
#[derive(Clone)]
struct Api115ClientSingle {
    client: Client,
    wish_cookie: String,
    aid_cookie: String,
    account_index: usize,
}

impl Api115ClientSingle {
    /// 创建新的单账号客户端实例
    pub fn new(wish_cookie: String, aid_cookie: String, client: Client, account_index: usize) -> Self {
        Self {
            client,
            wish_cookie,
            aid_cookie,
            account_index,
        }
    }

    /// 处理单个账号的所有操作
    async fn process_single_account(&self) -> Result<()> {
        let account_msg = format!("===== 开始处理第 {} 个账号 =====", self.account_index + 1);
        info!("{}", account_msg);

        // 执行许愿操作
        self.handle_wish_process().await?;

        // 处理待处理愿望
        self.handle_pending_wishes().await?;

        Ok(())
    }

    /// 处理许愿流程
    async fn handle_wish_process(&self) -> Result<()> {
        info!("[账号-{}] 准备开始许愿...", self.account_index + 1);

        match self.make_wish().await {
            Ok(Some(wish_id)) => {
                info!("[账号-{}] 许愿成功完成，ID: {}", self.account_index + 1, wish_id);
            }
            Ok(None) => {
                warn!("[账号-{}] 许愿未成功完成", self.account_index + 1);
            }
            Err(e) => {
                error!("[账号-{}] 许愿过程发生错误: {}", self.account_index + 1, e);
            }
        }

        Ok(())
    }

    /// 处理待处理愿望
    async fn handle_pending_wishes(&self) -> Result<()> {
        let pending_wishes = self.get_pending_wishes().await?;

        for wish_id in pending_wishes {
            if let Ok(Some(aid_id)) = self.aid_desire(&wish_id).await {
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                match self.adopt_aid(&wish_id, &aid_id).await {
                    Ok(true) => info!("愿望 {} 的助力已被成功采纳", wish_id),
                    Ok(false) => warn!("采纳愿望 {} 的助力失败", wish_id),
                    Err(e) => error!("采纳愿望 {} 的助力时发生错误: {}", wish_id, e),
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(DEFAULT_WAIT_TIME)).await;
        }

        Ok(())
    }

    /// 账号创建许愿
    pub async fn make_wish(&self) -> Result<Option<String>> {
        info!("开始发送许愿请求...");

        let url = "https://act.115.com/api/1.0/web/1.0/act2024xys/wish";

        let response = match self.client.post(url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9")
            .header("Cache-Control", "no-cache")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Cookie", &self.wish_cookie)
            .header("Origin", "https://v.115.com")
            .header("Referer", "https://v.115.com/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36")
            .header("sec-ch-ua", "\"Not(A:Brand\";v=\"99\", \"Google Chrome\";v=\"133\", \"Chromium\";v=\"133\"")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", "\"Windows\"")
            .form(&[
                ("content", "gogogog"),
                ("images", ""),
                ("rewardSpace", "5"),
            ])
            .send()
            .await {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("发送许愿请求失败: {}", e);
                error!("{}", msg);
                return Ok(None);
            }
        };

        if !response.status().is_success() {
            let msg = format!("许愿请求失败，状态码: {}", response.status());
            error!("{}", msg);
            return Ok(None);
        }

        let wish_response = match response.json::<WishResponse>().await {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("解析许愿响应失败: {}", e);
                error!("{}", msg);
                return Ok(None);
            }
        };

        if wish_response.state == 1 && wish_response.code == 0 {
            let msg = format!("许愿成功！ID: {} 等待60s时间用于审核", wish_response.data.xys_id);
            info!("{}", msg);
            // 等待一小段时间，避免请求过于频繁
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            Ok(Some(wish_response.data.xys_id))
        } else {
            let msg = format!("许愿失败: {} (状态: {}, 代码: {})",
                              wish_response.message, wish_response.state, wish_response.code);
            warn!("{}", msg);
            Ok(None)
        }
    }

    pub async fn get_pending_wishes(&self) -> Result<HashSet<String>> {
        info!("开始获取待处理愿望列表...");

        let url = "https://act.115.com/api/1.0/web/1.0/act2024xys/my_desire";

        let response = match self.client.get(url)
            .query(&[
                ("type", "0"),
                ("start", "0"),
                ("page", "1"),
                ("limit", "10"),
            ])
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9")
            .header("Cache-Control", "no-cache")
            .header("Cookie", &self.wish_cookie)
            .header("Origin", "https://v.115.com")
            .header("Referer", "https://v.115.com/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36")
            .send()
            .await {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("获取愿望列表请求失败: {}", e);
                error!("{}", msg);
                return Ok(HashSet::new());
            }
        };

        if !response.status().is_success() {
            let msg = format!("获取愿望列表失败，状态码: {}", response.status());
            error!("{}", msg);
            return Ok(HashSet::new());
        }

        let desires_response = match response.json::<MyDesiresResponse>().await {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("解析愿望列表响应失败: {}", e);
                error!("{}", msg);
                return Ok(HashSet::new());
            }
        };

        if desires_response.state == 1 && desires_response.code == 0 {
            let pending_wishes: HashSet<String> = desires_response.data.list
                .into_iter()
                .filter(|item| item.aid_num == 0)
                .map(|item| item.code)
                .collect();

            let msg = format!("成功获取到 {} 个待处理愿望", pending_wishes.len());
            info!("{}", msg);
            Ok(pending_wishes)
        } else {
            let msg = format!("获取愿望列表失败: {} (状态: {}, 代码: {})",
                              desires_response.message, desires_response.state, desires_response.code);
            warn!("{}", msg);
            Ok(HashSet::new())
        }
    }

    pub async fn aid_desire(&self, wish_id: &str) -> Result<Option<String>> {
        info!("开始为愿望 {} 提供助力...", wish_id);

        let wish_code = self.get_desire_code(wish_id).await?;

        if wish_code == "" {
            let msg = format!("获取愿望 {} 的详情失败", wish_code);
            error!("{}", msg);
            return Ok(None);
        }

        let url = "https://act.115.com/api/1.0/web/1.0/act2024xys/aid_desire";

        let payload = [
            ("id", wish_code),
            ("content", String::from("gogogo")),  // 使用相同的内容
            ("images", String::new()),
            ("file_ids", String::new()),
        ];

        let response = match self.client
            .post(url)
            .header("Host", "act.115.com")
            .header("Accept", "application/json, text/plain, */*")
            .header("Sec-Fetch-Site", "same-site")
            .header("Accept-Language", "zh-CN,zh-Hans;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Sec-Fetch-Mode", "cors")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Content-Length", "42")
            .header("Origin", "https://v.115.com")
            .header("User-Agent", "Mozilla/5.0 (iPhone; CPU iPhone OS 12_3_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 UDown/32.9.2")
            .header("Referer", "https://v.115.com/")
            .header("Connection", "keep-alive")
            .header("Sec-Fetch-Dest", "empty")
            .header("Cookie", &self.aid_cookie)
            .form(&payload)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("为愿望 {} 提供助力请求失败: {}", wish_id, e);
                error!("{}", msg);
                return Ok(None);
            }
        };

        let status = response.status();
        let response_text = response.text().await?;
        info!("服务器响应状态: {}", status);
        info!("服务器响应内容: {}", response_text);

        let aid_response: AidResponse = match serde_json::from_str(&response_text) {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("解析愿望 {} 助力响应失败: {} (响应内容: {})",
                                  wish_id, e, response_text);
                error!("{}", msg);
                return Ok(None);
            }
        };

        if aid_response.state == 1 && aid_response.code == 0 {
            if let Some(data) = aid_response.data.as_object() {
                if let Some(aid_id) = data.get("aid_id").and_then(|v| v.as_str()) {
                    info!("助力成功，等待10s时间防止频繁请求");
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    let msg = format!("为愿望 {} 助力成功！aid_id: {}", wish_id, aid_id);
                    info!("{}", msg);
                    return Ok(Some(aid_id.to_string()));
                }
            }
            let msg = format!("为愿望 {} 助力成功但未返回 aid_id", wish_id);
            warn!("{}", msg);
            Ok(None)
        } else {
            let msg = format!("为愿望 {} 助力失败: {} (状态: {}, 代码: {})",
                              wish_id, aid_response.message, aid_response.state, aid_response.code);
            warn!("{}", msg);
            Ok(None)
        }
    }
    // 添加采纳助力的方法
    pub async fn adopt_aid(&self, wish_id: &str, aid_id: &str) -> Result<bool> {
        info!("开始采纳愿望 {} 的助力 {}...", wish_id, aid_id);

        let url = "https://act.115.com/api/1.0/web/1.0/act2024xys/adopt";

        let response = match self.client.post(url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Cookie", &self.wish_cookie)  // 使用许愿的 cookie
            .header("Origin", "https://v.115.com")
            .header("Referer", "https://v.115.com/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36")
            .form(&[
                ("did", wish_id),
                ("aid", aid_id),
                ("to_cid", "0"),
            ])
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("采纳助力请求失败: {}", e);
                error!("{}", msg);
                return Ok(false);
            }
        };

        if !response.status().is_success() {
            let msg = format!("采纳助力失败，状态码: {}", response.status());
            error!("{}", msg);
            return Ok(false);
        }

        let adopt_response = match response.json::<AdoptResponse>().await {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("解析采纳助力响应失败: {}", e);
                error!("{}", msg);
                return Ok(false);
            }
        };

        if adopt_response.state == 1 && adopt_response.code == 0 {
            let msg = format!("成功采纳愿望 {} 的助力 {}", wish_id, aid_id);
            info!("{}", msg);
            Ok(true)
        } else {
            let msg = format!("采纳助力失败: {} (状态: {}, 代码: {})",
                              adopt_response.message, adopt_response.state, adopt_response.code);
            warn!("{}", msg);
            Ok(false)
        }
    }

    // 获取愿望详情，多这一步的原因是愿望列表中的code，虽然看似一样，但是不知道什么原因，无法助力成功，而通过这个接口获取到的code可以成功助力
    pub async fn get_desire_code(&self, id: &str) -> Result<String> {
        info!("开始获取待助力愿望 {} 的详情...", id);

        let url = "https://act.115.com/api/1.0/web/1.0/act2024xys/get_desire_info?id=".to_owned() + id;

        let response = match self.client.get(url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "zh-CN,zh;q=0.9")
            .header("Cache-Control", "no-cache")
            .header("Cookie", &self.aid_cookie)
            .header("Origin", "https://v.115.com")
            .header("Referer", "https://v.115.com/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36")
            .send()
            .await {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("获取愿望详情请求失败: {}", e);
                error!("{}", msg);
                return Ok(String::new());
            }
        };

        if !response.status().is_success() {
            let msg = format!("获取愿望详情失败，状态码: {}", response.status());
            error!("{}", msg);
            return Ok(String::new());
        }
        // 先获取原始响应文本进行调试
        let response_text = match response.text().await {
            Ok(text) => {
                info!("收到的响应内容: {}", text);
                text
            }
            Err(e) => {
                let msg = format!("读取响应内容失败: {}", e);
                error!("{}", msg);
                return Ok(String::new());
            }
        };

        // 尝试解析JSON
        let desire_response: DesireInfoResponse = match serde_json::from_str(&response_text) {
            Ok(resp) => resp,
            Err(e) => {
                let msg = format!("解析愿望详情响应失败: {} \n响应内容: {}", e, response_text);
                error!("{}", msg);
                return Ok(String::new());
            }
        };


        if desire_response.state == 1 && desire_response.code == 0 {
            let msg = format!("成功获取到 {} 愿望详情", desire_response.data.code);
            info!("{}", msg);
            Ok(desire_response.data.code)
        } else {
            let msg = format!("获取愿望详情: {} (状态: {}, 代码: {})",
                              desire_response.message, desire_response.state, desire_response.code);
            warn!("{}", msg);
            Ok(String::new())
        }
    }
}

/// 多账号客户端
#[derive(Clone)]
struct Api115Client {
    client: Client,
    wish_cookies: Vec<String>,
    aid_cookie: String,
}

impl Api115Client {
    /// 创建新的多账号客户端实例
    pub fn new(wish_cookies: Vec<String>, aid_cookie: String) -> Self {
        let client = ClientBuilder::new()
            .gzip(true)
            .deflate(true)
            .brotli(true)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            wish_cookies,
            aid_cookie,
        }
    }

    /// 对所有账号一个个处理，以防并发风控
    pub async fn process_all_accounts(&self) -> Result<()> {
        for (index, wish_cookie) in self.wish_cookies.iter().enumerate() {
            info!("开始处理第 {} 个账号，共 {} 个账号", index + 1, self.wish_cookies.len());

            let single_client = Api115ClientSingle::new(
                wish_cookie.clone(),
                self.aid_cookie.clone(),
                self.client.clone(),
                index,
            );

            if let Err(e) = single_client.process_single_account().await {
                error!("[账号-{}] 处理账号时出错: {}", index + 1, e);
            }

            // Add a delay between processing different accounts to avoid rate limiting
            if index < self.wish_cookies.len() - 1 {
                info!("等待60秒后处理下一个账号...");
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统
    if let Err(e) = setup_logger() {
        eprintln!("初始化日志系统失败: {}", e);
        return Ok(());
    }

    info!("程序开始执行 - {}", Local::now().format("%Y-%m-%d %H:%M:%S"));

    // 加载并验证配置
    let config = match AppConfig::load() {
        Ok(cfg) => {
            if cfg.wish_cookies.is_empty() {
                error!("未配置任何 wish cookie");
                return Ok(());
            }
            if cfg.aid_cookie.is_empty() {
                error!("未配置 aid cookie");
                return Ok(());
            }
            cfg
        }
        Err(e) => {
            error!("加载配置文件失败: {}", e);
            return Ok(());
        }
    };

    // 创建客户端并处理所有账号
    let client = Api115Client::new(config.wish_cookies, config.aid_cookie);
    if let Err(e) = client.process_all_accounts().await {
        error!("处理账号时发生错误: {}", e);
    }

    info!("所有愿望处理完成 - {}", Local::now().format("%Y-%m-%d %H:%M:%S"));

    Ok(())
}