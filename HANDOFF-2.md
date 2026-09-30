# HANDOFF-2.md · 拾刻 TallyMoment 交接文档（第三版）

> 生成时间：2026-09-30 ｜ 用途：切换对话/换机器时给新会话与未来的自己的完整上下文
> 上一版交接见 `HANDOFF-1.md`（截至 0.2.0 打包发布）。本版覆盖其后的全部工作：
> **批次 7 反馈修复（五轮）+ 原子岛（09）+ 模块重构（06）+ 后台时长（12）+ 六条反馈修复 + 原子岛行为设置与吸附**。
> 从本版起，大功能框架已全部落地，**后续进入细节打磨阶段**。

---

## 一、项目与路径（以本版为准，HANDOFF-1 里的旧路径已失效）

**拾刻 TallyMoment** — Windows 桌面时间记录与效率回顾工具（本地优先，不联网）。
三个核心功能：使用时长记录 ｜ 任务督促（待办+固定事项+提醒）｜ 分析与建议（日志+洞察）。

- **开发仓库（代码）**：`E:\04_Archives\01_Personal\_ZCode\TallyMoment`（工作区已从 `Ex - Project` 迁移并并入 `_ZCode`）
- **记录仓库（不写代码）**：`E:\04_Archives\01_Personal\# ZCode\拾刻 - TallyMoment`（WORKLOG.md 索引 + logs/2026-09-29-批次7-反馈修复与增强/LOG.md 分册，**第十三~十九节是本轮**）
- **远端**：`https://github.com/Frostleaf0929/TallyMoment`（**公开仓库**，安全红线见第六节）
- **开发数据库**：`src-tauri/target/debug/Data/tallymoment.db`（**绝不能删整个 target/**）
- **当前版本**：0.2.0 已发布；本轮所有改动在 master，**未发版**

## 二、本轮做了什么（相对 HANDOFF-1 的增量）

### 1. 批次 7 反馈修复（第 3~5 轮，详见分册九~十二节）
- 洞察页：ResizeObserver 无害告警过滤、碎片化用青色 `--fragment`、时间范围加"总共"
- 习惯追踪格（7/14/30 天）+ 点任务跳详细·事项；近 14 天 7 列对齐
- 日志编辑器 Notion 式（选区工具栏、实时草稿、分栏/预览三态、图片缩略 640px、zoom 按钮）
- 详细页完成率 1000% 修复（后端已是百分比不再乘 100）；跳转遮罩互斥与关闭修复
- **应用友好名**：读 exe 的 FileDescription 回填 display_name（Tai 式，花笺已验证），改名编辑功能已删
- **应用忽略**：apps.ignored + 全部报表 JOIN 过滤 + 信息卡开关
- **行内信息卡**：详细页选中应用时卡片跟在行下方，与底部卡互斥显示（滚动可见性检测）
- **Tai 导入**：真实库验证（578 应用/11455 日行），Models 后缀表名兼容，三模式（补充/合并/替代+自动备份）

### 2. 原子岛（09，全程 ~7 轮迭代，最终形态见分册十三~十九节）
- **独立轻量窗口**：`island.html` + `src/island.ts`（vanilla TS，bundle 2.73KB），Vite 多页入口；主前端不再加载进岛（此前 2 秒+黑屏）
- **三态**：idle（无悬停隐藏态）→ normal（悬停完整胶囊）→ expanded（点击本体展开待办面板）；点击 vs 拖动由前端判别（位移>6px 才交给系统拖动，**不能用 data-tauri-drag-region，会吞 click**）
- **窗口插值动画**：Rust 侧 spawn 线程 8 步 × 16ms easeOutQuad 同步插值 set_size+set_position（整体放大、水平中心对齐、顶边不动）；ANIM_GEN 原子代数防动画堆叠
- **样式**：圆角胶囊（**用户明确要求保持胶囊外形**）、电标圆环（专注时每分钟转一圈+呼吸光晕）、徽章、横排单元、终末地 L 角标；强调色模式可切（终末地黄绿 #b8e34b / 跟随主程序强调色，island.ts 内置 ACCENT_MAP）
- **个性化卡**（个性化 → 原子岛）：启用 / 预设（终末地/跟随强调色）/ 材质（纯色/毛玻璃/云母，切换=关窗重建）/ 位置（顶部居中/靠左/靠右，拖动后自动 custom）/ 自动隐藏 / 隐藏形态（**缩小胶囊 / 靠边吸附**）/ 隐藏延迟（0~30s）/ 隐藏后宽度（120~360px）/ 吸附露出高度（4~24px，仅吸附态）/ 模块管理（focus/next/done/clock 排序增删）
- **入场动画**：窗口创建后从屏幕上方 64px 滑入（约 140ms，参考 zmd-charge 插电弹出）
- **已知修复**：启动死锁（setup 锁内调 show 重锁）、多屏/缩放居中（logical_screen 必须带主屏 origin 偏移）、加载慢（独立页）

### 3. 模块重构（06，提交 8aa42e1）
- 任务级提醒：`tasks.remind_style`（''=默认卡片 / card / fullscreen / none）+ `tasks.remind_interval_min`
- 到期提醒按 style 弹卡片或全屏；进行中间隔提醒（手动回补）：start_ts 有值 + interval>0 时每到间隔弹"进行中 · 已 N 分钟"卡，动作=继续（重置计时）/完成/停止计时，`reminded_key` 存 `i:<ts>`
- TaskBoard 编辑改为"内容+提醒方式+间隔确认+保存"块（行尾"编辑"按钮，**双击方案已废**——单击跳转抢事件）；行内小标（每N分确认/全屏提醒/不提醒）
- "待办提醒"模块文案窄化为"提示"（喝水等轻提示），任务提醒归任务本身

### 4. 后台时长（12，提交 87fa74e）
- `apps.track_background` 开关；hourly_stats/daily_stats 各加 `bg_seconds`（独立于前台 seconds，UPSERT 累加）
- `tracker::spawn_bg_tracker`：每 5 秒 Toolhelp32Snapshot 枚举进程名（Cargo.toml 加 `Win32_System_Diagnostics_ToolHelp` feature，未新增依赖），命中 track_background=1 且未忽略的应用累计 +5s
- 展示：排行应用行"后台 X"低饱和小标（4 列 grid 第 4 列，bg>0 才显示）；应用信息卡（行内+底部）"跟踪后台时长"开关
- **数据已实证正常**：dev 库 bongocat.exe 单日 bg_seconds=34285s≈9.5h，逐小时满额

### 5. 六条反馈修复（提交 541ba03）
- 后台小标竖排修复（grid 归位+nowrap）；统计卡单行自适应字号（szClass 按内容长度降两档）；信息卡长名省略+title
- **"记录拾刻自身"开关**（设置→系统，默认关）：TrackerShared.track_self + settings `track.self`
- 已完成列表折叠（默认最近 12 条+展开全部）

### 6. 会话级事故（前段发生，已处置完毕）
- `.dev-data-backup-20260930/`（含真实 DB/图片/模型 DLL）曾被 `git add -A` 误提交 → `git rm -r --cached` + .gitignore + `git commit --amend`（推送前发现），最终提交干净。**教训见第六节红线**

## 三、本轮改动过的文件（开发仓库）

**Rust**：`src-tauri/src/island.rs`（新，全岛后端）、`lib.rs`（island/行为/后台/自记录命令 + TrackerShared.track_self + spawn_bg_tracker）、`tracker.rs`（提醒调度扩展+后台线程+自排除可配置）、`storage.rs`（6 条独立 ALTER 迁移 + interval_tasks/bg_add_seconds/app_set_track_background/task_update 扩展）、`reminder.rs`（不透明+DWM 圆角+全屏 ready-show）、`app_icon.rs`（FileDescription）、`Cargo.toml`（ToolHelp）
**前端**：`island.html`（新）、`src/island.ts`（新）、`vite.config.ts`（多页）、`App.vue`、`PersonalizePage.vue`（原子岛卡）、`SettingsPage.vue`（记录自身）、`DetailPage.vue`（行内信息卡+后台小标+字号自适应）、`TaskBoard.vue`、`TodoPage.vue`、`InsightsPage.vue`、`HistoryPage.vue`、`DayDetail.vue`、`MarkdownPreview.vue`、`types.ts`、`lib/mdBlocks.ts`、`lib/uiState.ts`
**配置**：`tauri.conf.json`（productName=TallyMoment 保持）

提交链（本轮）：`468102e → 639e3a9 → ce99c10 → 88304f1 → 8aa42e1 → 87fa74e → 541ba03 → 85d227f → 30ab44d`（+记录仓库同步提交）

## 四、验证状态

- **已验证**：cargo test 23/23+2 ignored（每轮提交前）；vue-tsc 零错误；pnpm build 成功；dev 启动日志 `[island] 窗口已创建 + 前端就绪`；后台时长数据实查正常；Tai 真实数据导入成功；花笺友好名生效
- **待用户实机验收**（下轮先问）：原子岛三态动效手感 / 吸附形态与露出高度 / 隐藏延迟与宽度 / 材质在用户机的表现 / 任务间隔提醒弹卡 / 后台时长排行小标 / 记录自身开关 / 双屏下岛居中

## 五、已知的坑（新会话必读）

1. **Mutex 不可重入**：setup 里持 `db.0.lock()` 时绝不能调 island::show 之类会再拿锁的函数（曾把软件搞到打不开）。模式：锁内读出需要的数据，释放后再操作。
2. **data-tauri-drag-region 吞 click**：岛的一切"点击展开"都用前端 mousedown/mousemove/mouseup 判别（位移>6px → island_start_drag）。
3. **透明窗口两个坑**：a) 透明合成在部分 Win11 24H2 机器不可靠（黑/灰框）→ 提醒窗用不透明卡片铺满方案；b) **Acrylic/Mica 显著拖慢 resize**（官方文档）→ 岛的动画只有 8 步，材质默认纯色且个性化里有警告。
4. **多屏/缩放**：`logical_screen()` 返回主屏 (originX, originY, w, h)，任何 set_position 都要加 origin 偏移，不能假定 (0,0)。
5. **AppUsage 有 5 处构造**：加字段要全改（storage.rs×4 + lib.rs×1）。
6. **迁移必须逐条独立 ALTER**（批处理遇"列已存在"会整体中断）。
7. **dev 环境**：`pnpm tauri dev` 的 Rust watch 不可靠，改 Rust 后手动重启 dev；重启前杀干净 tallymoment.exe + 占 1420 的 node（这是本任务自己启动的开发实例，杀前 netstat/tasklist 确认）；**绝不删整个 target/**（dev DB 在 target/debug/Data）。
8. **写文件**：长文件用 Write/Edit 工具，bash heredoc 会静默截断；Python 脚本写中文注意转义（`\\n` 折叠教训），写完 grep 验证。
9. **Escape**：Rust/TS 字符串里的换行经 Python heredoc 会折叠，换行用 `String.fromCharCode(10)` 或 Edit 工具。
10. **目录迁移后**：pnpm junction 失效 → `CI=true pnpm install`；target/debug/build 缓存含旧绝对路径 → 只清 build/ 子目录。

## 六、安全红线（公开仓库，违者难挽回）

1. `E:\04_Archives\01_Personal\_ZCode\TallyMoment` 是 **PUBLIC** 仓库：**logs/、WORKLOG.md、*.db、Data/、.dev-data-backup*/ 永不提交/推送**（.gitignore 已配，但 `git add -A`/`git add .` 仍是危险动作，**提交前必须 `git status` 逐文件确认**）。
2. 记录仓库（含 logs 分册）是另一仓库，**不得**推送或混入开发仓库。
3. 用户真实数据（DB、日志图片、BongoCat 模型）在任何仓库都不能出现；`.dev-data-backup-20260930/` 已入 .gitignore，永不再提交。
4. 必停清单仍然有效（删除/覆盖、工作区外写入、外发、装包、凭据）。

