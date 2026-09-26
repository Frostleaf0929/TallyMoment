//! 规则版每日洞察（M7）：纯本地统计规则生成建议，不依赖云端 AI。
//! 每条建议带 tone（good/warn/info），数据不足时给出温和提示而不是硬编结论。
use rusqlite::Connection;
use serde::Serialize;

use crate::storage;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Insight {
    pub title: String,
    pub detail: String,
    /// good | warn | info
    pub tone: String,
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

pub fn compute(conn: &Connection) -> Result<Vec<Insight>, String> {
    let date = storage::today_date();
    let day_start = storage::today_start_ts();
    let day_end = day_start + 86_400;

    let apps = storage::today_app_usage(conn, &date)?;
    let total: i64 = apps.iter().map(|a| a.seconds).sum();
    let hourly = storage::today_hourly(conn, &date)?;
    let segments = storage::today_segments(conn, day_start, day_end)?;
    let (keys, clicks) = storage::input_for_date(conn, &date)?;
    let recent = storage::recent_daily(conn, 8)?;

    let mut out: Vec<Insight> = Vec::new();

    if total <= 0 && recent.iter().all(|(d, _)| *d != date) {
        out.push(Insight {
            title: "数据还在积累".into(),
            detail: "今天还没有记录。拾刻会在后台安静记录，用一会儿再回来看洞察。".into(),
            tone: "info".into(),
        });
        return Ok(out);
    }

    // 1. 总时长 vs 近 7 日均值（不含今天）
    let past: Vec<i64> = recent
        .iter()
        .filter(|(d, _)| *d != date)
        .map(|(_, s)| *s)
        .collect();
    if !past.is_empty() {
        let avg = past.iter().sum::<i64>() / past.len() as i64;
        if avg > 300 {
            let diff = ((total - avg) as f64 / avg as f64 * 100.0).round() as i64;
            let (tone, word) = if diff >= 15 {
                ("warn", "高于")
            } else if diff <= -15 {
                ("info", "低于")
            } else {
                ("good", "接近")
            };
            out.push(Insight {
                title: format!("今日 {fmt}", fmt = fmt_hm(total)),
                detail: if word == "接近" {
                    format!("与近 {} 天均值（{}）基本持平，节奏稳定。", past.len(), fmt_hm(avg))
                } else {
                    format!("{}近 {} 天均值（{}）约 {}%。", word, past.len(), fmt_hm(avg), diff.abs())
                },
                tone: tone.into(),
            });
        }
    }

    // 2. 专注高峰时段
    if !hourly.is_empty() {
        let mut by_hour: std::collections::HashMap<i32, i64> = std::collections::HashMap::new();
        for h in &hourly {
            *by_hour.entry(h.hour).or_insert(0) += h.seconds;
        }
        if let Some((peak_hour, peak_secs)) = by_hour.iter().max_by_key(|(_, s)| **s) {
            if *peak_secs >= 600 {
                out.push(Insight {
                    title: format!("{} 点是今天的高峰", peak_hour),
                    detail: format!(
                        "该小时使用了约 {}，深工作尽量安排在这个时段。",
                        fmt_hm(*peak_secs)
                    ),
                    tone: "info".into(),
                });
            }
        }
    }

    // 3. 应用切换碎片化
    let switches = segments.len();
    if switches >= 40 {
        out.push(Insight {
            title: "节奏偏碎片".into(),
            detail: format!(
                "今天切换应用 {} 次。试着把同类任务放进整块时间，减少来回跳。", switches
            ),
            tone: "warn".into(),
        });
    } else if switches >= 8 {
        out.push(Insight {
            title: "节奏不错".into(),
            detail: format!("今天切换应用 {} 次，专注块保持得可以。", switches),
            tone: "good".into(),
        });
    }

    // 4. 久坐提醒：最长单段使用
    if let Some(longest) = segments
        .iter()
        .map(|s| s.end_ts - s.start_ts)
        .max()
    {
        if longest >= 5400 {
            let app = segments
                .iter()
                .find(|s| s.end_ts - s.start_ts == longest)
                .map(|s| s.app_name.trim_end_matches(".exe").to_string())
                .unwrap_or_default();
            out.push(Insight {
                title: "该起来活动了".into(),
                detail: format!(
                    "最长连续使用 {}（{}）未休息，起身倒杯水吧。",
                    app,
                    fmt_hm(longest)
                ),
                tone: "warn".into(),
            });
        }
    }

    // 5. 使用集中度：前 3 应用占比
    if apps.len() >= 3 && total > 0 {
        let top3: i64 = apps.iter().take(3).map(|a| a.seconds).sum();
        let pct = (top3 as f64 / total as f64 * 100.0).round() as i64;
        let names: Vec<String> = apps
            .iter()
            .take(3)
            .map(|a| a.display_name.trim_end_matches(".exe").to_string())
            .collect();
        out.push(Insight {
            title: format!("前 3 名占 {}%", pct),
            detail: format!("时间主要花在：{}", names.join("、")),
            tone: "info".into(),
        });
    }

    // 6. 键鼠活跃（M6 数据）
    if keys > 0 || clicks > 0 {
        out.push(Insight {
            title: format!("键入 {} 次 · 点击 {} 次", keys, clicks),
            detail: "只统计次数，不记录内容。键鼠密度仅供回顾，不代表效率。".into(),
            tone: "info".into(),
        });
    }

    if out.is_empty() {
        out.push(Insight {
            title: "数据还在积累".into(),
            detail: "再使用一段时间，洞察会越来越有参考价值。".into(),
            tone: "info".into(),
        });
    }
    Ok(out)
}
