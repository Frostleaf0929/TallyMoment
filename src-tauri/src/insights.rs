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
    /// 归属任务：块的应用集合与任务"相关应用"重叠 ≥50% 时自动匹配
    pub task_id: Option<i64>,
    pub task_name: Option<String>,
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

/// 心流判定参数（设置页可调；阈值起点来自真实使用数据回算，见 2026-10-03 分册）
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowParams {
    /// 上下文应用数 K：按时间占比取前 K 个应用当作"这件事的上下文"
    pub context_apps: i64,
    /// 上下文一致度门槛（%）：前 K 个应用覆盖的活跃时间占比 ≥ 此值才算心流
    pub context_min: i64,
    /// 活跃占比门槛（%）：活跃时间 / 块跨度 ≥ 此值才算心流
    pub active_min: i64,
}

impl Default for FlowParams {
    fn default() -> Self {
        Self {
            context_apps: 4,
            context_min: 60,
            active_min: 80,
        }
    }
}

/// 从设置读判定参数（键：flow.context_apps / flow.context_min / flow.active_min）
pub fn flow_params(conn: &Connection) -> FlowParams {
    let g = |k: &str, d: i64, lo: i64, hi: i64| {
        crate::storage::get_setting(conn, k)
            .and_then(|v| v.parse::<i64>().ok())
            .map(|v| v.clamp(lo, hi))
            .unwrap_or(d)
    };
    FlowParams {
        context_apps: g("flow.context_apps", 4, 2, 6),
        context_min: g("flow.context_min", 60, 50, 90),
        active_min: g("flow.active_min", 80, 70, 95),
    }
}

