# HANDOFF.md · 拾刻 TallyMoment 交接文档

> 生成时间：2026-09-26 23:20 ｜ 用途：切换对话时给新会话的完整上下文

---

## 一、项目是什么

**拾刻 TallyMoment** — Windows 绿色免安装软件：前台应用使用时长记录（聚焦）、今日事件与效率回顾、可查看的有趣数据、极简现代 UI、桌宠、清单提醒。本地优先，不上传任何数据。

- **用户**：Frostleaf0929（零基础开发者，AI 负责写码、用户负责测试/反馈/存档确认）
- **GitHub**：https://github.com/Frostleaf0929/TallyMoment（**私有**）
- **开发路径**：`E:\04_Archives\01_Personal\Ex - Project\TallyMoment`
- **日志记录路径**：`E:\04_Archives\01_Personal\# ZCode\拾刻 - TallyMoment`（WORKLOG.md 索引 + logs/ 分册；此路径含 `#` 与中文，**不能放 Rust 工程**）

## 二、技术栈与架构

| 层 | 技术 |
|---|---|
| 框架 | **Tauri 2**（Rust 核心 + WebView2 渲染） |
| 前端 | **Vue 3 + TypeScript + Vite + Pinia + Naive UI + ECharts** |
| Rust 依赖 | rusqlite(bundled), active-win-pos-rs, chrono, windows(0.62), tauri-plugin-dialog, tauri-plugin-autostart, zip, rust_xlsxwriter |
| 数据库 | SQLite（WAL 模式），存 exe 旁 `Data/` 目录 |
| 图标 | Solar Line Duotone（CC BY 4.0，via Iconify） |
| 协议 | MIT |

**Rust 模块清单**（src-tauri/src/）：
| 文件 | 职责 |
|---|---|
| `lib.rs` | Tauri Builder 装配、全部命令注册、托盘、退出钩子 |
| `storage.rs` | SQLite 全部读写（apps/segments/hourly_stats/daily_stats/tasks/reminder_rules/input_stats/settings/checklist）；Tai 对齐导出；JSON 导出/恢复；删除 |
| `tracker.rs` | 每秒追踪循环：前台窗口→会话→SQLite；空闲检测（GetLastInputInfo）；切换防抖（2 秒）；提醒调度；键鼠 5 秒增量落库 |
| `input_hook.rs` | Windows 低级键鼠钩子（WH_KEYBOARD_LL/WH_MOUSE_LL），只发"有按键/有点击"+ VK 码（不落库不显示），计数器 |
| `reminder.rs` | 提醒小窗：Payload 结构化、PENDING 队列确定性投递、窗口自适应高度 |
| `pet_settings.rs` | 桌宠设置持久化（settings 表）+ 应用到窗口；Mver 模型导入（文件夹/ZIP）/列表/删除/config 解析 |
| `insights.rs` | 规则版洞察引擎：专注块归并（<5min 间隔）→ 心流/专注/碎片推断 → 分析+建议 |

**前端结构**（src/）：
| 路径 | 职责 |
|---|---|
| `App.vue` | 侧边栏外壳（可收展）+ 按窗口标签分流（main/reminder/pet）+ 品牌语言/主题/强调色/毛玻璃 |
| `pages/TodayPage.vue` | 今日：当前专注大分区 + 统计卡 + 热力时间线 + 24h 分布 + 排行 |
| `pages/HistoryPage.vue` | 历史：近 14 天日期按钮 → 选中日完整报表 |
| `pages/InsightsPage.vue` | 洞察：三态摘要 + 心流块时间线 + 频率折线 + 作息带 + 分析建议卡 |
| `pages/TodoPage.vue` | 待办：完成率四卡 + 任务看板 + 完成用时分布 + 提醒规则 |
| `pages/SettingsPage.vue` | 个性化：主题/强调色/毛玻璃/自启/桌宠显隐 + 品牌语言 |
| `components/` | HeatTimeline, HourlyChart, AppRanking, DayTimeline(弃用), TaskBoard, ReminderRules, ChecklistCard(弃用), DataCard, PetView, ToastStack, Icon, ReminderCard(弃用), PetSettingsPage |

## 三、已完成里程碑

