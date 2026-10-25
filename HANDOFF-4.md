# HANDOFF-4.md · 拾刻 TallyMoment 交接文档（第五版）

> 生成时间：2026-10-26 ｜ 用途：切换对话/换机器时给新会话与未来的自己的完整上下文
> 上一版交接见 `HANDOFF-3.md`（截至 0.3.0/0.3.1/0.3.2 发布、应用归族）。
> 本版覆盖其后全部工作：**七轮细节打磨（打磨1~7）+ v0.3.3 发布**。

---

## 一、项目与路径（以本版为准）

**拾刻 TallyMoment** — Windows 桌面时间记录与效率回顾工具（本地优先，不联网）。
三个核心功能：使用时长记录 ｜ 任务督促（待办+固定事项+提醒）｜ 分析与建议（日志+洞察）；另加**原子岛**（常驻悬浮胶囊）。

- **开发仓库（代码，PUBLIC）**：`E:\04_Archives\01_Personal\_ZCode\TallyMoment`
- **记录仓库（不写代码）**：`E:\04_Archives\01_Personal\_ZCode\拾刻 - TallyMoment`（WORKLOG.md 索引 + logs/ 分册；**注意已从 `# ZCode` 改名为 `_ZCode`，HANDOFF-3 里的旧路径作废**）
- **远端**：`https://github.com/Frostleaf0929/TallyMoment`（公开，默认分支 `master`）
- **当前版本**：**0.3.3**（package.json / Cargo.toml / tauri.conf.json / 设置页 四处已同步）
- **本地发布归档**：`release/0.3.3/`（gitignored；setup + portable + RELEASE_NOTES）
- **用户实际运行的便携版**：`release/0.3.2/Data/tallymoment.db`（**live 数据库在这里！**排查线上数据问题先看它，别只看 dev 库）

## 二、本轮做了什么（打磨1~7，全部已提交）

### 打磨1（a79793c）重复任务停止与去重 + 静默启动 + 提醒生效范围 + 日志总览
- `tasks.repeat_stopped` 列 + `task_stop_repeat`：停止=不再生成以后实例，历史全保留可恢复；删除模板=停止+隐藏
- TaskBoard 实例合并成一行；habit_grid 纳入已停止/已删除模板（stopped 标记）
- 编辑模板同步未完成实例（task_update 传播，已完成不动）；实例 due 时区修复（and_utc→Local，"满屏 17:00"根因）
- due_tasks 只提醒今天到期；主窗 visible:false + `--autostart` 标记，开机默认静默；`startup_show_get/set`
- 提醒规则 `start_date/end_date` 生效区间（调度按今天过滤，导入导出兼容）
- 日志历史总览（后被打磨2重做）

### 打磨2（6304227）三卡统一「列表+次级编辑」+ 清单子页 + 画廊
- 任务/提示/日志三卡：进卡先看列表、右上新建按钮、新建/编辑进次级表单
- 日志：列表默认主体+搜索，Notion 式画廊（首图封面懒加载≤60篇）；详细页「事项」→「清单」（总览/任务/提示/日志四子页）
- uiState 扩展 `view:"checklist"+sub`；待办三卡弹层头部跳转胶囊
- 习惯追踪删时间轴视图、滚动优化（去圆点动效+contain）

### 打磨3（27f6709）样式统一收尾
- `.edit` 类名冲突修复（表单输入 width:100% 拉满行内编辑按钮）；`.mini` svg 尺寸修复
- DayDetail `mode="stats"`；NotesPanel `defaultView`；备注淡化 `.hint`；`〈〉`→`《》`（后又改「」）
- 软件名英文：`app_icon::friendly_name`（FileDescription 空→回退 ProductName，116 个应用受益）
- **空闲阈值设置项** `track.idle_min`（0~120 分钟）：拾刻<Tai 的根因=60s 空闲剔除写死

