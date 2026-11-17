# Windows 11 任务栏行为分析

这份文档分析 Windows 11 任务栏的完整行为，对照 Nectar Dock 的现状，列出功能完备还差什么、以及每一项该怎么改。

它是一份**分析与计划**，不是实现记录。代码引用指向当前分支（`security/privacy-hardening`）。

---

## 1. 任务栏的解剖

Win11 任务栏从左到右分为四段。理解这个分区是理解后面所有差异的前提。

```
┌────────┬──────────────────────────┬────────────────────────┬──────────┐
│ 开始   │      应用按钮区           │    系统托盘区          │ 显示桌面 │
│ Widgets│  (大图标，任务分组)       │  (小图标 + 时钟)       │  细条    │
└────────┴──────────────────────────┴────────────────────────┴──────────┘
```

| 区域 | 内容 | Nectar 现状 |
|---|---|---|
| **左侧** | 开始、搜索、任务视图、Widgets、聊天 | 部分（开始/搜索/任务视图，无 Widgets/聊天） |
| **中间** | 运行中与固定的应用按钮，带分组、未读、进度、跳转列表 | 基本齐备（分组/未读/预览），缺进度与跳转列表 |
| **右侧托盘** | 通知图标、音量/网络/电池、**时钟日期**、通知中心 | 新增了托盘文件夹，**缺时钟** |
| **最右** | 显示桌面细条 | 有（可开关） |

### 1.1 应用按钮的六种视觉状态

原生一个按钮可以同时表达多层信息。Nectar 目前只实现了前三层。

| 状态 | 原生表现 | Nectar |
|---|---|---|
| 未运行（仅固定） | 只有图标 | ✅ |
| 运行中 | 图标 + 底部横条/圆点 | ✅ 可配置条形/固定/圆点 |
| 有多个窗口 | 横条变长 / 多个圆点 | ✅ |
| **需要关注**（闪烁） | 按钮背景闪烁橙色 | ⚠️ 只有笔记本头上的圆点，且仅对触发 `HSHELL_FLASH` 的应用 |
| **有进度** | 图标下方一条绿色进度条 | ❌ 完全没有 |
| **覆盖图标**（overlay） | 图标右下角小徽标 | ❌ 完全没有 |

> 后两项来自 `ITaskbarList3`：`SetProgressValue` 和 `SetOverlayIcon`。这两个 API 由**应用主动调用**，所以 Nectar 不能"查询"谁有进度——它自己的窗口不在原生任务栏上，也就收不到这些值。
>
> **结论：进度条和覆盖图标在 Nectar 架构下无法实现。** 这是替代任务栏的根本限制，需要在文档里明确写出来，而不是假装能做。

### 1.2 时钟区域

原生右侧是一个时钟块，点击打开通知中心/日历浮出。

- 上排：时间（`14:32`）
- 下排：日期（`2026/11/17`）
- 悬停：完整日期
- 点击：日历 + 通知面板
- 右键：调整日期/时间设置
- 有未读通知时：显示通知计数徽标

**Nectar 的 Dock 里完全没有时钟。** 时钟只出现在刘海（`NotchClock`）。如果用户隐藏刘海只用 Dock，就看不到时间——这是功能倒退。

---

## 2. 分组行为

### 2.1 原生行为

| 设置 | 行为 |
|---|---|
| **合并按钮**（默认） | 同一应用的所有窗口合成一个按钮；悬停显示预览画廊；点击在窗口间循环 |
| **从不合并** | 每个窗口一个独立按钮，带标题 |
| **已满时合并** | 空间不足时才合并 |

Win11 只提供"合并 / 不合并"两档（`TaskbarGlomLevel`），没有"已满时合并"了。

### 2.2 Nectar 现状

`nectar-dock-mixed-reorder` 控制的是"固定与运行是否混排"，不是合并策略。Nectar **始终合并**，且点击行为是：

```
1 个窗口 → 聚焦
多个窗口 → focus_app_windows（聚焦最近，再点循环）
```

**缺口**：没有"每个窗口一个按钮"的模式。对于重度多窗口用户（比如开了 8 个资源管理器窗口），原生可以让他们散开，Nectar 只能在一个图标上循环。

### 2.3 需要的改动

1. 新设置 `nectar-dock-grouping`：`"combined"` / `"never"`，默认 `combined`。
2. `never` 模式下，`get_active_windows` 返回的每个 `all_hwnds` 条目渲染成独立按钮，图标右下角叠一个小角标表示"同一应用的第 N 个窗口"。
3. 拖拽排序在 `never` 模式下按 hwnd 而非 appIdentity 分组。
4. 预览画廊在 `combined` 下保持现状；`never` 下悬停单个按钮就预览那个窗口。

