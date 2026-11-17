# Windows 11 任务栏行为分析

这份文档分析 Windows 11 任务栏的行为，对照 Nectar 的现状，列出还差什么以及该怎么改。

它是一份**分析与计划**，不是实现记录。代码引用指向当前分支（`security/privacy-hardening`）。

**先看 §0**：Nectar 把状态信息放在刘海，把启动和切换放在 Dock，所以任务栏有而 Dock 没有，并不等于缺口。

---

## 0. 先读这一节：Nectar 不是任务栏的克隆

对照原生行为时最容易犯的错误，是把"任务栏有、Dock 没有"的每一项都算成缺口。Nectar 的设计是**两个表面分工**：

| 表面 | 职责 | 承载的信息 |
|---|---|---|
| **刘海** | 状态与即时操作 | 时间、天气、电池、CPU、内存、磁盘、网络、媒体、日历、计时器、音量/亮度、Wi-Fi/蓝牙/勿扰/托盘入口 |
| **Dock** | 启动与窗口切换 | 固定的应用、运行中的应用、预览、第三方托盘图标、显示桌面 |

原生任务栏把这两件事压在一个水平条里，所以它右侧既有图标又有时间和状态。Nectar 把它们拆开了。

**判断原则**：如果某项信息已经在刘海里有等价呈现，它就不是 Dock 的缺口，而是分工选择。下面每一节都会先说明"刘海是否已覆盖"，只有两侧都没有的才算真正的 gap。

### 刘海已覆盖的能力（不算 gap）

| 原任务栏右侧 | 刘海对应物 |
|---|---|
| 时钟（时间） | `TimeDisplay`（`src/App.tsx`，跟随 `nectar-time-format-24h`） |
| 音量图标 + 滑块 | Control Center 音量磁贴 + 滑块 |
| 网络图标 | Control Center Wi-Fi / 以太网磁贴 |
| 电池图标 | 状态小组件 `battery`（含低电量、充电脉冲） |
| 通知中心入口 | Control Center 的通知按钮 |
| 系统托盘 / 隐藏图标 | Control Center 的 `tray` 按钮 |
| 日历浮出 | `calendar` 面板（日/周/月/日程 + 计时器 + 秒表） |
| 输入法 / 语言 | 不需要，跟随系统 |
| — | 额外：天气、CPU、内存、磁盘、网络速率、媒体播放、双时钟 |

**结论**：Dock 不需要复制这些。Dock 的托盘区只需要管**第三方应用的托盘图标**，那才是 Dock 独有的职责。

---

## 1. 任务栏的解剖

Win11 任务栏从左到右分为四段。理解这个分区是理解后面所有差异的前提。

```
┌────────┬──────────────────────────┬────────────────────────┬──────────┐
│ 开始   │      应用按钮区           │    系统托盘区          │ 显示桌面 │
│ Widgets│  (大图标，任务分组)       │  (小图标 + 时钟)       │  细条    │
└────────┴──────────────────────────┴────────────────────────┴──────────┘
```

| 区域 | Nectar 对应 | 状态 |
|---|---|---|
| 左侧（开始 / 搜索 / 任务视图） | Dock 左侧按钮 | ✅ 已覆盖 |
| 左侧（Widgets / 聊天） | 无 | ⚪ 不打算做（刘海已提供更强的状态展示） |
| 中间（应用按钮） | Dock 应用区 | ✅ 基本覆盖，见 §2 |
| 右侧（第三方托盘图标） | Dock 托盘文件夹 | ✅ 本轮实现 |
| 右侧（系统图标 / 时钟） | **刘海** | ⚪ 已在刘海，不算 Dock 缺口 |
| 最右（显示桌面） | Dock 细条 | ✅ |

### 1.1 应用按钮的六种视觉状态

原生一个按钮可以同时表达多层信息。Nectar 目前只实现了前三层。

| 状态                    | 原生表现               | Nectar                                                   |
| ----------------------- | ---------------------- | -------------------------------------------------------- |
| 未运行（仅固定）        | 只有图标               | ✅                                                        |
| 运行中                  | 图标 + 底部横条/圆点   | ✅ 可配置条形/固定/圆点                                   |
| 有多个窗口              | 横条变长 / 多个圆点    | ✅                                                        |
| **需要关注**（闪烁）    | 按钮背景闪烁橙色       | ⚠️ 只有笔记本头上的圆点，且仅对触发 `HSHELL_FLASH` 的应用 |
| **有进度**              | 图标下方一条绿色进度条 | ❌ 完全没有                                               |
| **覆盖图标**（overlay） | 图标右下角小徽标       | ❌ 完全没有                                               |