### 打磨4（343c461）下拉弹窗统一主题 + 深色中性化
- 坚持情况长条 NSelect+记忆；任务名双击编辑（去点击跳详情）；`.mini.edit` gap:0 居中
- **naive theme-overrides 接 token**（primary=强调色/语义色/圆角10px）+ 6 个原生 select 换 NSelect
- 深色 `--bg`→**#161616** 中性去蓝、文字提亮；柔光改 Zen 式整体渐变（wall-b/c 删除，从强调色 color-mix 派生）
- 默认值对齐（bgBlur26/sideAlpha60/cardShadow1.1，**UI_VERSION bump 5**）+ 各卡恢复默认按钮（打磨5 归位）
- 皮肤预设卡删除；取色器灰遮罩+铅笔图标；侧边栏图标单色化（`public/app-icon-line[-inv].svg` 双主题）

### 打磨5（3ab6695）图表/日志视图记忆/恢复默认归位
- HourlyChart tooltip 只列 >0 分钟且降序、全 0 系列不进图
- 日志：删「添加图片」、编辑器视图默认编辑+记忆（`notes.view`）、列表/画廊记忆（`notes.listMode`）、底部附件条仅编辑视图（后被打磨6改去重）、画廊封面修 watch 不触发+挂载竞态
- 主题与壁纸卡移最上；恢复默认拆回背景/侧边栏/卡片三卡

### 打磨6（6a29f71）附件条去重 + 提示 toast
- 预览"图消失"根因=被块删除引用的附件靠底部条兜底→底部条**去重**（编辑=全部；分栏/预览=仅正文未引用的附件，检测 `![](images/x)` 与 `<img src>` 两种写法）
- 个性化页底部 msg/err 改 **Teleport 底部居中 toast**（2.6s 自动消失，Teleport 留根 div 内）

### 打磨7（e1c9fb9）引号回退「」+ 全屏选图下沉 + 提醒统计
- `⌈⌋`→`「」`（标准直角引号；《》是书名号——两者别再混）
- **全屏背景轮换根治**：选图从"前端弹窗后自选"下沉到 `show_fullscreen` 弹出时就地 `reminder_bg_pick`、随 `payload.bg` 下发；每次提醒恰好消耗一张（旧链路建窗双耗+复用时序错位）；前端优先用 payload、兜底才自选
- 试看直接读编辑图随 payload 下发（kind="test" 不计数），`reminder_bg_test_pick` 删除
- **提醒统计**：`reminder_stats(kind,ref_id,shown,ack,snooze,other,last_ts)` UPSERT；`reminder::show` 统一计弹出、`reminder_action` 按动作归类（确认/稍后/其他，test 不计）；`reminder_stats_list` 命令；清单·提示子页行尾「弹 N · 点 M」标签

### v0.3.3 发布（5518135 + Release）
- 版本四处同步；删除未引用的 `public/vite.svg`、`public/tauri.svg`（模板残留，仓库已同步删除）
- 打包：`pnpm tauri build --bundles nsis` ✓；dist 标准 backdrop-filter 11 处（前缀检查 ✓）
- **Release v0.3.3**：setup 3,971,418 B sha256 `b5ae07a4…399ed0` ｜ portable 9,554,944 B sha256 `af959bbb…4a7516`
- 本地归档 `release/0.3.3/`（含 RELEASE_NOTES-0.3.3.md）

## 三、本轮新增/重点文件

- 后端：`storage.rs`（repeat_stopped/统计表/生效区间/friendly_name/idle_threshold/reminder_bg_read_file）、`reminder.rs`（Payload.bg/BgData/stat_shown/show_fullscreen 选图）、`tracker.rs`（阈值缓存/初始化）、`lib.rs`（新命令 task_stop_repeat、startup_show_get/set、idle_threshold_get/set、reminder_stats_list、rule_add/update 加日期、自启动重注册、启动显示策略）
- 前端：`TaskBoard.vue`/`ReminderRules.vue`/`NotesPanel.vue`（三卡列表+次级表单范式重写）、`HabitGrid.vue`（删时间轴）、`DayDetail.vue`（mode=stats）、`DetailPage.vue`（清单四子页+chips→长条下拉+统计标签）、`TodoPage.vue`（弹层头部跳转胶囊、双击编辑）、`FullscreenReminder.vue`（payload.bg 优先）、`SettingsPage.vue`（启动显示/空闲判定开关）、`theme.css`（#161616/柔光重做）、`lib/appearance.ts`（默认值+UI_VERSION 5）、`lib/uiState.ts`（checklist+sub）、`public/app-icon-line[-inv].svg`

