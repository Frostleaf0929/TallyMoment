//! 洞察引擎（批次 3 升级）：专注块重构 + 心流推断（规则版，界面标注"推断"）+ 使用频率/区间分析。
//! 所有结论来自本地统计规则，随数据量增长而更有参考价值。
use rusqlite::Connection;
use chrono::TimeZone as _;
use serde::Serialize;

use crate::storage;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Insight {
    pub title: String,
    /// 数据说了什么
    pub analysis: String,
    /// 建议怎么做
    pub suggestion: String,
    /// good | warn | info
    pub tone: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockView {
    pub start_ts: i64,
    pub end_ts: i64,
    /// 实际活跃秒数（不含空闲间隙）
    pub seconds: i64,
    /// 墙钟跨度
    pub span: i64,
    pub apps: Vec<String>,
    pub switches: usize,
    /// flow | focused | fragmented（规则推断）
    pub state: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanDay {
    pub date: String,
    pub first_ts: i64,
    pub last_ts: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightReport {
    pub blocks: Vec<BlockView>,
    pub daily: Vec<(String, i64)>,
    pub input_daily: Vec<storage::InputDay>,
    pub spans: Vec<SpanDay>,
    /// 近 14 天按小时聚合的使用时长（作息分布图）
    pub hourly14: Vec<storage::HourSlice>,
    pub flow_seconds: i64,
    pub focused_seconds: i64,
    pub fragmented_seconds: i64,
    pub insights: Vec<Insight>,
}

/// 把前台区间归并成专注块：间隔 < 5 分钟视为同一块（纯函数，可测）
pub fn compute_blocks(segments: &[(i64, i64, String)]) -> Vec<BlockView> {
    let mut segs: Vec<(i64, i64, String)> = segments
        .iter()
        .filter(|(s, e, _)| e > s)
        .cloned()
        .collect();
    segs.sort_by_key(|(s, _, _)| *s);

    const GAP: i64 = 300;
    let mut out: Vec<BlockView> = Vec::new();
    for (s, e, app) in segs {
        match out.last_mut() {
            Some(b) if s - b.end_ts < GAP => {
                b.end_ts = b.end_ts.max(e);
                b.span = b.end_ts - b.start_ts;
                b.seconds += e - s;
                b.switches += 1;
                if !b.apps.contains(&app) {
                    b.apps.push(app);
                }
            }
            _ => out.push(BlockView {
                start_ts: s,
                end_ts: e,
                seconds: e - s,
                span: e - s,
                apps: vec![app],
                switches: 0,
                state: String::new(),
            }),
        }
    }
    // 心流推断（规则版）：跨度 ≥25 分钟且少切换 = 心流；
    // 切换速率 ≥0.4 次/分钟 或跨度 <10 分钟 = 碎片；其余 = 专注
    for b in &mut out {
        let rate = b.switches as f64 / (b.span as f64 / 60.0);
        b.state = if b.span >= 1500 && b.switches <= 3 {
            "flow"
        } else if b.span < 600 || rate >= 0.4 {
            "fragmented"
        } else {
            "focused"
        }
        .into();
    }
    out
}

fn fmt_hm(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h} 小时 {m} 分")
    } else {
        format!("{m} 分钟")
    }
}

fn hhmm(ts: i64) -> String {

    chrono::Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|t| t.format("%H:%M").to_string())
        .unwrap_or_default()
}