## 七、下一步（细节打磨方向，按用户表述"后续是更加细节化的打磨"）

**原子岛（优先）**
1. 吸附态露出边加"把手"视觉（亮色小横线，提升可发现性）——已向用户预告
2. "改个性化设置后原子岛消失"bug：已加 rebuild 日志，等用户复现时看 dev 控制台输出定位
3. 动效手感微调（入场约 140ms / 悬停约 128ms，等用户反馈）；idle 态内容截断待用户新截图确认
4. 用户若仍喜欢终末地元素需确认 L 角标/徽章去留；弹出动画可再参考 zmd-charge 三态节奏

**功能收尾**
5. 安装/卸载：等用户用系统"设置→应用"实测 NSIS 卸载（上次报错来自第三方工具 HiBit），报错则贴原文排查
6. 提醒规则表单草稿（deferred 多轮）；全屏+卡片提醒排队策略
7. 后台时长长期准确性观察；Insights 页是否也展示后台时长（当前只做详细页）
8. MSI/WiX 打包（此前超时 deferred，NSIS 已够用，低优先）

## 八、待用户决定 / 批准的事项

- 原子岛各动效速度、延迟默认值（现：入场 140ms/悬停 128ms/延迟 1s）是否合手
- 材质三选（毛玻璃/云母有性能代价）是否保留在个性化里
- 吸附露出边的把手视觉要不要做
- 发版节奏：本轮改动较多，是否择机打 0.2.1（需要用户确认后走打包流程）

## 九、记录位置

- 索引：`E:\04_Archives\01_Personal\# ZCode\拾刻 - TallyMoment\WORKLOG.md`（批次 7 行）
- 分册：`logs/2026-09-29-批次7-反馈修复与增强/LOG.md` 第十三~十九节（原子岛/06/12/反馈修复全过程与踩坑）
- 本文件：开发仓库根 `HANDOFF-2.md`
