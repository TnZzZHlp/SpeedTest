use std::{collections::VecDeque, io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    symbols::Marker,
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Chart, Dataset, GraphType, Paragraph, Wrap},
    DefaultTerminal, Frame,
};

const BACKGROUND: Color = Color::Rgb(15, 23, 42);
const PANEL: Color = Color::Rgb(22, 33, 53);
const BORDER: Color = Color::Rgb(51, 65, 85);
const TEXT: Color = Color::Rgb(226, 232, 240);
const MUTED: Color = Color::Rgb(148, 163, 184);
const ACCENT: Color = Color::Rgb(56, 189, 248);
const GREEN: Color = Color::Rgb(52, 211, 153);
const HISTORY_LEN: usize = 120;

pub struct Dashboard {
    pub address: String,
    pub errors: usize,
    group: &'static str,
    concurrency: usize,
    total: u64,
    elapsed: Duration,
    speed: f64,
    average: f64,
    peak: f64,
    history: VecDeque<f64>,
}

impl Dashboard {
    pub fn new(group: &'static str, concurrency: usize) -> Self {
        Self {
            address: String::new(),
            errors: 0,
            group,
            concurrency,
            total: 0,
            elapsed: Duration::ZERO,
            speed: 0.0,
            average: 0.0,
            peak: 0.0,
            history: VecDeque::with_capacity(HISTORY_LEN),
        }
    }

    pub fn sample(&mut self, total: u64, elapsed: Duration, interval: Duration) {
        self.speed = total.saturating_sub(self.total) as f64 / interval.as_secs_f64().max(0.001);
        self.average = total as f64 / elapsed.as_secs_f64().max(0.001);
        self.peak = self.peak.max(self.speed);
        self.total = total;
        self.elapsed = elapsed;
        if self.history.len() == HISTORY_LEN {
            self.history.pop_front();
        }
        self.history.push_back(self.speed / 1_000_000.0);
    }

    pub fn cli_line(&self) -> String {
        format!(
            "当前下载速度: {:.2} MB/s | {:.2} Mbps | 平均: {:.2} MB/s | 峰值: {:.2} MB/s | 已下载: {:.3} GB | 用时: {}s | 并发上限: {} | 重试: {} | 分组: {} | 地址: {}",
            self.speed / 1_000_000.0,
            self.speed * 8.0 / 1_000_000.0,
            self.average / 1_000_000.0,
            self.peak / 1_000_000.0,
            self.total as f64 / 1_000_000_000.0,
            self.elapsed.as_secs(),
            self.concurrency,
            self.errors,
            self.group,
            self.address,
        )
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(Block::default().bg(BACKGROUND).fg(TEXT), area);
        let status = if self.address.is_empty() {
            "正在寻找最佳下载地址..."
        } else if self.speed == 0.0 && self.errors > 0 {
            "连接异常，正在重试..."
        } else if self.history.is_empty() {
            "正在连接，等待首个采样..."
        } else {
            "持续测速中"
        };
        if area.width < 60 || area.height < 22 {
            frame.render_widget(
                Paragraph::new(format!(
                    "SpeedTest / {}\n{}\n\n{:.2} MB/s  /  {:.2} Mbps\n平均 {:.2}  峰值 {:.2} MB/s\n已下载 {:.3} GB / {}s\n并发上限 {} / 重试 {}\n{}\n\nQ / Esc / Ctrl+C 退出",
                    self.group, status, self.speed / 1_000_000.0,
                    self.speed * 8.0 / 1_000_000.0, self.average / 1_000_000.0,
                    self.peak / 1_000_000.0, self.total as f64 / 1_000_000_000.0,
                    self.elapsed.as_secs(), self.concurrency, self.errors, self.address,
                ))
                .block(panel("网络测速"))
                .wrap(Wrap { trim: false }),
                area,
            );
            return;
        }

        let rows = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(7),
            Constraint::Length(6),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" SpeedTest ", Style::new().fg(ACCENT).bold()),
                Span::styled(" / 网络测速    ", Style::new().fg(MUTED)),
                Span::styled(status, Style::new().fg(GREEN)),
            ]))
            .block(Block::default().bg(PANEL)),
            rows[0],
        );
        let cards = Layout::horizontal([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(rows[1]);
        for (index, (title, value, color, detail)) in [
            (
                " 实时下载 ",
                self.speed,
                ACCENT,
                format!("{:.2} Mbps", self.speed * 8.0 / 1_000_000.0),
            ),
            (
                " 平均速度 ",
                self.average,
                GREEN,
                "本次测速平均".to_string(),
            ),
            (
                " 峰值速度 ",
                self.peak,
                Color::Rgb(192, 132, 252),
                "最高采样速度".to_string(),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(format!("{:.2} MB/s", value / 1_000_000.0))
                        .fg(color)
                        .bold(),
                    Line::from(""),
                    Line::from(detail).fg(MUTED),
                ])
                .centered()
                .block(panel(title)),
                cards[index],
            );
        }

        let points: Vec<_> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, value)| (i as f64, *value))
            .collect();
        let upper = self.history.iter().copied().fold(1.0_f64, f64::max) * 1.2;
        frame.render_widget(
            Chart::new(vec![Dataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::new().fg(ACCENT))
                .data(&points)])
            .block(panel(" 下载趋势 / 最近 120 次采样 "))
            .x_axis(
                Axis::default()
                    .style(Style::new().fg(BORDER))
                    .bounds([0.0, (HISTORY_LEN - 1) as f64]),
            )
            .y_axis(
                Axis::default()
                    .title("MB/s".fg(MUTED))
                    .style(Style::new().fg(BORDER))
                    .bounds([0.0, upper])
                    .labels(["0".to_string(), format!("{:.1}", upper)]),
            ),
            rows[2],
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    "分组  ".fg(MUTED),
                    self.group.fg(ACCENT),
                    format!(
                        "    并发上限  {}    重试  {}",
                        self.concurrency, self.errors
                    )
                    .into(),
                ]),
                Line::from(format!(
                    "已下载  {:.3} GB    运行时间  {:02}:{:02}:{:02}",
                    self.total as f64 / 1_000_000_000.0,
                    self.elapsed.as_secs() / 3600,
                    self.elapsed.as_secs() / 60 % 60,
                    self.elapsed.as_secs() % 60
                )),
                Line::from(self.address.as_str()).fg(MUTED),
            ])
            .block(panel(" 连接信息 "))
            .wrap(Wrap { trim: false }),
            rows[3],
        );
        frame.render_widget(
            Paragraph::new(
                " Q / Esc / Ctrl+C 退出    |    --cli 纯文本模式    |    1 MB = 1,000,000 B",
            )
            .fg(MUTED),
            rows[4],
        );
    }
}