> 后两项来自 `ITaskbarList3`：`SetProgressValue` 和 `SetOverlayIcon`。这两个 API 由**应用主动调用**，所以 Nectar 不能"查询"谁有进度——它自己的窗口不在原生任务栏上，也就收不到这些值。
>
> **结论：进度条和覆盖图标在 Nectar 架构下无法实现。** 这是替代任务栏的根本限制，需要在文档里明确写出来，而不是假装能做。

### 1.2 时钟区域

原生右侧是时间 + 日期，点击打开通知中心。

**刘海已经有时间**（`TimeDisplay`，跟随 `nectar-time-format-24h`），点击刘海可展开 `calendar` 面板看完整日历。所以这不是 Dock 的缺口。

唯一需要注意的是：**如果用户隐藏刘海、只用 Dock，就看不到时间**。这一点在 §8 单独讨论，作为可选项而不是必备功能。

## 2. 分组行为

### 2.1 原生行为

| 设置                 | 行为                                                               |
| -------------------- | ------------------------------------------------------------------ |
| **合并按钮**（默认） | 同一应用的所有窗口合成一个按钮；悬停显示预览画廊；点击在窗口间循环 |
| **从不合并**         | 每个窗口一个独立按钮，带标题                                       |
| **已满时合并**       | 空间不足时才合并                                                   |

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

| 能力                  | 状态                                           |
| --------------------- | ---------------------------------------------- |
| 悬停延迟可调          | ✅ `nectar-dock-preview-delay`                  |
| 缩略图预览            | ✅ `capture_window_thumbnail` + DWM 实时缩略图  |
| 预览里关窗            | ✅ `close_window`                               |
| 点击预览切换          | ✅                                              |
| **Thumbnail Toolbar** | ❌ 无法实现（同 1.1，应用主动注册的按钮收不到） |
| **Aero Peek 高亮**    | ❌ 未实现                                       |

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

### 4.3 实测结论：Jump List 无法读取

这一条经过专门验证，不是凭印象下的结论。三条路线都试过：

| 路线 | 结果 |
|---|---|
| `IShellItem` → `IShellFolder::GetUIObjectOf(IContextMenu)` | 能拿到 30 项菜单，但**全是文件操作**（打开/管理员运行/复制/剪切/删除/属性/发送到/第三方 shell 扩展），**没有任何 Jump List 条目** |
| `CLSID_ApplicationDestinations` | 文档明确写"An application calls the methods of this interface when it wants to **remove items from** a Jump List's automatically generated destinations"，且 "not implemented by third parties"。**只写不读，只操作自己** |
| 直接解析 `%APPDATA%\Microsoft\Windows\Recent\AutomaticDestinations\*.automaticDestinations-ms` | ✅ 能解出真实条目（实测从 Word 的列表里读到了 `https://d.docs.live.net/.../xxx.docx`），**但文件名是 AppUserModelID 的 CRC-64 哈希**，试了 4 种多项式 × 2 种编码 × refin/refout 组合共 16 种组合，**全部无法匹配已知 AUMID**。归属不到应用，列表就没有意义 |

**根因**：Jump List 属于"任务栏按钮"，不属于"文件"。`IContextMenu` 走的是"exe 作为文件"的路径，而任务栏按钮的菜单是 `ITaskbarList3` 的扩展，没有暴露给第三方。

所以：

- **任务条目的读取：做不到**
- **最近文件的读取：技术上能做，但无法归属到应用，实际不可用**

### 4.4 能做的部分

| 项 | 可行性 | 做法 |
|---|---|---|
| **关闭所有窗口** | ✅ 容易 | 遍历 `all_hwnds` 逐个 `WM_CLOSE` |
| **关闭窗口** | ✅ 已有 | `close_window` |
| **新实例** | ✅ 已有 | `launch_new_instance` |
| 任务（Jump List Tasks） | ❌ 读不到 | 见 4.3 |
| 最近文件 | ⚠️ 不可用 | 能读但归属不到应用 |

**建议**：只做"关闭所有窗口"。

> 如果将来想再试，唯一没验证过的方向是用 `ICustomDestinationList::BeginList` 的 `IObjectArray` 返回值——但它同样只针对调用者自己的 AppID，且需要把 Nectar 伪装成目标应用，不现实。

## 5. 键盘与鼠标交互

### 5.1 原生行为矩阵

| 操作                         | 结果                                 |
| ---------------------------- | ------------------------------------ |
| **左键单击**（未运行）       | 启动                                 |
| **左键单击**（运行，单窗口） | 聚焦；已聚焦则最小化                 |
| **左键单击**（运行，多窗口） | 聚焦最近一个；再点循环到下一个       |
| **中键单击**                 | 新开一个实例                         |
| **Shift + 左键**             | 新开一个实例                         |
| **Ctrl + Shift + 左键**      | 以管理员身份启动                     |
| **右键**                     | Jump List 菜单                       |
| **Shift + 右键**             | 完整经典菜单（含"还原/最小化/关闭"） |
| **Ctrl + 左键**（多个已选）  | 多选加入组                           |
| **拖动图标**                 | 重排；拖到托盘区 = 隐藏              |
| **拖文件到图标**             | 用该应用打开文件                     |
| **悬停**                     | 预览                                 |