**工作量**：中等。主要风险在 `dockItems` 的 `useMemo`——目前按 app 聚合，改成窗口粒度会牵动排序、拖拽、隐藏应用、上下文菜单多处。

---

## 3. 悬停预览

### 3.1 原生行为

- 悬停约 400ms（可调）显示预览卡片
- 卡片上是窗口缩略图 + 标题
- **卡片底部有 Thumbnail Toolbar**（`ITaskbarList3::ThumbBarAddButtons`）：播放器有播放/暂停，浏览器有时有标签页
- 相邻窗口的预览可以并排显示（Aero Peek）
- 点击预览 = 切换到该窗口；预览右上角有 ×，等于关窗
- 鼠标悬停在预览上时，**被预览的窗口会高亮**（Aero Peek 效果）

### 3.2 Nectar 现状

| 能力 | 状态 |
|---|---|
| 悬停延迟可调 | ✅ `nectar-dock-preview-delay` |
| 缩略图预览 | ✅ `capture_window_thumbnail` + DWM 实时缩略图 |
| 预览里关窗 | ✅ `close_window` |
| 点击预览切换 | ✅ |
| **Thumbnail Toolbar** | ❌ 无法实现（同 1.1，应用主动注册的按钮收不到） |
| **Aero Peek 高亮** | ❌ 未实现 |

### 3.3 需要的改动

**Aero Peek 是唯一可做的**：悬停预览时，把被预览窗口之外的所有窗口调暗。

实现路径：Nectar 已有一个 `overlay` 窗口（全屏、置顶、透明）。可以在预览期间显示一个半透明黑色蒙版，并用 `DwmRegisterThumbnail` 把被预览窗口"挖"到蒙版之上。

- 蒙版：`overlay` 窗口铺满该显示器，背景 `rgba(0,0,0,0.35)`
- 高亮：对目标 hwnd 调 `DwmRegisterThumbnail` 到 overlay 的一个矩形，或直接让 overlay 不覆盖那块区域（用 region 挖洞）
- 现在的 `live_preview.rs` 已经在用 DWM 缩略图，可复用

**工作量**：中等。主要难点是 overlay 的点击穿透与 z 序，以及预览卡片本身不能被蒙版盖住。

---

## 4. 右键菜单（Jump List）

### 4.1 原生行为

原生右键一个按钮，给出：

1. **任务**：应用声明的快捷操作（Chrome 的"新建窗口/新建无痕窗口"、Explorer 的"固定/最近"）
2. **最近/常用文件**：该应用打开过的文件
3. **固定到任务栏 / 取消固定**
4. **关闭窗口**（有多个窗口时）/ **关闭所有窗口**
5. 应用自己的条目（有些应用会往这里加东西）

任务和最近文件来自 **Jump List**（`ICustomDestinationList`），是应用主动注册的。

Win11 还会在按钮上显示"最近文件"的子菜单（如果应用提供了）。

### 4.2 Nectar 现状

现有 `dockmenu.*` 项：

```
pin / unpin          固定
closeWindow          关闭窗口
newInstance          打开新实例
hide                 从 Dock 隐藏
changeIcon / resetIcon   改图标
quitApp              退出
running              运行中
```

**没有**：任务、最近文件、关闭所有窗口。

### 4.3 需要的改动

| 项 | 可行性 | 做法 |
|---|---|---|
| **关闭所有窗口** | ✅ 容易 | 遍历 `all_hwnds` 逐个 `PostMessage(WM_CLOSE)`；已有的 `end_task` 可以做更彻底的版本 |
| **任务（Jump List）** | ❌ 不可行 | `ICustomDestinationList` 的数据归应用所有，第三方读不到 |
| **最近文件** | ⚠️ 部分可行 | 可读 `%APPDATA%\Microsoft\Windows\Recent` 的 `.lnk`，但无法可靠归属到某个应用；Explorer 的"快速访问"有单独存储 |

**建议**：
1. 加"关闭所有窗口"——纯收益，实现简单。
2. Jump List 明确写进"架构限制"，不做。
3. 最近文件做一个**可选实验**：解析 `.lnk` 的 `System.AppUserModel.ID` 尝试归属，效果不好就不做。

---

## 5. 键盘与鼠标交互

### 5.1 原生行为矩阵

