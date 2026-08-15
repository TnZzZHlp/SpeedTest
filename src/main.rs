use clap::Parser;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering::Relaxed},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{spawn, task::JoinSet};

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
static OVERSEAS_ADDRESS: [&str; 1] =
    ["https://speed.cloudflare.com/__down?bytes=99999999"];

static SPEED: AtomicUsize = AtomicUsize::new(0);
static DOWNLOADED: AtomicUsize = AtomicUsize::new(0);
static DOWNLOADING: AtomicUsize = AtomicUsize::new(0);
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
}
#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.concurrency == 0 {
        panic!("线程数不合法");
    }

    // 按 --mainland/--overseas 选择测速分组, 未指定时默认使用国内地址
    let mut addresses: Vec<&'static str> = Vec::new();
    if args.mainland {
        addresses.extend(MAINLAND_ADDRESS);
    }
    if args.overseas {
        addresses.extend(OVERSEAS_ADDRESS);
    }
    if addresses.is_empty() {
        addresses.extend(MAINLAND_ADDRESS);
    }

    let group_name = if !args.url.is_empty() {
        "自定义地址"
    } else if args.mainland && args.overseas {
        "国内 + 海外"
    } else if args.overseas {
        "海外"
    } else {
        "国内"
    };

    let client = Arc::new(reqwest::Client::new());
    let addresses = Arc::new(addresses);

    if !args.url.is_empty() {
        *BEST.lock().unwrap() = args.url.clone();
    } else {
        println!("正在寻找最佳下载地址...");
        find_best(&client, &addresses).await;
    }

    loop {
        if DOWNLOADING.load(Relaxed) < args.concurrency {
            for _ in DOWNLOADING.load(Relaxed)..args.concurrency {
                spawn(downloader(client.clone(), args.ua.clone(), addresses.clone()));
                DOWNLOADING.fetch_add(1, Relaxed);
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
        let downloaded = SPEED.swap(0, Relaxed);
        DOWNLOADED.fetch_add(downloaded, Relaxed);

        // 清屏
        print!("{esc}[2J{esc}[1;1H", esc = 27 as char);

        println!(
            "当前下载速度: {:.2} MB/s {:.0}Mbps\n已下载: {:.2} GB\n当前下载线程数: {} \n测速分组: {}\n当前下载地址: {}",
            (downloaded as f64) / 1024.0 / 1024.0,
            ((downloaded as f64) / 1024.0 / 1024.0) * 8.0,
            (DOWNLOADED.load(std::sync::atomic::Ordering::Relaxed) as f64) /
                1024.0 /
                1024.0 /
                1024.0,
            DOWNLOADING.load(Relaxed),
            group_name,
            BEST.lock().unwrap()
        );
    }
}

async fn find_best(client: &reqwest::Client, addresses: &[&'static str]) {
    let mut tasks = JoinSet::new();

    for target in addresses {
        tasks.spawn(test(client.clone(), target.to_string()));
    }

    let mut output = tasks.join_all().await;

    output.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    *BEST.lock().unwrap() = output[0].0.clone();
}

async fn test(client: reqwest::Client, address: String) -> (String, u128) {
    let now = std::time::Instant::now();

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

async fn downloader(
    client: Arc<reqwest::Client>,
    ua: String,
    addresses: Arc<Vec<&'static str>>,
) {
    loop {
        let best = BEST.lock().unwrap().clone();
        let mut res = match client
            .get(best)
            .header("User-Agent", &ua)
            .send()
            .await
        {
            Ok(res) => res,
            Err(_) => {
                // 连接失败: 重新选优后退出, 主循环会补位新线程
                find_best(&client, &addresses).await;
                DOWNLOADING.fetch_sub(1, Relaxed);
                return;
            }
        };

        loop {
            match res.chunk().await {
                Ok(Some(chunk)) => {
                    SPEED.fetch_add(chunk.len(), Relaxed);
                }
                Ok(None) => {
                    break;
                }
                Err(_) => {
                    // 连接中断: 重新选优后退出, 主循环会补位新线程
                    find_best(&client, &addresses).await;
                    DOWNLOADING.fetch_sub(1, Relaxed);
                    return;
                }
            }
        }
    }
}