### 5.2 Nectar 现状

| 操作                         | 状态                                                                        |
| ---------------------------- | --------------------------------------------------------------------------- |
| 左键单击                     | ✅（含"已聚焦则最小化"？需确认——`focus_window` 里有 `should_minimize` 逻辑） |
| 中键                         | ✅ `handleMiddleClick` → `launch_new_instance`                               |
| 右键                         | ✅ 自定义菜单                                                                |
| **Shift + 左键**             | ❌                                                                           |
| **Ctrl + Shift + 左键**      | ❌                                                                           |
| **Shift + 右键**（经典菜单） | ❌                                                                           |
| **拖动图标**                 | ✅ 重排                                                                      |
| **拖文件到图标**             | ⚠️ 只在 `dock-file-drag` 里有 hover 高亮，**未实现"用它打开"**               |
| 拖到托盘区隐藏               | ⚠️ 有"从 Dock 隐藏"菜单项，但无拖动                                          |

### 5.3 需要的改动

1. **Shift + 左键 → 新实例**：`handleAppClick` 里读 `e.shiftKey`，改走 `launch_new_instance`。**简单**。
2. **Ctrl + Shift + 左键 → 管理员启动**：需要 `ShellExecute` 带 `runas` verb。已有 `native_uninstall` 里的 `relaunch_elevated` 可参考。**中等**，要考虑 UAC 提示。
3. **Shift + 右键 → 经典菜单**：在现有菜单上追加"还原/最小化/最大化/关闭"一组。**容易**。
4. **拖文件到图标 → 打开文件**：`shelf.rs` 已有 `IDropTarget` 基础设施，但只挂在 notch/dock 窗口本身。需要为每个图标加细分命中测试，把 `CF_HDROP` 的路径作为参数传给 `launch_path`。**中等**。

> **注意**：第 4 项原生行为是拖文件上去时图标高亮、松手后应用打开该文件。Nectar 的 `Dock.tsx` 里已有 `dock-file-drag` 事件（hover 时高亮图标），但松手没有动作——这是半成品。

---

## 6. 系统托盘区

### 6.1 原生行为

| 行为         | 说明                                               |
| ------------ | -------------------------------------------------- |
| 显示所有图标 | 用户可把图标"固定显示"在任务栏上，其余收进溢出面板 |
| 溢出面板     | 点击 `^` 或悬停展开，网格排列                      |
| 拖动图标     | 在"显示"与"溢出"之间拖                             |
| 图标左键     | 打开应用/菜单                                      |
| 图标右键     | 上下文菜单（U 盘弹出在这里）                       |
| 图标悬停     | 工具提示（`szTip`）                                |
| 双击         | 部分应用区分双击                                   |
| 排序         | 固定显示区从左到右，新增的追加在右                 |

### 6.2 Nectar 现状

刚实现的托盘文件夹（`TrayFolder.tsx` + `tray_host.rs`）：

| 能力                           | 状态                                                         |
| ------------------------------ | ------------------------------------------------------------ |
| 枚举图标                       | ✅ `Shell_NotifyIconGetRect` + uID 扫描                       |
| 图标点击                       | ✅ 真实像素点击（左/右/中/双击）                              |
| 悬停展开                       | ✅                                                            |
| 2×2 预览                       | ✅                                                            |
| 显示系统图标（音量/网络/电池） | ⚠️ 只显示注册了 `Shell_NotifyIcon` 的，**系统内置图标不可见** |
| 工具提示                       | ❌ 未实现（`szTip` 读不到——`NOTIFYICONDATA` 是私有结构）      |
| 溢出/固定分区                  | ❌ 未区分                                                     |
| 拖动重排                       | ❌ 无法（顺序由 shell 决定）                                  |

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

## 8. 时钟（可选，不算核心缺口）

**刘海已有时间**：`TimeDisplay`（`src/App.tsx`），跟随 `nectar-time-format-24h`。展开刘海可看 `calendar` 面板的完整日历。

所以 Dock 不需要时钟。**唯一的场景**是用户把刘海隐藏起来、只留 Dock——那时看不到时间。

### 如果要做

在托盘文件夹右侧放一个极简日期块（只显示日号或 `11/17`），点击打开 `calendar` 面板。

- 新设置 `nectar-dock-date-enabled`，**默认关闭**（与刘海重复）
- 只在用户关闭刘海时提示可以开启
- **工作量**：小

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

按"用户感知收益 ÷ 实现成本"排序。注意：**Dock 的内建元素（时钟、音量、网络、电池）不在此表**，它们由刘海负责，见 §0。