| 操作 | 结果 |
|---|---|
| **左键单击**（未运行） | 启动 |
| **左键单击**（运行，单窗口） | 聚焦；已聚焦则最小化 |
| **左键单击**（运行，多窗口） | 聚焦最近一个；再点循环到下一个 |
| **中键单击** | 新开一个实例 |
| **Shift + 左键** | 新开一个实例 |
| **Ctrl + Shift + 左键** | 以管理员身份启动 |
| **右键** | Jump List 菜单 |
| **Shift + 右键** | 完整经典菜单（含"还原/最小化/关闭"） |
| **Ctrl + 左键**（多个已选） | 多选加入组 |
| **拖动图标** | 重排；拖到托盘区 = 隐藏 |
| **拖文件到图标** | 用该应用打开文件 |
| **悬停** | 预览 |

### 5.2 Nectar 现状

| 操作 | 状态 |
|---|---|
| 左键单击 | ✅（含"已聚焦则最小化"？需确认——`focus_window` 里有 `should_minimize` 逻辑） |
| 中键 | ✅ `handleMiddleClick` → `launch_new_instance` |
| 右键 | ✅ 自定义菜单 |
| **Shift + 左键** | ❌ |
| **Ctrl + Shift + 左键** | ❌ |
| **Shift + 右键**（经典菜单） | ❌ |
| **拖动图标** | ✅ 重排 |
| **拖文件到图标** | ⚠️ 只在 `dock-file-drag` 里有 hover 高亮，**未实现"用它打开"** |
| 拖到托盘区隐藏 | ⚠️ 有"从 Dock 隐藏"菜单项，但无拖动 |

### 5.3 需要的改动

1. **Shift + 左键 → 新实例**：`handleAppClick` 里读 `e.shiftKey`，改走 `launch_new_instance`。**简单**。
2. **Ctrl + Shift + 左键 → 管理员启动**：需要 `ShellExecute` 带 `runas` verb。已有 `native_uninstall` 里的 `relaunch_elevated` 可参考。**中等**，要考虑 UAC 提示。
3. **Shift + 右键 → 经典菜单**：在现有菜单上追加"还原/最小化/最大化/关闭"一组。**容易**。
4. **拖文件到图标 → 打开文件**：`shelf.rs` 已有 `IDropTarget` 基础设施，但只挂在 notch/dock 窗口本身。需要为每个图标加细分命中测试，把 `CF_HDROP` 的路径作为参数传给 `launch_path`。**中等**。

> **注意**：第 4 项原生行为是拖文件上去时图标高亮、松手后应用打开该文件。Nectar 的 `Dock.tsx` 里已有 `dock-file-drag` 事件（hover 时高亮图标），但松手没有动作——这是半成品。

---

## 6. 系统托盘区

### 6.1 原生行为

| 行为 | 说明 |
|---|---|
| 显示所有图标 | 用户可把图标"固定显示"在任务栏上，其余收进溢出面板 |
| 溢出面板 | 点击 `^` 或悬停展开，网格排列 |
| 拖动图标 | 在"显示"与"溢出"之间拖 |
| 图标左键 | 打开应用/菜单 |
| 图标右键 | 上下文菜单（U 盘弹出在这里） |
| 图标悬停 | 工具提示（`szTip`） |
| 双击 | 部分应用区分双击 |
| 排序 | 固定显示区从左到右，新增的追加在右 |

### 6.2 Nectar 现状

刚实现的托盘文件夹（`TrayFolder.tsx` + `tray_host.rs`）：

| 能力 | 状态 |
|---|---|
| 枚举图标 | ✅ `Shell_NotifyIconGetRect` + uID 扫描 |
| 图标点击 | ✅ 真实像素点击（左/右/中/双击） |
| 悬停展开 | ✅ |
| 2×2 预览 | ✅ |
| 显示系统图标（音量/网络/电池） | ⚠️ 只显示注册了 `Shell_NotifyIcon` 的，**系统内置图标不可见** |
| 工具提示 | ❌ 未实现（`szTip` 读不到——`NOTIFYICONDATA` 是私有结构） |
| 溢出/固定分区 | ❌ 未区分 |
| 拖动重排 | ❌ 无法（顺序由 shell 决定） |

### 6.3 已知技术边界（重要）

Windows 11 的任务栏是 XAML，**没有 `ToolbarWindow32`**，所以：

- **枚举**只能靠 `Shell_NotifyIconGetRect` 反查 ✅
- **图标位图**读不到 ❌（Nectar 用应用 exe 图标替代）
- **工具提示文本**读不到 ❌（`NOTIFYICONDATA.szTip` 只在应用和 shell 之间）
- **固定/溢出分区**读不到 ❌

