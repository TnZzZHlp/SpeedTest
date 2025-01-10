use clap::Parser;
use std::{ io::{ self, Write }, sync::{ atomic::AtomicUsize, Arc }, time::Duration };
use tokio::{ spawn, task::JoinSet };

static ADDRESS: [&str; 8] = [
    "https://download.alicdn.com/wireless/taobao4android/latest/taobao4android_703304.apk",
    "https://dldir1.qq.com/qqfile/qq/TIM3.5.0/TIM3.5.0.22143.exe",
    "https://res.app.coc.10086.cn/downfile/apk/CM10086_android_V11.4.0_20241023213523371.apk",
    "https://bce.bdstatic.com/bce-app/android/4.9.14-release.apk",
    "https://cloud.video.taobao.com/play/u/null/p/1/e/6/t/1/d/ud/329682839911.mp4",
    "https://e890f2fd0e182ec52eb7bae54f5fd897.b.hon.cc.cdnhwc8.com:32590/appdl-1-drcn.dbankcdn.com/dl/appdl/application/apk/a2/a2088eec037441b89156fe405d41c761/PC661608e54be346009a87ff4923a609e5.2409091528.exe",
    "https://ctyun-portal.gdoss.xstore.ctyun.cn/download/ctyun.apk",
    "https://speed.cloudflare.com/__down?bytes=1000000000",
];

static SPEED: AtomicUsize = AtomicUsize::new(0);
static DOWNLOADED: AtomicUsize = AtomicUsize::new(0);
static DOWNLOADING: AtomicUsize = AtomicUsize::new(0);
static mut BEST: String = String::new();

#[derive(Parser)]
struct Args {
    /// 下载地址
    #[clap(short, long, default_value = "")]
    url: String,

    /// 线程数
    #[clap(short, long, default_value = "16")]
    concurrency: usize,

    /// User-Agent
    #[clap(
        long,
        default_value = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0"
    )]
    ua: String,
}
#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.concurrency == 0 {
        panic!("线程数不合法");
    }

    let client = Arc::new(reqwest::Client::new());

    if !args.url.is_empty() {
        unsafe {
            BEST = args.url.clone();
        }
    } else {
        println!("正在寻找最佳下载地址...");
        find_best().await;
    }

    loop {
        if DOWNLOADING.load(std::sync::atomic::Ordering::Relaxed) < args.concurrency {
            for _ in DOWNLOADING.load(std::sync::atomic::Ordering::Relaxed)..args.concurrency {
                spawn(downloader(client.clone(), args.ua.clone()));
                DOWNLOADING.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let downloaded = SPEED.swap(0, std::sync::atomic::Ordering::Relaxed);
        DOWNLOADED.fetch_add(downloaded, std::sync::atomic::Ordering::Relaxed);
        io::stdout().flush().unwrap(); // 刷新标准输出缓冲区
        print!(
            "\r当前下载速度：{:6.2} MB/s {:4.0}Mbps 已下载：{:6.2} GB 当前下载地址：{}",
            (downloaded as f64) / 1024.0 / 1024.0,
            ((downloaded as f64) / 1024.0 / 1024.0) * 8.0,
            (DOWNLOADED.load(std::sync::atomic::Ordering::Relaxed) as f64) /
                1024.0 /
                1024.0 /
                1024.0,
            unsafe {
                &BEST
            }
        );
    }
}

async fn find_best() {
    let mut tasks = JoinSet::new();

    for target in ADDRESS {
        tasks.spawn(test(target.to_string()));
    }

    let mut output = tasks.join_all().await;

    output.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    unsafe {
        BEST = output[0].0.clone();
    }
}

async fn test(address: String) -> (String, u128) {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();

    let now = std::time::Instant::now();

    match client.get(&address).send().await {
        Ok(res) => {
            if res.status().is_success() {
                (address, now.elapsed().as_millis())
            } else {
                (address, u128::MAX)
            }
        }
        Err(_) => (address, u128::MAX),
    }
}

async fn downloader(client: Arc<reqwest::Client>, ua: String) {
    loop {
        let mut res = client
            .get(unsafe { &BEST })
            .header("User-Agent", &ua)
            .send().await
            .unwrap();

        loop {
            match res.chunk().await {
                Ok(Some(chunk)) => {
                    SPEED.fetch_add(chunk.len(), std::sync::atomic::Ordering::Relaxed);
                }
                Ok(None) => {
                    break;
                }
                Err(_) => {
                    find_best().await;
                    return;
                }
            }
        }
    }
}
