# HANDOFF-3.md · 拾刻 TallyMoment 交接文档（第四版）

> 生成时间：2026-10-05 ｜ 用途：切换对话/换机器时给新会话与未来的自己的完整上下文
> 上一版交接见 `HANDOFF-2.md`（截至 0.2.0 发布 / 批次 7 + 原子岛 + 模块重构 + 后台时长）。
> 本版覆盖其后全部工作：**三路调研、原子岛方案 A（架构重做 + 六轮打磨）、心流判定重写 + 任务绑定、提醒背景图组、待办页布局、软删除、两个致命 bug 的定位修复、代码清理、0.3.0 发布**。

---

## 一、项目与路径（以本版为准）

**拾刻 TallyMoment** — Windows 桌面时间记录与效率回顾工具（本地优先，不联网）。
三个核心功能：使用时长记录 ｜ 任务督促（待办+固定事项+提醒）｜ 分析与建议（日志+洞察）；另加**原子岛**（常驻悬浮胶囊）。

- **开发仓库（代码，PUBLIC）**：`E:\04_Archives\01_Personal\_ZCode\TallyMoment`
- **记录仓库（不写代码）**：`E:\04_Archives\01_Personal\# ZCode\拾刻 - TallyMoment`（WORKLOG.md 索引 + logs/ 分册）
- **远端**：`https://github.com/Frostleaf0929/TallyMoment`（**公开**，默认分支 `master`）
- **开发数据库**：`src-tauri/target/debug/Data/tallymoment.db`（**绝不能删整个 target/**）
- **当前版本**：**0.3.0**（package.json / Cargo.toml / tauri.conf.json / 设置页 四处已同步）
- **本地发布归档**：`release/`（仓库根，已 gitignore；存放各版本安装包副本）

## 二、本轮做了什么（按主题）

### 1. 三路调研（2026-10-01，纯调研）
- 原子岛流畅度根因：**"每帧改窗口尺寸"是错的架构**——同类流畅项目全部原生渲染（WinIsland=D3D12+DirectComposition；同栈的 NetSpeed-Dynamic 官方承认架构受限、继任者改 C#+SkiaSharp）
- 心流判定：调研 Rize/RescueTime/Flowtime/Timemator/Clockk + 中断研究 → 规则框架
- 洞察升级：L1 趋势 → L2 分类评分 → L3 异常规律 → L4 学习版；"计划 vs 实际"是独有维度
- 记录：`logs/2026-10-01-调研-原子岛流畅度与洞察心流/LOG.md`

### 2. 原子岛方案 A（架构重做，六轮打磨）
- **固定窗口 392×324 一次创建、永不 resize**；idle/normal/expanded/snap 四态形变全部窗口内 CSS 动画
- **点击穿透**：光标轮询（120ms）动态 `set_ignore_cursor_events`
- **灰框根治**：`SetWindowRgn` 把窗口裁成胶囊形状（形状之外系统层面不存在）
- **标题栏白条根治**：tao 的无边框窗口样式表保留 `WS_CAPTION`，任何 NC 重绘（SetWindowRgn/show）都会把它画出来 → `island::strip_caption` 在每次贴区域与每次 show 之后调用
- **透明度可调**（40~100%，替代原"材质"档）；**置顶/鼠标穿透开关**；**吸附四边判定**（按位置模式优先选边 + work_area 距离 ≤48px）；**唤回方式**（靠近自动 / 点击）；**个性化卡折叠**
- 已知遗留（未解决）：吸附的顶/底不触发、侧边唤醒只露一半、底部停靠展开方向——见第七节

### 3. 心流判定重写 + 任务绑定
- **旧规则"整块切换 ≤3 次"被真实数据证伪**（254 分钟工作段、占比 88%、切换 389 次被误判碎片）
- **新规则＝上下文一致度**：跨度 ≥ 自适应门槛（下限 15 分钟）&& 活跃占比 ≥ 门槛（默认 80%）&& 前 K 个应用（默认 4）覆盖活跃时间 ≥ 门槛（默认 60%）；切换次数仅展示
- 参数在〈设置 · 心流判定〉可调（context_apps / context_min / active_min）
- **任务绑定**：任务 `related_apps`（JSON）；TaskBoard 编辑块可增删（含从任务时间窗的高频应用推荐）；洞察页"心流时段 · 归属任务"，未归属可一键确认并入集合（越用越准）

### 4. 提醒与背景图
- 提醒卡区域裁剪**已撤销**（tao 白边框防不胜防）→ 回 DWM 圆角
- **背景图组**：每组 ≤10 张、组数不限、组名可改、组间一键切换；播放**默认顺序循环**，开关切**随机洗牌**（轮内不重复）；缩略图 ✕ 移除；**双击缩略图进全图编辑**（左键拖动 / 滚轮缩放 50~300% / 蒙版实时预览）
- **渲染口径唯一**：`src/lib/bgStyle.ts` 被编辑器与真实全屏提醒共用（本轮"取景不一致"的结构性根治）

### 5. 其他
- 待办页两卡等高（**量月历"内容层"高度设到行容器**，绕开 flex 拉伸死循环）；习惯追踪时间轴 30 天日期抽稀；主窗口 16px 外框可拖动（含双击最大化）；任务**软删除**（删除保留完成记录与统计）；设置页作者卡；代码清理（死代码 + 编译警告 22→0）

### 6. 两个致命 bug 的定位与修复（本轮最大教训）
- **① 主线程自死锁**：sync 命令跑在主线程，而**窗口创建必须由事件循环线程执行**——在命令里同步 `build()` = 等自己 → 全应用假死（表现为所有按钮无反应、洞察卡"分析中"）。由**看门狗**（`[diag]` 行）实测坐实。修复：`reminder_bg_test_show` 与 `island_set_enabled` 的窗口创建移入 `std::thread::spawn`
- **② 实机取景错误**：实机 `.bgimg`（img）**缺显式宽高** → 浏览器按**原始像素尺寸**渲染（原图大只见左上角、"缩放随机但固定"、原图小露黑区）。修复：`.bgbox`（overflow:hidden）+ `.bgimg { width/height:100% }`

### 7. 发布后再发现的构建期缺陷：release 版毛玻璃整体失效（0.3.1 修复）
- **现象**：release 版所有二级界面卡片"发透、背后不模糊"，且调材质/模糊/不透明度**滑杆完全无反应**；dev 版正常。
- **根因（构建期，非代码逻辑）**：源码里 CSS 前缀**手写双份**（标准 `backdrop-filter` + `-webkit-backdrop-filter`）→ **Vite 8（Rolldown）压缩时做前缀去重，丢弃标准属性、只留 `-webkit-`** → 而 **WebView2（Chromium）不认 `-webkit-backdrop-filter`（Safari 专属）** → 模糊整体失效。dev 用开发服务器不压缩，故正常。
- **定位证据**：`dist/assets/*.css` 里标准属性仅 2 处（且为硬编码）/ 前缀 12 处；裸 esbuild 对照试验证明它不删双写 → 锁定 Vite 8 压缩环节。
- **修复**：删除手写的 9 处前缀，**只写标准属性**（构建器会自动补前缀）→ dist 标准属性 2 → 11 处 ✓
- **教训入坑清单第 15 条**；连带影响：v0.3.0 发布版带此缺陷 → **0.3.1 重发**

### 8. 应用归族（A+C）：同软件的不同文件名/副本不再分裂显示
- **背景**：应用身份用 exe 文件名 → 绿色版改名（`xxx-portable.exe`）、发布改名等会造成"同一软件多条记录"（用户提问："要是其他软件也改个名呢？"）
- **A 展示层归并**：`apps` 新增 `family` 列；报表/排行/作息分布等 7 处聚合查询改为按 `CASE WHEN a.family = '' THEN a.name ELSE a.family END` 分组（**未回填时行为与从前完全一致，零回归**）；启动时用 exe 产品名（`ProductName`）自动回填 family（只填未归族的，增量幂等）
- **C 手动归并**：详细页应用信息卡新增「并入其他应用…」（下拉选目标）与「取消归并」——**只改 family 标记、不动任何统计数据**，随时可还原
- 提交 `986fff9`；cargo test 28+2 ✓

## 三、关键文件与新增

**新增**：`src-tauri/src/island.rs` 大改（方案 A）、`src/lib/bgStyle.ts`（渲染口径唯一）、`island.html` / `src/island.ts`（四态 CSS）、`release/`（本地发布归档，gitignore）
**重点改动**：`lib.rs`（看门狗 `spawn_watchdog`、`diag_log`、`spawn_cursor_poll` 相关命令、`today_report` 会话快照化）、`reminder.rs`（`show_card` / `show_fullscreen` 线程约定）、`storage.rs`（背景图组/软删除/心流参数）、`insights.rs`（`FlowParams` + 上下文一致度）、`FullscreenReminder.vue`、`PersonalizePage.vue`、`TaskBoard.vue`、`InsightsPage.vue`、`TodoPage.vue`、`SettingsPage.vue`

## 四、验证状态

- `cargo test` **27 通过 + 2 忽略**；`pnpm build` 零错误；**Rust 编译警告 0**
- 实机验收通过：原子岛流畅度/无灰框/无标题栏白条/透明度/穿透/置顶；心流划分正确（洞察页可见）；背景图组与全图编辑；待办页；试看不再卡死
- **未验证/遗留**：吸附三个问题（第七节）；心流参数待长期使用调优

## 五、已知的坑（新会话必读，按重要性）

1. **绝不在命令（主线程）里同步创建窗口**——用 `std::thread::spawn`（tao/tauri 窗口创建须事件循环线程执行；在主线程同步 build = 自等待死锁，全应用假死）
2. **看门狗与 `[diag]` 埋点**：卡死时终端自动打印结论行（主线程阻塞 / Db 锁被占）；**前端 console 在 dev 终端不可见 → 用 `diag_log` 命令回传**
3. **CSS 替换元素尺寸**：`<img>` 必须有显式 width/height，否则按**固有像素尺寸**渲染（取景/黑区类怪象优先查这里）
4. **渲染口径唯一**：背景图的展示样式只走 `src/lib/bgStyle.ts`，编辑器与实机共用（禁止两处各写一份）
5. **`v-show` 页面组件必须单根节点**：Teleport 必须放在根 div **内部**，否则隐藏失效（本轮"五页面粘连个性化页"事故）
6. **改完文件立即 grep 验证落盘**：写入丢失真实发生过（`.bgimg` 的 CSS 修复曾丢失一轮）
7. **区域裁剪（SetWindowRgn）仅用于原子岛**；提醒窗用 DWM 圆角（tao 的 WS_CAPTION 会在 NC 重绘时带出白边框）
8. **判断命令成败别用管道**：`out=$(cmd 2>&1); rc=$?`（管道退出码是 tail 的）
9. **目录迁移后**：不仅清 `target/debug/build`，**release 打包前也要清 `target/release/build`**（含旧绝对路径 → 打包失败 "failed to read plugin permissions"）
10. **锁**：tracker 每秒"session→Db"双持；**持锁期间绝不做窗口/托盘/emit 等会派发到主线程的操作**；`today_report` 已改快照式
11. **Mutex 不可重入**：持 `db.0.lock()` 时绝不调用会再取锁的函数（island::show 等）
12. **dev 环境**：改 Rust 后手动重启 `pnpm tauri dev`；大批量改动后热重载会进入坏状态（现象类似假死，**重启即好**——但本轮①是真死锁，区别看 `[diag]`）
13. **写文件**：bash heredoc 单次别超 ~10KB（会静默截断）；优先用 Write/Edit 工具
14. **多屏/缩放**：本机 2560×1440 @175%（逻辑 1463×823）；编辑层虚拟屏幕取 **primaryMonitor** 宽高比（与全屏提醒同源）
15. **CSS 前缀绝不手写双份**：Vite 8（Rolldown）压缩会做前缀去重，**"标准 + `-webkit-`"双写会丢标准属性、只留 Safari 前缀** → WebView2 不认 → 效果（毛玻璃等）在 release 里整体失效，而 **dev 正常**（不压缩）。**只写标准属性**，让构建器自动补前缀。排查法：`dist/assets/*.css` 里标准属性数量 vs 源码数量（`main-*.css` 里 `(?<!-)backdrop-filter` 计数）
16. **dev 与 release 的 localStorage 按 origin 隔离**：dev=`http://localhost:1420`、release=`http://tauri.localhost`，**外观设置互不相通**（同一 WebView 数据目录但键空间不同）——"dev 调好的外观在 release 不生效"先查这里；但**若滑杆调了完全无视觉变化，则是渲染链路问题（见第 15 条）而非设置问题**
17. **发布检查清单（打包前逐项过）**：① 版本号四处同步；② 清 `target/release/build/`（若报 plugin permissions 路径错）；③ `pnpm build` 后抽查 `dist` 关键属性数量（前缀类问题只在此暴露）；④ 用 `out=$(cmd 2>&1); rc=$?` 判成败；⑤ 资产 sha256 与本地归档一致
18. **身份绝不用 exe 文件名**：应用身份/自身识别若依赖文件名，会随版本号（发布时改名区分下载）或用户重命名而**分裂**——「记录自身」曾把一个软件记成 3 条（tallymoment.exe / tallymoment_0.2.0_x64-setup.exe / tallymoment_0.3.0_x64-portable.exe）。稳定标识用 exe 版本信息的 `ProductName`/`FileDescription`（`app_icon::product_name` / `is_self_exe`）；自身判定再加「exe 完整路径相同」最可靠。历史已分裂的条目由 `storage::merge_self_entries` 一次性幂等合并（**执行前自动备份** `<db>.bak-selfmerge-<ts>`）

## 六、安全红线（公开仓库）

1. `logs/`、`WORKLOG.md`、`*.db`、`Data/`、`.dev-data-backup*/`、`target/`、`release/` **永不提交**（.gitignore 已配；`git add -A` 仍是危险动作，**提交前必须 `git status` 逐文件确认**）
2. 发布前体检：无密钥、无真实数据、无本机绝对路径
3. 必停清单仍有效（删除/覆盖、工作区外写入、外发、装包、凭据）；**git push / GitHub Release 需用户明确授权**（本轮已授权）

## 七、下一步（优先级）

1. **原子岛吸附三个遗留**（用户明确要修，需 dev 控制台 `[island]` 日志）：顶/底不触发吸附；侧边唤醒只露一半（不回全）；底部停靠时展开面板应向上生长
2. 心流参数按长期使用数据调优（阈值已可调，缺真实体感反馈）
3. 洞察 L1/L2 落地（趋势对比 + 分类评分 + 计划 vs 实际）
4. 详情页导出图片（仅登记想法，未动）
5. 发布相关：MSI（WiX 下载超时，deferred）、安装包签名、ICO 精修

## 八、待用户决定

- 版本号 0.3.0 的定位是否合适（本轮为功能级升级）
- 吸附遗留问题的优先次序
- 洞察升级从哪一级开始做

## 九、发布现状（0.3.0，2026-10-05）

| 项 | 值 |
|---|---|
| Release | https://github.com/Frostleaf0929/TallyMoment/releases/tag/v0.3.0 |
| 安装包 | `TallyMoment_0.3.0_x64-setup.exe`（3,948,550 B，sha256 `e8cfcf55…1eadbb`） |
| 免安装 | `TallyMoment_0.3.0_x64-portable.exe`（9,480,192 B，sha256 `f0b965e0…38b047a`） |
| 本地归档 | `release/0.3.0/`（仓库根，**已 gitignore，绝不上传**）——与 Release 内容一致（两资产 + RELEASE_NOTES） |
| 打包命令 | `pnpm tauri build --bundles nsis`（**本机必须限定 nsis**；打包前若报 plugin permissions 路径错误，先清 `target/release/build/`） |

## 十、记录位置

- 索引：`E:\04_Archives\01_Personal\# ZCode\拾刻 - TallyMoment\WORKLOG.md`
- 本轮分册：`logs/2026-10-01-调研-原子岛流畅度与洞察心流/`、`logs/2026-10-01-原子岛方案A-固定窗口与DOM动画/`、`logs/2026-10-01-心流判定改造-第一步无任务版/`、`logs/2026-10-03-反馈修复-白边与待办记录/`
- 本文件：开发仓库根 `HANDOFF-3.md`