/// 把前台区间归并成专注块并推断状态（真实数据回算后的上下文一致度版）。
/// 旧版按"切换次数/切换率"判定会把开发者式的多应用高频协同误判成碎片
/// （实测：4 小时真实工作段切换 389 次、占比 88%，被错杀）。现行规则：
/// ① 块 = 间隔 <5 分钟的使用归并（沿用）；
/// ② 心流 = 跨度 ≥ 门槛（自适应中位数，下限 15 分钟）
///        且 活跃占比 ≥ active_min%
///        且 上下文一致度 ≥ context_min%（按时间占比取前 context_apps 个应用，其覆盖占比）；
/// ③ 碎片 = 跨度 <10 分钟或活跃占比 <60%（大量空档/浅尝辄止）；
/// ④ 其余 = 专注。切换次数仅作展示，不再一票否决。
pub fn compute_blocks_with(
    segments: &[(i64, i64, String)],
    p: FlowParams,
) -> Vec<BlockView> {
    /// 块归并间隔：间隔 <5 分钟视为同一块
    const GAP: i64 = 300;
    /// 过场宽限：块内 ≤45s 的短段不计入切换展示
    const BLIP_SECS: i64 = 45;

    struct Acc {
        start_ts: i64,
        end_ts: i64,
        seconds: i64,
        apps: Vec<String>,
        /// 应用 → 活跃秒数（上下文一致度用）
        app_secs: Vec<(String, i64)>,
        switches: usize,
        last_app: String,
    }
    impl Acc {
        fn new(start_ts: i64, end_ts: i64, app: &str) -> Self {
            Self {
                start_ts,
                end_ts,
                seconds: end_ts - start_ts,
                apps: vec![app.to_string()],
                app_secs: vec![(app.to_string(), end_ts - start_ts)],
                switches: 0,
                last_app: app.to_string(),
            }
        }
    }

    let mut segs: Vec<(i64, i64, String)> = segments
        .iter()
        .filter(|(s, e, _)| e > s)
        .cloned()
        .collect();
    segs.sort_by_key(|(s, _, _)| *s);

    let mut accs: Vec<Acc> = Vec::new();
    for (s, e, app) in segs {
        match accs.last_mut() {
            Some(b) if s - b.end_ts < GAP => {
                if b.last_app != app && e - s > BLIP_SECS {
                    b.switches += 1; // 仅展示用
                }
                if !b.apps.contains(&app) {
                    b.apps.push(app.to_string());
                }
                match b.app_secs.iter_mut().find(|(a, _)| a == &app) {
                    Some(entry) => entry.1 += e - s,
                    None => b.app_secs.push((app.to_string(), e - s)),
                }
                b.last_app = app;
                b.end_ts = b.end_ts.max(e);
                b.seconds += e - s;
            }
            _ => accs.push(Acc::new(s, e, &app)),
        }
    }

    // 心流跨度门槛：样本多（≥6 块）时用本次范围里块跨度的中位数（下限 15 分钟）
    let mut span_sorted: Vec<i64> = accs.iter().map(|b| b.end_ts - b.start_ts).collect();
    span_sorted.sort_unstable();
    let median_span = if span_sorted.len() >= 6 { span_sorted[span_sorted.len() / 2] } else { 0 };
    let flow_span_min = if median_span > 0 { median_span.max(900) } else { 1500 };

    accs.iter()
        .map(|b| {
            let span = b.end_ts - b.start_ts;
            let ratio = if span > 0 { b.seconds as f64 / span as f64 } else { 1.0 };
            // 上下文一致度：按活跃时间取前 K 个应用，其覆盖占比
            let mut secs: Vec<i64> = b.app_secs.iter().map(|(_, s)| *s).collect();
            secs.sort_unstable_by(|a, b| b.cmp(a));
            let k = (p.context_apps.max(1) as usize).min(secs.len());
            let top: i64 = secs[..k].iter().sum();
            let coverage = if b.seconds > 0 { top as f64 / b.seconds as f64 } else { 0.0 };
            let state = if span >= flow_span_min
                && ratio >= p.active_min as f64 / 100.0
                && coverage >= p.context_min as f64 / 100.0
            {
                "flow"
            } else if span < 600 || ratio < 0.6 {
                "fragmented"
            } else {
                "focused"
            };
            BlockView {
                start_ts: b.start_ts,
                end_ts: b.end_ts,
                seconds: b.seconds,
                span,
                apps: b.apps.clone(),
                switches: b.switches,
                state: state.into(),
                task_id: None,
                task_name: None,
            }
        })
        .collect()
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

/// 洞察报表：days = 统计范围（1 = 今天，7 = 近 7 天，90 封顶）
pub fn report(conn: &Connection, days: i64) -> Result<InsightReport, String> {
    let date = storage::today_date();
    let day_start = storage::today_start_ts();
    let day_end = day_start + 86_400;
    let (days, range_start) = if days <= 0 {
        let min_ts: i64 = conn
            .query_row(
                "SELECT COALESCE(MIN(start_ts), ?1) FROM segments",
                [day_start],
                |r| r.get(0),
            )
            .map_err(|e| format!("读取最早记录失败: {e}"))?;
        let start = chrono::Local
            .timestamp_opt(min_ts, 0)
            .single()
            .map(|t| {
                t.date_naive()
                    .and_hms_opt(0, 0, 0)
                    .and_then(|nt| chrono::Local.from_local_datetime(&nt).single())
                    .map(|x| x.timestamp())
            })
            .flatten()
            .unwrap_or(day_start);
        (0, start)
    } else {
        (days.clamp(1, 90), day_start - (days.clamp(1, 90) - 1) * 86_400)
    };
    let n = if days <= 0 { 3650 } else { days as i32 };

    // 区间内全部前台区间：既归并专注块，也生成每日首末活动
    let segments = storage::today_segments(conn, range_start, day_end)?;
    let raw: Vec<(i64, i64, String)> = segments
        .iter()
        .map(|s| (s.start_ts, s.end_ts, s.app_name.clone()))
        .collect();
    let mut blocks = compute_blocks_with(&raw, flow_params(conn));
    blocks.retain(|b| b.span >= 60); // 过滤 <1 分钟的碎屑块

    // 心流归属：块的应用集合与任务"相关应用"重叠 ≥50% 即归属到重叠最高的任务
    let candidates: Vec<(i64, String, Vec<String>)> = conn
        .prepare("SELECT id, content, related_apps FROM tasks WHERE related_apps <> '[]'")
        .and_then(|mut st| {
            st.query_map([], |r| {
                let apps: Option<String> = r.get(2)?;
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    apps.and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
                        .unwrap_or_default(),
                ))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect::<Vec<_>>())
        })
        .unwrap_or_default();
    for b in &mut blocks {
        let mut best: Option<(f64, i64, String)> = None;
        for (id, content, apps) in &candidates {
            if apps.is_empty() {
                continue;
            }
            let hit = b
                .apps
                .iter()
                .filter(|a| apps.contains(&a.to_lowercase()))
                .count();
            let score = hit as f64 / b.apps.len() as f64;
            if score >= 0.5 && best.as_ref().map(|(s, _, _)| score > *s).unwrap_or(true) {
                best = Some((score, *id, content.clone()));
            }
        }
        if let Some((_, id, name)) = best {
            b.task_id = Some(id);
            b.task_name = Some(name);
        }
    }

    let daily = storage::recent_daily(conn, n)?;
    let input_daily = storage::input_daily(conn, n)?;
    let hourly14 = storage::recent_hourly(conn, n)?;

    // 区间内每天的活跃区间（首末活动时刻）
    let mut spans: Vec<SpanDay> = Vec::new();
    for s in &segments {
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

    let insights = build_insights(&blocks, &daily, &input_daily, &spans, &date, days);
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
    days: i64,
) -> Vec<Insight> {
    let mut out = Vec::new();
    let range_label = if days <= 0 {
        "总共".to_string()
    } else if days > 1 {
        format!("近 {days} 天")
    } else {
        "今天".to_string()
    };
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
            title: format!("{range_label}有 {} 段心流，共 {}", flow_blocks.len(), fmt_hm(total)),
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
            title: format!("{range_label}还没有成块的心流"),
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
                title: format!("碎片化占{range_label}使用的 {}%", frag_pct),
                analysis: "碎片块 = 频繁切换、单块不到 10 分钟的使用。零散消息与来回跳转是主因。".into(),
                suggestion: "试试把同类小事攒到固定时段批量处理，给大任务留整块时间。".into(),
                tone: "warn".into(),
            });
        } else {
            out.push(Insight {
                title: format!("{range_label}的节奏比较整"),
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
            let day_label = if days > 1 { "最近一天" } else { "今天" };
            let avg_label = if days > 1 { "此前日均" } else { "近三天日均" };
            out.push(Insight {
                title: format!("键入频率{}近期水平", if diff >= 15 { "高于" } else if diff <= -15 { "低于" } else { "接近" }),
                analysis: format!(
                    "{} {} 次，{} {} 次（{}%）。只计次数，不记录内容。",
                    day_label, today_keys, avg_label, avg, diff.abs()
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
    use super::compute_blocks_with;
    use super::FlowParams;

    #[test]
    fn report_respects_days_range() {
        let dir = std::env::temp_dir().join(format!("tallymoment-insights-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("insights-range.db");
        let _ = std::fs::remove_file(&p);
        let conn = crate::storage::open(&p).unwrap();
        let day = crate::storage::today_start_ts();
        conn.execute("INSERT INTO apps(name) VALUES('a.exe')", [])
            .unwrap();
        let app_id = conn.last_insert_rowid();
        // 昨天 1 小时一段 + 今天 30 分钟一段
        for (s, e) in [(day - 86_400 + 3_600, day - 86_400 + 7_200), (day + 3_600, day + 5_400)] {
            conn.execute(
                "INSERT INTO segments(app_id, start_ts, end_ts, title) VALUES(?1, ?2, ?3, ?4)",
                rusqlite::params![app_id, s, e, "t"],
            )
            .unwrap();
        }
        // 今天：只含今天那段
        let r1 = super::report(&conn, 1).unwrap();
        assert_eq!(r1.blocks.len(), 1);
        assert!(r1.blocks[0].start_ts >= day);
        // 近 7 天：两段都在
        let r7 = super::report(&conn, 7).unwrap();
        assert_eq!(r7.blocks.len(), 2);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn blocks_merge_within_gap_and_infer_states() {
        // 10:00-10:30（30 分钟）+ 2 分钟后接续 8 分钟 → 合并为 flow 块
        // 11:00-11:05（间隔 20 分钟、跨度 5 分钟）→ 独立碎片块
        let segs = vec![
            (3600, 5400, "a.exe".to_string()),
            (5520, 6000, "b.exe".to_string()),
            (7200, 7500, "a.exe".to_string()),
        ];
        let b = compute_blocks_with(&segs, FlowParams::default());
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].state, "flow");
        assert_eq!(b[0].seconds, 2280);
        assert_eq!(b[0].span, 2400);
        assert_eq!(b[0].switches, 1);
        assert_eq!(b[0].apps, vec!["a.exe", "b.exe"]);
        assert_eq!(b[1].state, "fragmented");
    }

    #[test]
    fn idle_gap_inside_block_prevents_flow() {
        // 跨度 5 分钟的独立块：span < 600 秒 → 碎片；此处验证占比逻辑不误伤正常块
        // 更长的稀释块在 report 层被 retain(span>=60) 与占比条件拦住
        let segs = vec![(3600, 3900, "a.exe".to_string())];
        let b = compute_blocks_with(&segs, FlowParams::default());
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].state, "fragmented");
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
        let b = compute_blocks_with(&segs, FlowParams::default());
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].state, "fragmented");
    }

    #[test]
    fn multi_app_coordination_is_flow() {
        // 40 分钟三应用协同：IDE 20min → 30s 过场查资料 → IDE 8min → 笔记 2min → IDE 10min
        // 旧规则（整块切换≤3 且把过场也计切换）会误杀；新规则判为心流
        let segs = vec![
            (0, 1200, "ide.exe".to_string()),
            (1201, 1231, "web.exe".to_string()),
            (1232, 1712, "ide.exe".to_string()),
            (1713, 1833, "note.exe".to_string()),
            (1834, 2434, "ide.exe".to_string()),
        ];
        let b = compute_blocks_with(&segs, FlowParams::default());
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].state, "flow");
        assert_eq!(b[0].switches, 3);
        assert_eq!(b[0].apps, vec!["ide.exe", "web.exe", "note.exe"]);
    }

    #[test]
    fn low_context_coverage_is_not_flow() {
        // 40 分钟、8 个应用均分 → top4 覆盖 50% < 门槛 → 专注（高频乱切不等于心流）
        let mut segs = Vec::new();
        let mut t = 0i64;
        for i in 0..8 {
            segs.push((t, t + 300, format!("app{}.exe", i)));
            t += 300;
        }
        let b = compute_blocks_with(&segs, FlowParams::default());
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].state, "focused");
    }

    #[test]
    fn short_detour_within_a_minute_does_not_break_flow() {
        // "1 分钟内切回来不算打断"：a 15min → b 40s 过场 → a 15min，只计 1 次切换
        let segs = vec![
            (0, 900, "a.exe".to_string()),
            (901, 941, "b.exe".to_string()),
            (942, 1842, "a.exe".to_string()),
        ];
        let b = compute_blocks_with(&segs, FlowParams::default());
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].state, "flow");
        assert_eq!(b[0].switches, 1);
    }
}
