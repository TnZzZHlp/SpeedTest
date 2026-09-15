mod ui;

use clap::Parser;
use std::{
    io::{self, IsTerminal, Write},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
        Mutex,
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::task::JoinSet;

/// 国内测速地址
static MAINLAND_ADDRESS: [&str; 9] = [
    "https://download.alicdn.com/wireless/taobao4android/latest/taobao4android_703304.apk",
    "https://dldir1.qq.com/qqfile/qq/TIM3.5.0/TIM3.5.0.22143.exe",
    "https://res.app.coc.10086.cn/downfile/apk/CM10086_android_V11.4.0_20241023213523371.apk",
    "https://bce.bdstatic.com/bce-app/android/4.9.14-release.apk",
    "https://cloud.video.taobao.com/play/u/null/p/1/e/6/t/1/d/ud/329682839911.mp4",
    "https://e890f2fd0e182ec52eb7bae54f5fd897.b.hon.cc.cdnhwc8.com:32590/appdl-1-drcn.dbankcdn.com/dl/appdl/application/apk/a2/a2088eec037441b89156fe405d41c761/PC661608e54be346009a87ff4923a609e5.2409091528.exe",
    "https://ctyun-portal.gdoss.xstore.ctyun.cn/download/ctyun.apk",
    "https://dl.hdslb.com/mobile/fixed/bili_win/bili_win-install.exe",
    "https://www.douyin.com/download/pc/obj/douyin-pc-web/douyin-pc-client/7044145585217083655/releases/12270856/5.3.1/win32-ia32/douyin-downloader-v5.3.1-win32-ia32-douyincold.exe",
];

/// 海外测速地址
/// Cloudflare 端点限制单次请求 < 100MB, 超出返回 403
static OVERSEAS_ADDRESS: [&str; 8] = [
    // 全球(Anycast)
    "https://speed.cloudflare.com/__down?bytes=99999999",
    // 欧洲
    "https://proof.ovh.net/files/100Mb.dat",
    "https://speedtest.london.linode.com/100MB-london.bin",
    // 亚太
    "https://speedtest.singapore.linode.com/100MB-singapore.bin",
    "https://speedtest.tokyo2.linode.com/100MB-tokyo2.bin",
    // 北美
    "https://speedtest.newark.linode.com/100MB-newark.bin",
    "https://speedtest.fremont.linode.com/100MB-fremont.bin",
    "https://speedtest.dallas.linode.com/100MB-dallas.bin",
];

const APPLE_DOWNLOAD_ADDRESS: &str = "https://mensura.cdn-apple.com/api/v1/gm/large";
const APPLE_UPLOAD_ADDRESS: &str = "https://mensura.cdn-apple.com/api/v1/gm/slurp";
static UPLOAD_CHUNK: [u8; 64 * 1024] = [0; 64 * 1024];

static DOWNLOADED: AtomicU64 = AtomicU64::new(0);
static UPLOADED: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicUsize = AtomicUsize::new(0);
static BEST: Mutex<String> = Mutex::new(String::new());

#[derive(Parser)]
#[command(name = "SpeedTest", version, about = "多线程网络测速工具")]
struct Args {
    /// 自定义下载地址(指定后优先于 --mainland/--overseas)
    #[arg(short, long, default_value = "")]
    url: String,

    /// 线程数
    #[arg(short, long, default_value_t = 16)]
    concurrency: usize,

    /// User-Agent
    #[arg(
        long,
        default_value = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0"
    )]
    ua: String,

    /// 使用国内测速地址(默认分组)
    #[arg(long)]
    mainland: bool,

    /// 使用海外测速地址
    #[arg(long)]
    overseas: bool,

    /// 使用纯文本命令行界面(支持重定向)
    #[arg(long, conflicts_with = "tui")]
    cli: bool,

    /// 强制使用 TUI 界面(需要交互终端)
    #[arg(long)]
    tui: bool,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.concurrency == 0 {
        return Err("并发数必须大于 0".into());
    }
    if !args.url.is_empty() {
        let url = reqwest::Url::parse(&args.url)?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("下载地址必须是有效的 HTTP 或 HTTPS URL".into());
        }
    }
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    if args.tui && !interactive {
        return Err("TUI 需要交互终端，请使用 --cli".into());
    }
    let use_tui = !args.cli && interactive;
    let group = if !args.url.is_empty() {
        "自定义地址"
    } else if args.mainland && args.overseas {
        "国内 + 海外"
    } else if args.overseas {
        "海外"
    } else {
        "国内"
    };
    let client = reqwest::Client::builder()
        .user_agent(&args.ua)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(15))
        .build()?;
    let mut terminal = if use_tui { Some(ui::Tui::new()?) } else { None };
    let mut dashboard = ui::Dashboard::new(group, args.concurrency);
    let measurement = measure(client, &args);
    tokio::pin!(measurement);
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut started = None;
    let mut sampled = Instant::now();
    if !use_tui {
        writeln!(io::stdout(), "正在寻找最佳下载地址... (Ctrl+C 退出)")?;
    }

    loop {
        tokio::select! {
            result = &mut measurement => { result?; break; }
            result = &mut shutdown => { result?; break; }
            _ = tick.tick() => {
                if let Some(terminal) = terminal.as_mut() {
                    if terminal.should_quit()? { break; }
                }
                dashboard.address = BEST.lock().unwrap().clone();
                dashboard.errors = ERRORS.load(Relaxed);
                let now = Instant::now();
                if !dashboard.address.is_empty() && started.is_none() {
                    started = Some(now);
                    sampled = now;
                }
                let interval = now.duration_since(sampled);
                if let Some(start) = started {
                    if interval >= Duration::from_secs(1) {
                        dashboard.sample(
                            DOWNLOADED.load(Relaxed),
                            UPLOADED.load(Relaxed),
                            now - start,
                            interval,
                        );
                        sampled = now;
                        if !use_tui {
                            writeln!(io::stdout(), "{}", dashboard.cli_line())?;
                        }
                    }
                }
                if let Some(terminal) = terminal.as_mut() {
                    terminal.draw(&dashboard)?;
                }
            }
        }
    }
    Ok(())
}

