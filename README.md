# 拾刻 · TallyMoment

> 一个把「时间去哪了」变成可读、可回顾、可改进的 Windows 桌面工具。
> 本地优先：**不联网、不上传、不注册**，所有数据都存在你自己的电脑里。

---

## 三个核心功能

### 1. 使用时长 —— 看清每一分钟花在哪个软件

- **前台追踪**：每秒记录当前前台窗口，按应用把时间切分入库；切应用、离开键盘 60 秒、退出程序都会立即落库，长会话每分钟打一次检查点（意外崩溃最多丢 1 分钟）
- **离开检测**：60 秒无键鼠输入即停止计时，挂机不会算成使用时长
- **防抖**：新应用需连续 2 秒在前台才确认切换，Alt-Tab 掠过的瞬态不计入
- **四种视图**：
  - 〈今日〉当前专注 + 统计卡 + 24 小时分布 + 应用排行
  - 〈历史〉近 14 天 / 按月 / 按年 / 总计
  - 〈详细 · 应用〉按应用下钻（今天 / 本周 / 本月 / 本年 / 全部）
  - 〈详细 · 事项〉某一天的使用与任务对照
- **热力月历**：一个月一眼看出哪天用得久，点某天即可进入当天详情
- **键鼠计数**：全局钩子只统计"有按键/有点击"的次数（**不读取键值内容、不记录坐标**）

### 2. 任务督促 —— 待办、固定事项与到点提醒

- **任务看板**：重要度（高/中/低）、到期时间、完成勾选；双击就地改名
- **固定事项**：日 / 周 / 月 / 年化，自动为"今天该出现"的模板生成当日实例（幂等，不会重复生成）
- **每日任务追踪**：可记录"开始做"的时间，完成时自动算出**完成用时**（开始→完成优先，退回创建→完成）
- **到点提醒**：规则支持**时间间隔**与**每日定点**两种触发方式，提醒方式可选
  - **右下角卡片**：可定制的停留时长 / 卡片常驻
  - **全屏提醒**：铺满屏幕的休息提示页（分层柔光 + 呼吸动效 + 大号提示语），支持**自定义背景图**
- **导入导出**：任务用 **Markdown**（`- [ ]` 语法，Obsidian / Notion 直接打开，兼容 `*`/`+` 符号与 Tasks 插件的 `📅`/⏫ 写法）；提醒规则用 **JSON**（结构化字段完整回环）

### 3. 分析与建议 —— 把数据变成结论

- **每日日志**：Markdown 正文 + 图片附件，**块式编辑**（标题/正文/待办/列表/引用/分割线/图片块，块可上下移动、图片可选宽度），导出为标准 Markdown（带 front-matter）
- **洞察页**：
  - 专注块归并与**心流 / 专注 / 碎片**状态推断（本地规则，不上传）
  - 今日各小时状态堆叠、近 14 天作息分布、使用频率趋势
  - **分析 + 建议**双段式结论：先说数据说了什么，再说可以怎么做
- **完成率与用时分布**：今日完成、本周完成率、按时完成率、平均完成用时，以及完成用时分布

---

## 技术栈

| 层 | 技术 |
|---|---|
| 框架 | Tauri 2（Rust 核心 + WebView2 渲染） |
| 前端 | Vue 3 + TypeScript + Vite + Pinia + Naive UI + ECharts |
| Rust 依赖 | rusqlite（SQLite，WAL 模式）、active-win-pos-rs、chrono、windows、tauri-plugin-dialog / autostart / opener、zip、rust_xlsxwriter |
| 数据 | 程序旁 `Data/tallymoment.db`（不可写时回退 `%APPDATA%\TallyMoment\Data`） |

## 构建与运行

```powershell
pnpm install       # 首次
pnpm tauri dev     # 开发运行
pnpm tauri build   # 打包（NSIS 安装包 / MSI / 免安装 exe）
```

前置：Node.js ≥18 + pnpm、Rust stable-msvc、VS Build Tools、WebView2 运行时（Win10 1809+ 系统自带）

## 数据与隐私

- 所有记录只写本机 `Data/` 目录；**没有任何联网代码**，也不含遥测
- 数据可整体导出：JSON（换机恢复）、Tai 对齐的 `data.db` + CSV + xlsx（可导入原版 Tai）、任务 Markdown、日志 Markdown + 图片
- 崩溃安全：SQLite WAL + `synchronous=FULL`，已落库的数据不会因强杀损坏

## 鸣谢与来源

**借鉴项目**
- [Tai](https://github.com/Planshit/Tai) —— 时间记录的交互与数据组织方式、Tai 对齐导出（多数界面思路参考它）
- [Catrace](https://github.com/lanxiuyun/Catrace) —— 热力图与提醒形态的参考

**灵感**
- [flow-insight](https://github.com/mewamew/flow-insight) —— 心流 / 专注状态的推断思路

**素材与设计来源**
- 软件内图标：[svgrepo · Solar Line Duotone](https://www.svgrepo.com/collection/solar-line-duotone-icons/)（480 Design, CC BY 4.0）
- 动效启发：[uiverse.io](https://uiverse.io/)

**开发方式**
本项目是 **vibe coding 产物**，由 **GLM-5.3-flash** 与 **DeepSeek-V4.1（dsf4.1）** 在 **ZCode** 中协作开发完成。

## 许可

MIT（见 `LICENSE`）。第三方素材遵循各自授权：Solar 图标为 CC BY 4.0，需保留署名。