pub fn report(conn: &Connection) -> Result<InsightReport, String> {
    let date = storage::today_date();
    let day_start = storage::today_start_ts();
    let day_end = day_start + 86_400;
    let week_ago = day_start - 13 * 86_400;

    let today_segments = storage::today_segments(conn, day_start, day_end)?;
    let raw: Vec<(i64, i64, String)> = today_segments
        .iter()
        .map(|s| (s.start_ts, s.end_ts, s.app_name.clone()))
        .collect();
    let mut blocks = compute_blocks(&raw);
    blocks.retain(|b| b.span >= 60); // 过滤 <1 分钟的碎屑块

    let daily = storage::recent_daily(conn, 14)?;
    let input_daily = storage::input_daily(conn, 14)?;
    let hourly14 = storage::recent_hourly(conn, 14)?;

    // 近 14 天活跃区间（首末活动时刻）
    let all_segments = storage::today_segments(conn, week_ago, day_end)?;
    let mut spans: Vec<SpanDay> = Vec::new();
    for s in &all_segments {
        let d = chrono::Local
            .timestamp_opt(s.start_ts, 0)
            .single()
            .map(|t| t.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        match spans.last_mut() {
            Some(sd) if sd.date == d => {
                sd.last_ts = sd.last_ts.max(s.end_ts);
            }
            _ => spans.push(SpanDay {
                date: d,
                first_ts: s.start_ts,
                last_ts: s.end_ts,
            }),
        }
    }

    let flow_seconds = blocks
        .iter()
        .filter(|b| b.state == "flow")
        .map(|b| b.seconds)
        .sum();
    let focused_seconds = blocks
        .iter()
        .filter(|b| b.state == "focused")
        .map(|b| b.seconds)
        .sum();
    let fragmented_seconds = blocks
        .iter()
        .filter(|b| b.state == "fragmented")
        .map(|b| b.seconds)
        .sum();

    let insights = build_insights(&blocks, &daily, &input_daily, &spans, &date);
    Ok(InsightReport {
        blocks,
        daily,
        input_daily,
        spans,
        hourly14,
        flow_seconds,
        focused_seconds,
        fragmented_seconds,
        insights,
    })
}

fn build_insights(
    blocks: &[BlockView],
    daily: &[(String, i64)],
    input_daily: &[storage::InputDay],
    spans: &[SpanDay],
    today: &str,
) -> Vec<Insight> {
    let mut out = Vec::new();
    let today_total: i64 = daily
        .iter()
        .find(|(d, _)| d == today)
        .map(|(_, s)| *s)
        .unwrap_or(0);

    // 1. 心流
    let flow_blocks: Vec<&BlockView> = blocks.iter().filter(|b| b.state == "flow").collect();
    if !flow_blocks.is_empty() {
        let total: i64 = flow_blocks.iter().map(|b| b.seconds).sum();
        let best = flow_blocks.iter().max_by_key(|b| b.span).unwrap();
        let app = best
            .apps
            .first()
            .map(|a| a.trim_end_matches(".exe").to_string())
            .unwrap_or_default();
        out.push(Insight {
            title: format!("今天有 {} 段心流，共 {}", flow_blocks.len(), fmt_hm(total)),
            analysis: format!(
                "最深的一段 {} 在 {}–{}，持续 {}，主要在 {}。",
                fmt_hm(best.seconds),
                hhmm(best.start_ts),
                hhmm(best.end_ts),
                fmt_hm(best.span),
                app
            ),
            suggestion: "心流块最珍贵：类似的深度工作优先安排到你最容易进入状态的时段。".into(),
            tone: "good".into(),
        });
    } else if today_total > 900 {
        out.push(Insight {
            title: "今天还没有成块的心流".into(),
            analysis: "记录里有使用，但没有持续 25 分钟以上、少切换的整块时间。".into(),
            suggestion: "挑一件事，给自己一个不被打断的 25 分钟试试。".into(),
            tone: "info".into(),
        });
    }

    // 2. 专注结构
    let total: i64 = blocks.iter().map(|b| b.seconds).sum();
    if total > 900 {
        let frag_pct = (fragmented_share(blocks) * 100.0).round() as i64;
        if frag_pct >= 35 {
            out.push(Insight {
                title: format!("碎片化占今日使用的 {}%", frag_pct),
                analysis: "碎片块 = 频繁切换、单块不到 10 分钟的使用。零散消息与来回跳转是主因。".into(),
                suggestion: "试试把同类小事攒到固定时段批量处理，给大任务留整块时间。".into(),
                tone: "warn".into(),
            });
        } else {
            out.push(Insight {
                title: "今天的节奏比较整".into(),
                analysis: format!("碎片化仅占 {}%，多数时间处在成块的使用中。", frag_pct),
                suggestion: "保持这个节奏；在块与块之间安排真正的休息。".into(),
                tone: "good".into(),
            });
        }
    }

    // 3. 使用区间
    if spans.len() >= 3 {
        let mut firsts: Vec<i64> = Vec::new();
        let mut lasts: Vec<i64> = Vec::new();
        for sd in spans {
            let f = sd.first_ts - (sd.first_ts / 86_400) * 86_400;
            let l = sd.last_ts - (sd.last_ts / 86_400) * 86_400;
            firsts.push(f);
            lasts.push(l);
        }
        firsts.sort_unstable();
        lasts.sort_unstable();
        let mid = firsts.len() / 2;
        let first = firsts.get(mid).copied().unwrap_or(0);
        let last = lasts.get(mid).copied().unwrap_or(0);
        let to_hm = |sec_of_day: i64| format!("{:02}:{:02}", sec_of_day / 3600, (sec_of_day % 3600) / 60);
        let night = last >= 23 * 3600 || first <= 6 * 3600;
        out.push(Insight {
            title: format!("活跃区间通常在 {} – {}", to_hm(first), to_hm(last)),
            analysis: format!("近 {} 天的作息中位水平。", spans.len()),
            suggestion: if night {
                "区间里包含深夜或清晨，注意睡眠对专注力的影响。".into()
            } else {
                "区间稳定，把最重要的任务放在区间的前半段效率更好。".into()
            },
            tone: if night { "warn".into() } else { "info".into() },
        });
    }

    // 4. 使用频率（键鼠趋势）
    if input_daily.len() >= 4 {
        let today_keys = input_daily.last().map(|d| d.keys).unwrap_or(0);
        let prev: Vec<i64> = input_daily
            .iter()
            .rev()
            .skip(1)
            .take(3)
            .map(|d| d.keys)
            .collect();
        let avg = prev.iter().sum::<i64>() / prev.len().max(1) as i64;
        if avg > 500 {
            let diff = ((today_keys - avg) as f64 / avg as f64 * 100.0).round() as i64;
            out.push(Insight {
                title: format!("键入频率{}近期水平", if diff >= 15 { "高于" } else if diff <= -15 { "低于" } else { "接近" }),
                analysis: format!(
                    "今天 {} 次，近三天日均 {} 次（{}%）。只计次数，不记录内容。",
                    today_keys, avg, diff.abs()
                ),
                suggestion: "键入密度反映动手强度，配合时间线看它在什么时段发生。".into(),
                tone: "info".into(),
            });
        }
    }

    // 5. 连续记录
    let mut streak_days = 0i64;
    let mut expect = chrono::Local::now().format("%Y-%m-%d").to_string();
    for (d, _) in daily.iter().rev() {
        if *d == expect {
            streak_days += 1;
        } else {
            break;
        }
        let prev = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .and_then(|x| x.pred_opt())
            .map(|x| x.format("%Y-%m-%d").to_string());
        match prev {
            Some(p) => expect = p,
            None => break,
        }
    }
    if streak_days >= 2 {
        out.push(Insight {
            title: format!("连续记录 {} 天", streak_days),
            analysis: "持续记录让趋势越来越准。".into(),
            suggestion: "今天结束后记得让拾刻保持运行，别断了连击。".into(),
            tone: "good".into(),
        });
    }

    if out.is_empty() {
        out.push(Insight {
            title: "数据还在积累".into(),
            analysis: "拾刻在后台安静记录。".into(),
            suggestion: "用半天到一天后，这里会出现真正的分析。".into(),
            tone: "info".into(),
        });
    }
    out
}

fn fragmented_share(blocks: &[BlockView]) -> f64 {
    let total: i64 = blocks.iter().map(|b| b.seconds).sum();
    if total <= 0 {
        return 0.0;
    }
    let frag: i64 = blocks
        .iter()
        .filter(|b| b.state == "fragmented")
        .map(|b| b.seconds)
        .sum();
    frag as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use super::compute_blocks;

    #[test]
    fn blocks_merge_within_gap_and_infer_states() {
        // 10:00-10:30（30 分钟）+ 2 分钟后接续 8 分钟 → 合并为 flow 块
        // 11:00-11:05（间隔 20 分钟、跨度 5 分钟）→ 独立碎片块
        let segs = vec![
            (3600, 5400, "a.exe".to_string()),
            (5520, 6000, "b.exe".to_string()),
            (7200, 7500, "a.exe".to_string()),
        ];
        let b = compute_blocks(&segs);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].state, "flow");
        assert_eq!(b[0].seconds, 2280);
        assert_eq!(b[0].span, 2400);
        assert_eq!(b[0].switches, 1);
        assert_eq!(b[0].apps, vec!["a.exe", "b.exe"]);
        assert_eq!(b[1].state, "fragmented");
    }

    #[test]
    fn long_multi_switch_block_is_fragmented() {
        // 40 分钟跨度内切了 12 次 → fragmented
        let mut segs = Vec::new();
        let mut t = 0i64;
        for i in 0..12 {
            segs.push((t, t + 60, format!("app{}.exe", i % 3)));
            t += 120;
        }
        let b = compute_blocks(&segs);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].state, "fragmented");
    }
}