async fn measure(client: reqwest::Client, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut addresses = Vec::new();
    if !args.url.is_empty() {
        addresses.push(args.url.clone());
        *BEST.lock().unwrap() = args.url.clone();
    } else {
        if args.mainland || !args.overseas {
            addresses.extend(MAINLAND_ADDRESS.map(String::from));
        }
        if args.overseas {
            addresses.extend(OVERSEAS_ADDRESS.map(String::from));
        }
        addresses.push(APPLE_DOWNLOAD_ADDRESS.to_string());
        find_best(&client, &addresses).await?;
    }

    // Apple 的 /slurp 接收无限长的二进制 POST 请求；自定义下载地址不启用它。
    let mut upload_workers = JoinSet::new();
    if args.url.is_empty() {
        for _ in 0..args.concurrency {
            upload_workers.spawn(uploader(client.clone()));
        }
    }

    // JoinSet 在退出时取消全部下载和上传，避免后台任务脱离界面生命周期。
    let mut workers = JoinSet::new();
    for _ in 0..args.concurrency {
        workers.spawn(downloader(client.clone()));
    }
    while let Some(result) = workers.join_next().await {
        result?;
        ERRORS.fetch_add(1, Relaxed);
        tokio::time::sleep(Duration::from_secs(1)).await;
        if args.url.is_empty() {
            find_best(&client, &addresses).await?;
        }
        workers.spawn(downloader(client.clone()));
    }
    Ok(())
}

async fn find_best(client: &reqwest::Client, addresses: &[String]) -> io::Result<()> {
    let mut tasks = JoinSet::new();
    for target in addresses {
        tasks.spawn(test(client.clone(), target.clone()));
    }
    let mut best = None;
    while let Some(result) = tasks.join_next().await {
        let (address, latency) = result.map_err(io::Error::other)?;
        if latency != u128::MAX && best.as_ref().is_none_or(|(_, time)| latency < *time) {
            best = Some((address, latency));
        }
    }
    match best {
        Some((address, _)) => {
            *BEST.lock().unwrap() = address;
            Ok(())
        }
        None => Err(io::Error::other(
            "没有可用的测速地址，请使用 --url 指定地址",
        )),
    }
}

async fn test(client: reqwest::Client, address: String) -> (String, u128) {
    let now = Instant::now();
    match client
        .get(&address)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => (address, now.elapsed().as_millis()),
        _ => (address, u128::MAX),
    }
}

struct UploadBody;

impl futures_core::Stream for UploadBody {
    type Item = Result<&'static [u8], io::Error>;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        UPLOADED.fetch_add(UPLOAD_CHUNK.len() as u64, Relaxed);
        Poll::Ready(Some(Ok(&UPLOAD_CHUNK)))
    }
}

async fn uploader(client: reqwest::Client) {
    loop {
        let result = client
            .post(APPLE_UPLOAD_ADDRESS)
            .header("Accept-Encoding", "identity")
            .header("Content-Type", "application/octet-stream")
            .body(reqwest::Body::wrap_stream(UploadBody))
            .send()
            .await;
        if let Ok(response) = result {
            if response.status().is_success() {
                let _ = response.bytes().await;
                continue;
            }
        }
        ERRORS.fetch_add(1, Relaxed);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn downloader(client: reqwest::Client) {
    loop {
        let best = BEST.lock().unwrap().clone();
        let mut response = match client
            .get(best)
            .send()
            .await
            .and_then(|res| res.error_for_status())
        {
            Ok(response) => response,
            Err(_) => return,
        };
        let mut received = false;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    received |= !chunk.is_empty();
                    DOWNLOADED.fetch_add(chunk.len() as u64, Relaxed);
                }
                Ok(None) if received => break,
                _ => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_flags_are_mutually_exclusive() {
        assert!(Args::try_parse_from(["SpeedTest", "--cli"]).unwrap().cli);
        assert!(Args::try_parse_from(["SpeedTest", "--tui"]).unwrap().tui);
        assert!(Args::try_parse_from(["SpeedTest", "--cli", "--tui"]).is_err());
    }
}
