# SpeedTest

多线程网络下载测速工具，交互终端默认使用 ratatui TUI，支持纯文本命令行模式。

```bash
cargo run --release                         # 默认 TUI，国内测速
cargo run --release -- --cli                # 纯文本命令行
cargo run --release -- --overseas           # 海外测速
cargo run --release -- --mainland --overseas # 国内与海外共同选优
cargo run --release -- --url "https://example.com/file.bin" --concurrency 8
cargo run --release -- --help
```

## 界面

- 深色面板、彩色实时/平均/峰值速度卡片，以及最近 120 次采样的下载趋势图。
- 显示累计下载量、运行时间、测速分组、并发上限、重试次数和当前下载地址。
- 小终端自动使用紧凑布局；建议至少 80 列、24 行以显示完整仪表盘。
- `Q`、`Esc` 或 `Ctrl+C` 退出 TUI，并恢复终端状态；CLI 使用 `Ctrl+C` 退出。
- 非交互环境或输出重定向自动使用 CLI，每秒输出一行，不包含清屏控制字符。
- `--tui` 显式要求 TUI，非交互终端下会报错；不能与 `--cli` 同时使用。

```bash
cargo run --release -- --cli > speedtest.log
```

测速持续下载直至退出，会消耗网络流量。默认按 HTTP 响应时间选择可用服务器，并非逐个比较下载吞吐量；指定 `--url` 后只使用该地址，连接失败不会切换至内置服务器。HTTP 错误响应不计入下载量。

速率按实际采样间隔计算，`MB/s`、`Mbps` 和 `GB` 使用十进制单位：1 MB = 1,000,000 字节，1 Mbps = 1,000,000 位/秒。重试次数统计已处理的失败下载任务。

## 开发检查

```bash
cargo fmt --check
cargo clippy
cargo test
```