| 里程碑 | 内容 | 存档 |
|---|---|---|
| M0 | Tauri 2 + Vue3 脚手架，窗口+托盘+关窗隐藏 | 5203f29 |
| M1 | 前台追踪+空闲检测+SQLite 落库+托盘今日时长 | 0d4d8d4 |
| M2 | 今日回顾仪表盘（时间线/分布/排行/统计卡） | 9fe7c75 |
| M3 | 清单提醒（调度器+右下角弹窗+防重复） | 149ce36 |
| M4 | Tai 对齐导出+JSON 导出/恢复（cargo test） | caba6be |
| M5 | 桌宠窗（兔子洞模型+全局钩子+托盘开关） | 6a164f8 |
| M6+M7 | 键鼠统计+规则洞察+侧边栏多页 | d03fbff |
| 批次 1 | UI 底座（token/Solar 图标/毛玻璃/侧栏收展/个性化/今日重构/追踪防抖） | 79bdc6e |
| 批次 2 | 待办与提醒中心（任务看板/完成率/提醒规则/多卡小窗） | c1a9b73 |
| 批次 3 | 洞察升级（专注块/心流推断/频率区间/双段式） | 951ed6b |
| 批次 4 | Tai 对齐导出+删除数据+自启+5 秒落库 | b86a829 |
| 批次 4 补 | xlsx 表格导出 | a0d52a0 |
| 批次 5 | 桌宠增强（设置页/Mver 导入/模型管理） | （最新） |
| 图标+品牌语言 | SVG 重设计图标 + 中/英切换 | （最新） |

**测试**：cargo test **10/10 通过**（Tai 导入去重/Tai 导出/JSON 回环/非 Tai 拒绝/专注块合并/碎片判定/模型导入回环/ZIP 导入/非模型拒绝/设置回环）

## 四、如何构建与运行

```powershell
cd E:\04_Archives\01_Personal\Ex - Project\TallyMoment
pnpm install          # 首次
pnpm tauri dev        # 开发运行（Rust 编译 + Vite + 窗口）
pnpm build            # 前端构建
pnpm tauri build      # 生产打包（NSIS/MSI/exe）
```

前置：Node.js ≥18 + pnpm + Rust stable-msvc（rustc 1.98.1）+ VS Build Tools + WebView2

## 五、已知问题与限制

| 问题 | 影响 | 计划 |
|---|---|---|
| 提醒窗与桌宠右下角重叠 | 视觉 | 批次后续调位置偏好 |
| 点击穿透开启后桌宠不响应拖动 | 交互 | 设置页有提示"回此页关闭" |
| 模型标准模式右手层不渲染 | 适配 | 单手模型只有 hand/N，后续补 |
| Live2D（cat_model/）不渲染 | 适配 | 需 Cubism SDK 授权，暂搁 |
| 兔子洞素材授权 | 发布 | 个人使用 OK；公开发布需模型作者授权 |
| Solar 图标 CC BY 4.0 | 发布 | 需在关于页/README 署名 |
| 心流阈值经验值 | 准确性 | 待使用反馈调参 |
| tauri dev 编辑中途触发失败构建 | 开发 | 重启 dev 即可；排查前端问题前先确认二进制新鲜度 |

## 六、下一步（优先级从高到低）

1. **细节打磨**（用户逐项纠正——9 点反馈中尚未覆盖的细节）
2. **网站追踪**（对标 Tai 的 WebSites 表，需浏览器扩展配合）
3. **提醒增强**：Catrace 的"仅在活跃时计时"（interval 模式挂机不累计）
4. **桌宠进阶**：按内容上报尺寸的逐像素命中、Live2D 评估
5. **数据可视化打磨**： Tai 式环形表盘（对标 Catrace heatmap）
6. **公开发布准备**：ICO 精修、NSIS 安装包签名、兔子洞/Solar 署名

## 七、环境详情

| 组件 | 版本/路径 |
|---|---|
| Windows | 11 24H2 (10.0.26100) |
| Rust | 1.98.1 stable-msvc |
| Node.js | 已装（pnpm v10.34.5） |
| VS Build Tools | 2022 MSVC v143 (14.44) |
| WebView2 | 153.0.4234.48 |
| GitHub | Frostleaf0929（gh CLI 已认证，keyring） |

## 八、关键设计决策（新会话需知）

1. **两仓库分工**：开发仓库（代码）+ 记录仓库（WORKLOG/logs）；记录仓库历史已并入开发仓库（统一档案）；此后每里程碑我同步记录快照到开发仓库
2. **追踪防抖**：新应用需连续 2 秒前台才确认切换（过滤 Alt-Tab 瞬态）
3. **键鼠只记次数**：钩子发 VK 码给桌宠选帧（实时、不落库不显示），入库只有总次数
4. **提醒投递**：PENDING 队列确定性投递（事件重试不可靠）；前端就绪后主动拉取
5. **模型管理**：Mver 格式，id=源文件夹/zip 主干名，同名拒绝；内置兔子洞 = keyboard 模式分层素材
6. **品牌语言**：中/英/双语三选，localStorage `ui.brandLang`，影响侧栏品牌 + 窗口标题
7. **数据目录**：exe 旁 `Data/`（不可写回退 %APPDATA%\TallyMoment\Data）
8. **Tai 对齐**：data.db 三表（App/DailyLog/HoursLog），Tai 自检自动补齐；datetime = "YYYY-MM-DD HH:MM:SS" 文本
9. **代提交惯例**：用户可能忘记提交，AI 按批次代提交（提交信息注明"代提交已披露"）