这些必须在文档里写清楚，否则用户会以为是 bug。

### 6.4 需要的改动

1. **系统图标**：音量/网络/电池是 Win11 XAML 自绘的，不在 `Shell_NotifyIcon` 体系里。Nectar 的 Control Center 已有这些控件（`cc.volume` / `cc.wifi` / `cc.batterySaver`），**建议不在托盘重复**，而是在 Dock 托盘文件夹里提供一个"打开 Control Center"的入口。
2. **工具提示**：能拿到进程名和应用名，可以自己生成提示（"微信 · Weixin.exe"），虽然不是 `szTip`，但够用。**容易**。
3. **右键原生菜单**：已实现 ✅。

---

## 7. 通知与未读

### 7.1 原生行为

- 应用有未读 → 按钮出现橙色/红色角标（应用自己通过 `SetOverlayIcon` 或标题里的数字实现）
- 新通知到达 → 按钮闪烁叠加
- 点击通知中心 → 角标消失

### 7.2 Nectar 现状

- `attention.rs` 监听 `HSHELL_FLASH`，解析窗口标题里的 `(3)` `(12+)` 等模式
- Dock 显示 `WindowDots` 和 `attentionBadge`

### 7.3 缺口

**实测发现**：微信、QQ 这类应用**从不上报 `HSHELL_FLASH`**。它们的"闪烁"是自绘托盘图标（`Shell_NotifyIcon(NIM_MODIFY)` 循环换图），没有任何 API 能订阅。

这意味着：

- 标准 Win32/Electron 应用（Slack、Spotify、Discord）→ 能检测到 ✅
- 微信/QQ/Tim → **检测不到** ❌

### 7.4 可行的改进

唯一可行路径是**读托盘图标像素**：

1. `Shell_NotifyIconGetRect` 拿到图标矩形
2. 让任务栏以 `alpha=0` 短暂显示（有真实绘制但不显示）
3. `BitBlt` 从屏幕 DC 抓那一小块
4. 与上次快照比对，变化即认为"有新消息"

**但**：
- 需要持续截屏（每 2 秒一次，只截 42×48 小块，开销很小）
- 杀软可能对"截屏"敏感
- 任务栏隐藏时坐标失效，必须先显示（已在 `TaskbarShown` 里实现）

**建议**：作为可选功能，默认关闭，设置里明确说明它在截屏。

---

## 8. 时钟

### 8.1 原生行为

见 1.2。

### 8.2 Nectar 现状

**Dock 里没有时钟。** 这是最明显的功能缺失。

### 8.3 需要的改动

在托盘文件夹右侧加一个时钟块：

```
┌─────────────────┐
│   14:32         │   ← 时间，点击打开 Calendar
│   2026/11/17    │   ← 日期，悬停显示完整
└─────────────────┘
```

- 复用 `NotchClock` 的时间格式化逻辑（`nectar-time-format-24h`）
- 点击 → `open_calendar_window`
- 悬停 → tooltip 显示 "2026年11月17日 星期二"
- 右键 → 系统日期时间设置
- 宽度随 Dock 缩放变化
- 新设置 `nectar-dock-clock-enabled`，默认 `true`

**工作量**：小。

---

## 9. 通知中心

### 9.1 原生行为

点击时钟或最右的通知图标 → 打开通知中心：
- 左侧：通知列表（按应用分组）
- 右侧：日历
- 底部：快速设置

### 9.2 Nectar 现状

- 有 `open_notification_center`（用 `Win+N` 模拟）
- 有 `nectar-clock-tab`（刘海里的日历标签）

### 9.3 缺口

**无法读取通知内容**——`UserNotificationListener` 虽然存在且 `AccessStatus = Allowed`，但需要应用注册为包身份（`AppUserModelID` + 包清单）。Nectar 是非打包 Win32 应用，**用不了**。

实测：直接从 PowerShell 调 `GetNotificationsAsync` 可以成功（因为 PowerShell 有包身份），但 Nectar 的 exe 不行。

**结论**：通知中心保持"打开原生面板"，不做自绘列表。写进文档。

---

## 10. 多显示器

### 10.1 原生行为

- 每个显示器有独立任务栏
- 设置可控制：只显示主屏 / 所有屏
- 副屏任务栏只显示该屏的窗口（"在所有显示器上显示任务栏的应用按钮"）
- 每个任务栏独立托盘

### 10.2 Nectar 现状

- ✅ 每显示器独立 Dock（`dock-{monitor_id}`）
- ✅ `nectar-dock-monitor-mode`：`primary` / `all` / `specific`
- ⚠️ **窗口归属**：`activeApps` 是全局的，没有按显示器过滤