| # | 缺口 | 收益 | 成本 | 优先级 |
|---|---|---|---|---|
| 1 | **拖文件到图标打开**（半成品修复） | 高 | 中 | **P1** |
| 2 | **Shift+点击新实例** | 中 | 很小 | **P1** |
| 3 | **关闭所有窗口** | 中 | 小 | **P1** |
| 4 | **Shift+右键经典菜单** | 中 | 小 | **P2** |
| 5 | 托盘工具提示（用应用名生成） | 低 | 小 | **P2** |
| 6 | Ctrl+Shift 管理员启动 | 中 | 中 | P2 |
| 7 | Aero Peek 高亮 | 中 | 中 | P2 |
| 8 | 按显示器过滤窗口 | 中 | 中 | P2 |
| 9 | 可选日期（仅隐藏刘海时有用） | 低 | 小 | P3 |
| 10 | 不合并模式 | 中 | 大 | P3 |
| 11 | 托盘图标像素检测未读 | 中 | 中 | P3（可选） |

### 已实现，不需要再做

这些曾经是缺口，现在已经补上：

| 能力 | 位置 |
|---|---|
| 托盘图标枚举与点击（左/右/中/双击） | `tray_host.rs` + `TrayFolder.tsx` |
| 托盘悬停展开 | `TrayFolder.tsx` |
| U 盘弹出（托盘右键菜单） | 通过原生菜单转发 |
| 关闭到托盘应用的恢复 | `restore_app_window` |
| pinned 应用不在托盘重复 | `trayIconsShown` 过滤 |
| 多显示器独立 Dock | `monitors.rs` |
| 全屏自动隐藏 | `services.rs` |

### 明确不做（架构限制）

这些不是"还没做"，是**做不到**：

| 项 | 原因 |
|---|---|
| 任务栏进度条 | `ITaskbarList3::SetProgressValue` 由应用调用，替代任务栏收不到 |
| 覆盖图标（overlay badge） | 同上，`SetOverlayIcon` |
| Thumbnail Toolbar | `ThumbBarAddButtons`，同上 |
| Jump List 任务与最近文件 | 见 §4.3 实测：无读取 API；`*.automaticDestinations-ms` 能解析但 AUMID 哈希无法破解 |
| 原生托盘图标位图 | Win11 XAML 私有，只能拿坐标 |
| 原生托盘工具提示文本 | `NOTIFYICONDATA.szTip` 不对外 |
| 托盘"固定/溢出"分区 | 同上 |
| 自绘通知中心列表 | 非打包应用无法用 `UserNotificationListener` |
| Widgets / 聊天 | 刘海已提供更强的状态展示，不打算复制 |

## 13. 建议的实施顺序

**第一批（P1，两三天）**

1. 拖文件到图标打开——补完 `dock-file-drag` 的落点处理，这是现有半成品
2. Shift+点击新实例——`handleAppClick` 里读 `e.shiftKey`
3. 关闭所有窗口——遍历 `all_hwnds` 逐个 `WM_CLOSE`

**第二批（P2，按需）**

4. Shift+右键经典菜单
5. 托盘工具提示
6. Aero Peek
7. 管理员启动
8. 按显示器过滤

**第三批（P3，可选）**

9. 可选日期显示（只在用户隐藏刘海时才有意义）
10. 不合并模式
11. 托盘图标像素检测未读

**不做**：进度条、overlay、Thumbnail Toolbar、Jump List、通知中心自绘、Widgets。

## 附录：当前实现索引

| 能力           | 位置                                                      |
| -------------- | --------------------------------------------------------- |
| 窗口枚举与分组 | `src-tauri/src/services.rs` → `enum_windows_proc`         |
| 应用聚合       | `src-tauri/src/commands.rs` → `get_active_windows`        |
| 托盘枚举       | `src-tauri/src/tray_host.rs` → `enumerate`                |
| 托盘点击       | `src-tauri/src/tray_host.rs` → `click`                    |
| 托盘 UI        | `src/components/TrayFolder.tsx`                           |
| 关注检测       | `src-tauri/src/attention.rs`                              |
| 预览           | `src/live_preview.rs` + `Dock.tsx` → `DockPreviewTooltip` |
| 全屏检测       | `src-tauri/src/services.rs` → `is_window_fullscreen`      |
| 刘海时间       | `src/App.tsx` → `TimeDisplay`                             |
| 刘海状态小组件 | `src/components/StatusWidgetConfig.tsx`（weather/battery/cpu/ram/disk/net） |
| 刘海 Control Center | `src/App.tsx`（Wi-Fi/蓝牙/勿扰/音量/亮度/托盘入口）  |
| Dock 设置      | `SETTINGS.md` 的 `nectar-dock-*` 一节                     |