## 四、验证状态

- `cargo test` **34 通过 + 2 忽略**；`cargo check` 警告 0；`pnpm build` ✓；NSIS 打包 ✓
- **未验证/待实机**：全屏图片轮换的实弹观察（连续弹几次确认逐张换）、提醒计数增长、深色柔光观感、便携版 0.3.3 覆盖升级

## 五、已知的坑（新会话必读，在 HANDOFF-3 十九条之上新增/更新）

1. **用户的 live 数据库在 `release/0.3.2/Data/tallymoment.db`**（便携版就地运行）——排查线上数据先看它，dev 库只是开发数据
2. **v-show 单根**：Teleport 必须放根 div 内（本轮 toast 遵循；PersonalizePage 根 div 内已有两个 Teleport 共存）
3. **button 不能嵌 button**：可点卡片要么整体 button 内部无按钮，要么外层 div[role=button]
4. **类名冲突**：scoped CSS 里输入框类（width:100%）别与按钮共用（.edit 事故）；`.mini` 基类 gap 会顶歪零宽 span 的图标（.mini.edit 需 gap:0）
5. **原生 `<select>` 下拉与主题不统一**：一律用 NSelect（naive overrides 已接 token）
6. **日志附件 vs 正文引用**：附件可不被正文引用（块删除后残留）——展示需两路并查（底部条去重逻辑见 NotesPanel `referencedImages`）
7. **发布侧命名**：资产改名曾致"记录自身"拆条——身份已用 ProductName，便携版改名安全；但仍遵守"资产 sha256 与归档一致"
8. **轮换唯一入口**：全屏背景图选取只在 `show_fullscreen`（Rust 端）发生，随 payload.bg 下发——前端不要再自己挑图（试看走 kind="test" 自带 bg）
9. **引号**：UI 文案统一「」；《》是书名号不要混用
10. HANDOFF-3 的 19 条坑全部仍有效（主线程建窗死锁/看门狗/CSS 前缀/锁规则等）

## 六、安全红线（公开仓库）

1. `logs/`、`WORKLOG.md`、`*.db`、`Data/`、`.dev-data-backup*/`、`target/`、`release/` 永不提交（.gitignore 已配；`git add -A` 危险，逐文件确认）
2. 发布前体检：无密钥、无真实数据、无本机绝对路径
3. git push / GitHub Release 需用户明确授权（本轮已授权并完成）

## 七、下一步（优先级）

1. 实机验收：全屏轮换实弹观察、提醒计数、便携版 0.3.3 覆盖升级（live 库在 release/0.3.2/，升级后注意数据目录）
2. 原子岛吸附三遗留（HANDOFF-3 第七节，用户明确要修，需 [island] 日志）
3. 洞察 L1/L2 落地（趋势对比+分类评分+计划 vs 实际）
4. 提醒统计的展示深化（趋势/成功率）与任务侧统计（目前 ref_id 是实例 id，跨天不聚合）
5. 心流参数按长期数据调优；详情页导出图片（想法登记未动）

## 八、发布现状（0.3.3，2026-10-26）

| 项 | 值 |
|---|---|
| Release | https://github.com/Frostleaf0929/TallyMoment/releases/tag/v0.3.3 |
| 安装包 | `TallyMoment_0.3.3_x64-setup.exe`（3,971,418 B，sha256 `b5ae07a49d8303e66de0ecce5402d2a079dff5d529d7cbcc4ac0d03447399ed0`） |
| 免安装 | `TallyMoment_0.3.3_x64-portable.exe`（9,554,944 B，sha256 `af959bbb52323b6959d097408e75faf0ae29852e7c6036099682c09ec84a7516`） |
| 本地归档 | `release/0.3.3/`（setup + portable + RELEASE_NOTES-0.3.3.md） |
| 打包命令 | `pnpm tauri build --bundles nsis`（本机必须限定 nsis） |

## 九、记录位置

- 索引：`E:\04_Archives\01_Personal\_ZCode\拾刻 - TallyMoment\WORKLOG.md`
- 本轮分册：`logs/2026-10-24-细节打磨2-界面统一列表与次级编辑/LOG.md`（打磨1~7 全部六~十一节）
- 本文件：开发仓库根 `HANDOFF-4.md`