### 10.3 需要的改动

原生在副屏任务栏上只显示该屏的窗口。Nectar 目前所有 Dock 显示同样的应用列表。

**改动**：
1. `get_active_windows` 返回每个窗口的显示器 ID
2. Dock 侧按自己的 label 后缀（`dock-{monitor_id}`）过滤
3. 加设置 `nectar-dock-filter-by-monitor`，默认 `false`（避免老用户突然发现图标变少）

**工作量**：中等。需要在 Rust 侧给每个 hwnd 算 `MonitorFromWindow`。

---

## 11. 全屏与自动隐藏

### 11.1 原生行为

- 全屏应用（游戏、视频）→ 任务栏自动隐藏
- 鼠标移到屏幕底边 → 任务栏滑出（可以盖在全屏应用上）
- 设置里可选"在全屏时隐藏"

### 11.2 Nectar 现状

- ✅ `services.rs` 里有 `CURRENT_FOREGROUND_FULLSCREEN` 检测
- ✅ Smart / Peek 模式下全屏时隐藏
- ✅ 边缘悬停唤出

**这块已经和原生一致。**

---

## 12. 优先级与工作量汇总

按"用户感知收益 ÷ 实现成本"排序。

| # | 缺口 | 收益 | 成本 | 优先级 |
|---|---|---|---|---|
| 1 | **时钟** | 高（没有时间就没法当任务栏用） | 小 | **P0** |
| 2 | **Shift+点击新实例** | 中 | 很小 | **P0** |
| 3 | **关闭所有窗口** | 中 | 小 | **P1** |
| 4 | **Shift+右键经典菜单** | 中 | 小 | **P1** |
| 5 | **拖文件到图标打开** | 高（半成品修复） | 中 | **P1** |
| 6 | 托盘图标工具提示 | 低 | 小 | P2 |
| 7 | Aero Peek 高亮 | 中 | 中 | P2 |
| 8 | Ctrl+Shift 管理员启动 | 中 | 中 | P2 |
| 9 | 按显示器过滤窗口 | 中 | 中 | P2 |
| 10 | 不合并模式 | 中 | 大 | P3 |
| 11 | 托盘图标像素检测未读 | 中 | 中 | P3（可选） |

### 明确不做（架构限制）

这些不是"还没做"，是**在 Nectar 的架构下做不到**，文档里要写清楚：

| 项 | 原因 |
|---|---|
| 任务栏进度条 | `ITaskbarList3::SetProgressValue` 由应用调用，替代任务栏收不到 |
| 覆盖图标（overlay badge） | 同上，`SetOverlayIcon` |
| Thumbnail Toolbar | `ThumbBarAddButtons`，同上 |
| Jump List 任务与最近文件 | `ICustomDestinationList`，数据归应用所有 |
| 原生托盘图标位图 | Win11 XAML 私有，只能拿坐标 |
| 原生托盘工具提示文本 | `NOTIFYICONDATA.szTip` 不对外 |
| 托盘"固定/溢出"分区 | 同上 |
| 自绘通知中心列表 | 非打包应用无法用 `UserNotificationListener` |

---

## 13. 建议的实施顺序

**第一批（P0，一天内）**
1. Dock 时钟
2. Shift+点击新实例

**第二批（P1，两三天）**
3. 关闭所有窗口
4. Shift+右键经典菜单
5. 拖文件到图标打开（补完 `dock-file-drag` 的落点处理）

**第三批（P2，按需）**
6. 托盘工具提示
7. Aero Peek
8. 管理员启动
9. 按显示器过滤

**不做**：进度条、overlay、Thumbnail Toolbar、Jump List、通知中心自绘。

---

## 附录：当前实现索引

| 能力 | 位置 |
|---|---|
| 窗口枚举与分组 | `src-tauri/src/services.rs` → `enum_windows_proc` |
| 应用聚合 | `src-tauri/src/commands.rs` → `get_active_windows` |
| 托盘枚举 | `src-tauri/src/tray_host.rs` → `enumerate` |
| 托盘点击 | `src-tauri/src/tray_host.rs` → `click` |
| 托盘 UI | `src/components/TrayFolder.tsx` |
| 关注检测 | `src-tauri/src/attention.rs` |
| 预览 | `src/live_preview.rs` + `Dock.tsx` → `DockPreviewTooltip` |
| 全屏检测 | `src-tauri/src/services.rs` → `is_window_fullscreen` |
| Dock 设置 | `SETTINGS.md` 的 `nectar-dock-*` 一节 |