fn panel(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BORDER))
        .title(title)
        .title_style(Style::new().fg(TEXT))
        .bg(PANEL)
}

pub struct Tui(DefaultTerminal);

impl Tui {
    pub fn new() -> io::Result<Self> {
        // try_init 安装 panic 恢复钩子；初始化失败也要还原已改变的终端状态。
        match ratatui::try_init() {
            Ok(terminal) => Ok(Self(terminal)),
            Err(error) => {
                ratatui::restore();
                Err(error)
            }
        }
    }

    pub fn should_quit(&self) -> io::Result<bool> {
        // 每帧只读一个事件，避免输入洪泛阻塞绘制和测速。
        if event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                return Ok(key.kind == KeyEventKind::Press
                    && (matches!(key.code, KeyCode::Char('q' | 'Q') | KeyCode::Esc)
                        || (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL))));
            }
        }
        Ok(false)
    }

    pub fn draw(&mut self, dashboard: &Dashboard) -> io::Result<()> {
        self.0.draw(|frame| dashboard.render(frame))?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn rates_history_and_layouts() {
        let mut dashboard = Dashboard::new("国内", 4);
        dashboard.sample(4_000_000, Duration::from_secs(2), Duration::from_secs(2));
        assert_eq!(dashboard.speed, 2_000_000.0);
        assert_eq!(dashboard.average, 2_000_000.0);
        assert!(dashboard.cli_line().contains("16.00 Mbps"));
        dashboard.sample(4_000_000, Duration::from_secs(3), Duration::from_secs(1));
        assert_eq!(dashboard.speed, 0.0);
        assert_eq!(dashboard.peak, 2_000_000.0);
        for second in 4..150 {
            dashboard.sample(
                4_000_000,
                Duration::from_secs(second),
                Duration::from_secs(1),
            );
        }
        assert_eq!(dashboard.history.len(), HISTORY_LEN);
        for (width, height) in [(100, 30), (60, 22), (40, 15), (10, 4), (1, 1), (0, 0)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| dashboard.render(frame)).unwrap();
            if width >= 40 {
                let contents: String = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect();
                assert!(contents.contains("SpeedTest"));
            }
        }
    }
}
