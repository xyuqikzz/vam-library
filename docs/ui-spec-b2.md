# VaM Library UI 规范 · B2「画廊」（v1.2：补充间距体系与实现约定）

> 本文件是 UI 改版的唯一依据。实现时如与本文冲突，以本文为准；本文没写到的，沿用现有实现，不要自行发挥。

## 0. 背景

- 技术栈：Tauri 2 + Vue 3.5（`<script setup lang="ts">`）+ Pinia + vue-router（hash）+ vue-i18n 9。
- 全局样式：`src/assets/styles/{variables,base,typography,glass,animations}.css`，在 `src/main.ts` 引入。
- 现状问题（改版要消灭的东西）：
  - `.vue` 里有约 358 处写死的颜色（hex / rgba），没走变量；
  - 按钮 class 约 40 种（`action-btn`、`page-btn`、`dup-action-btn`、`step-next-btn`…），弹窗至少 8 套各自实现（`modal-*`、`delete-confirm-*`、`preview-*`、`image-preview-*`、`result-dialog`…）；
  - 字号写死 `10px` 约 20 处（低于规范最小值 11px），圆角出现 5px / 7px 等非规范值；
  - 131 个内联 `<svg>`，侧边栏用 `v-html` 塞图标字符串；
  - `alert()` / `confirm()` 共 2 处。

## 1. 设计原则

1. **预览图是主角，界面退后**：资源卡片统一约 255 × 181px，整卡宽高比 45:32，大图优先；界面本身不抢色。
2. **无边框，用明度分层**：窗口底 → 内容区 → 面板 → 控件，每层亮一点；只有浮层（菜单、弹窗、悬停卡片）才有阴影。
3. **一个强调色**：橙色 `#ff8a5c` 只给「主操作」和「选中」。强调色上的文字一律用深色 `#1a0e08`。
4. **胶囊控件**：按钮、筛选片、搜索框都是全圆角；主按钮高 44，工具栏控件高 40，筛选片高 32。
5. **数字等宽**：全局 `font-variant-numeric: tabular-nums`。
6. **所有可交互元素有键盘焦点环**：2px 间隙 + 2px 白环。

## 2. 设计令牌（`variables.css`）

**策略：沿用现有变量名，只改值**。组件里已有的 `var(--…)`（约 223 处字号、234 处圆角）会自动生效。新增变量在下方以 `/* new */` 标出。

```css
:root {
  /* Backgrounds */
  --bg-base:     #0a0a0b;   /* 窗口底 */
  --bg-surface:  #111113;   /* 主内容区 */
  --bg-elevated: #19191c;   /* 卡片、面板、详情 */
  --bg-hover:    #222226;
  --bg-active:   #2a2a2e;   /* 选中、按下、次要按钮按下 */
  --bg-overlay:  rgba(6, 6, 7, 0.78);
  --bg-input:    #1c1c1f;   /* 输入框、筛选片、搜索框 */
  --bg-subtle:   rgba(255, 255, 255, 0.03);
  --bg-control:  #26262a;   /* new 次要按钮、图标按钮 */
  --bg-control-hover: #303035; /* new */
  --bg-menu:     #1f1f22;   /* new 下拉/右键菜单 */
  --bg-on-image: rgba(10, 10, 11, 0.72); /* new 叠在图片上的标签/按钮 */

  /* Text（对比度以 #111113 为底） */
  --text-primary:   #f4f2f0;  /* 17.4:1 */
  --text-secondary: #b8b4ae;  /* 9.4:1 */
  --text-tertiary:  #9c9893;  /* 6.6:1（原 #6e6e73 仅 3.6:1，不达标） */
  --text-disabled:  #5c5955;
  --text-inverse:   #111113;
  --text-muted-strong: #d6d2cc; /* new 未选中筛选片、幽灵按钮文字 */

  /* Accent */
  --accent-primary:        #ff8a5c;
  --accent-primary-hover:  #ff9d76;
  --accent-primary-active: #f07a4c;
  --accent-secondary:      #ff9d76;
  --accent-secondary-hover:#ffb08f;
  --accent-gradient:       #ff8a5c;   /* 不再使用渐变；保留变量名，值改为纯色 */
  --accent-muted:          rgba(255, 138, 92, 0.16);
  --accent-subtle:         rgba(255, 138, 92, 0.08);
  --accent-ink:            #1a0e08;   /* new 强调色上的文字/图标 */

  /* Semantic（背景统一 14%） */
  --color-success:    #5fd68a;  --color-success-bg: rgba(95, 214, 138, 0.14);
  --color-warning:    #f5c451;  --color-warning-bg: rgba(245, 196, 81, 0.14);
  --color-warning-ink:#241a02;  /* new 黄底上的文字（缺依赖角标、补全按钮） */
  --color-error:      #ff7a6e;  --color-error-bg:   rgba(255, 122, 110, 0.14);
  --color-error-surface: #2e1a19;       /* new 危险按钮底 */
  --color-error-surface-hover: #3a1f1d; /* new */
  --color-info:       #7cc4ff;  --color-info-bg:    rgba(124, 196, 255, 0.14);

  /* Resource types：只做圆点/细标，不铺大色块 */
  --color-scene:      #f5b041;  --color-scene-bg:      rgba(245, 176, 65, 0.14);
  --color-appearance: #f27aa8;  --color-appearance-bg: rgba(242, 122, 168, 0.14);
  --color-morph:      #b096fa;  --color-morph-bg:      rgba(176, 150, 250, 0.14);
  --color-clothing:   #4fcbe0;  --color-clothing-bg:   rgba(79, 203, 224, 0.14);
  --color-plugin:     #5fd68a;  --color-plugin-bg:     rgba(95, 214, 138, 0.14);
  --color-texture:    #ff935a;  --color-texture-bg:    rgba(255, 147, 90, 0.14);
  --color-asset:      #a3abff;  --color-asset-bg:      rgba(163, 171, 255, 0.14);
  --color-hair:       #d4a8ff;  --color-hair-bg:       rgba(212, 168, 255, 0.14);

  /* new 无预览图时卡片的占位底色（按类型） */
  --thumb-scene: #4a3524; --thumb-appearance: #46263a; --thumb-clothing: #21384a; --thumb-hair: #3a2a4a;
  --thumb-morph: #352a4a; --thumb-texture: #4a3a22;    --thumb-plugin: #1e2a23;   --thumb-asset: #2a3240;
  --thumb-other: #232327;

  /* Borders（B 风格基本不用边框，仅分隔线/输入焦点） */
  --border-subtle:  rgba(255, 255, 255, 0.05);
  --border-default: rgba(255, 255, 255, 0.08);
  --border-strong:  rgba(255, 255, 255, 0.12);
  --border-accent:  rgba(255, 138, 92, 0.45);
  --divider:        #232327;  /* new 导航分组线、列表分隔 */

  /* Radius */
  --radius-xs: 6px;    /* 小标签 */
  --radius-sm: 10px;   /* 小缩略图、菜单项 */
  --radius-md: 14px;   /* 输入框、小卡片 */
  --radius-lg: 18px;   /* 面板 */
  --radius-xl: 24px;   /* 弹窗 */
  --radius-card: 16px; /* new 包卡片 */
  --radius-shell: 20px;/* new 导航栏、主内容区外壳 */
  --radius-full: 9999px;

  /* Shadows：只用于浮层 */
  --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.4);
  --shadow-md: 0 12px 28px rgba(0, 0, 0, 0.45); /* 卡片悬停 */
  --shadow-lg: 0 24px 80px rgba(0, 0, 0, 0.6);  /* 菜单、弹窗、浮动操作栏 */
  --shadow-float: var(--shadow-lg);             /* new */
  --focus-ring: 0 0 0 2px var(--bg-surface), 0 0 0 4px var(--text-primary); /* new */
  --ring-selected: 0 0 0 2px var(--bg-surface), 0 0 0 4px var(--accent-primary); /* new */

  /* Glass（保留变量名兼容旧代码） */
  --glass-bg: #19191c; --glass-bg-heavy: #1f1f22;
  --glass-blur: none; --glass-blur-light: none; --glass-blur-heavy: none;
  --glass-border: 1px solid transparent;
  --glass-shadow: none; --glass-shadow-lg: var(--shadow-lg);
  --glass-panel-bg: #19191c; --glass-sidebar-bg: #141416; --glass-toolbar-bg: #111113;

  /* Typography */
  --font-family: 'Manrope Variable', 'Manrope', 'PingFang SC', 'Microsoft YaHei UI', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', 'Cascadia Mono', 'Consolas', monospace;
  /* 字号变量值不变：xs 11 / sm 12 / base 13 / md 14 / lg 16 / xl 20 / 2xl 24 / 3xl 28 */
  --font-heavy: 800; /* new 页面标题、卡片名、主按钮 */

  /* Controls & layout */
  --control-h-sm: 32px; /* new 筛选片、小按钮 */
  --control-h-md: 40px; /* new 工具栏按钮、搜索框 */
  --control-h-lg: 44px; /* new 主按钮、详情操作、输入框 */
  --app-gutter: 8px;    /* new 窗口内边距与外壳间距 */
  --sidebar-width: 76px;
  --sidebar-collapsed-width: 76px;
  /* 其余（spacing、z-index、duration、ease）保持现值 */
}
```

字号使用对照：

| 用途 | 变量 | 字重 |
|---|---|---|
| 页面标题 | `--text-3xl`（28） | 800，`letter-spacing: -0.02em` |
| 详情标题 | `--text-xl`（20） | 800 |
| 区块标题 | `--text-lg`（16） | 800 |
| 正文 | `--text-md`（14） | 400–600 |
| 控件文字、筛选片 | `--text-base`（13） | 700 |
| 辅助信息 | `--text-sm`（12） | 500 |
| 标签、导航文字 | `--text-xs`（11） | 700 |

**禁止**：任何 < 11px 的字号；规范外的圆角值。


### 间距（v1.1 补充，必须遵守）

**规则**：布局层（页面、区块、面板、网格、列表）只用 4 的倍数，并且只用下面的语义变量；2px 级别的微调只允许出现在组件内部的小元素上（角标、圆点、分段控件内边距）。

```css
--space-page-x: var(--space-6);   /* 24 页面左右内边距，PageHeader 与内容区共用，保证左缘对齐 */
--space-page-y: var(--space-5);   /* 20 页头下方到第一块内容 */
--space-section: var(--space-6);  /* 24 大区块之间（统计行 / 双栏 / 区块） */
--space-stack: var(--space-3);    /* 12 区块标题到内容 */
--space-grid: var(--space-4);     /* 16 卡片网格、磁贴之间 */
--space-panel: var(--space-5);    /* 20 面板内边距 */
--space-item: var(--space-2);     /* 8 列表行之间、同组控件之间 */
--focus-room: var(--space-1);     /* 4 滚动容器为选中外环预留 */
```

| 场景 | 用法 |
|---|---|
| 页头 | `padding: 20px var(--space-page-x) 16px`；副标题与标题间距 4 |
| 页面内容区 | `padding: var(--space-page-y) var(--space-page-x)` |
| 区块之间 | `gap: var(--space-section)` |
| 区块标题 → 内容 | `margin-bottom: var(--space-stack)` |
| 卡片/磁贴网格 | `gap: var(--space-grid)` |
| 面板、统计磁贴 | `padding: var(--space-panel)`（磁贴可用 `16px 20px`） |
| 列表行之间 | `gap: var(--space-item)` |

**对齐规则**：

1. 所有页面内容的左缘必须与页头标题左缘对齐（都等于 `--space-page-x`）。滚动容器如果为了选中外环加了 `padding: var(--focus-room)`，必须同时加 `margin: calc(-1 * var(--focus-room))` 抵消。
2. 并排的两栏，标题必须在同一水平线上：标题放在面板**外面**，面板只包内容；不要一栏标题在面板外、另一栏在面板内。
3. 禁止的值：3 / 5 / 6 / 7 / 9 / 10 / 14 / 18 / 22px 出现在布局层的 padding / margin / gap 中。

浅色主题（`:root.light-theme`）本期不重新设计，只需保证不崩：把其中 accent 相关改为 `--accent-primary: #c2521f; --accent-ink: #ffffff;`，并给新增变量补上合理的浅色值。

### 字体

- `pnpm add @fontsource-variable/manrope`，在 `main.ts` 顶部 `import '@fontsource-variable/manrope'`（本地打包，离线可用）。
- 中文走系统字体，见 `--font-family`。
- `base.css` 的 `body` 加 `font-variant-numeric: tabular-nums;`。

## 3. 布局外壳（`AppLayout.vue` / `AppSidebar.vue`）

- 窗口：`background: var(--bg-base); padding: var(--app-gutter); gap: var(--app-gutter);`，flex 横排。
- 导航栏：宽 76px，`background: var(--glass-sidebar-bg); border-radius: var(--radius-shell)`，纵向，内边距 14px 6px。
- 主内容区：`background: var(--bg-surface); border-radius: var(--radius-shell); overflow: hidden`。
- **必须保留**：`.main-content`、`.resource-content-scrollable`、`.hub-content-scroll` 这几个 class（AppLayout 的滚动位置恢复依赖它们）；`data-tauri-drag-region`；`<keep-alive>` 与 Transition 逻辑。

### 侧边导航（图标栏）

- 顶部 Logo：36×36，圆角 12，强调色底，深色图标。
- 导航项：图标 18px，外包 44×32 的胶囊（圆角 12）；下方文字 11px / 700。
  - 默认：文字 `--text-tertiary`。
  - 当前页：胶囊底 `--bg-active`，图标 `--accent-primary`，文字 `--text-primary`。
  - 角标（下载数）：16px 高，强调色底，`--accent-ink` 字，2px 描边同导航栏底色，压在图标右上角。
- 分组之间 1px `--divider` 线（左右各缩 14px，上下 8px）：
  1. 仪表盘 · 包管理 · 统计
  2. 预设 · 收藏 · 在线 · 下载
  3. 工具（点开为浮动菜单，列出：补全依赖、资源清理、去重分析、资源迁移、快捷入库、游戏设置、回收站）
- 底部：「开始游戏」48×48，圆角 16，强调色，下面 10.5px 文字；再下面是「设置」。
- 导航文字需要短标签：在 i18n 里新增 `sidebar.short.*`（中英文都加），不要改原有 key。

### 页面头部（新建 `PageHeader.vue`，所有页面统一）

- 内边距 `20px var(--space-page-x) 16px`，底部 1px `--border-subtle` 分隔线。
- 左：标题（28 / 800）+ 副标题行（13，`--text-tertiary`，用 3px 圆点分隔，可带状态点，如绿色「已同步」）。
- 右：插槽 `#actions`，按顺序放：搜索框 → 次要按钮 → 视图切换 → 主按钮。
- 第二行插槽 `#filters`：左边是类型筛选片，右边是「目录」「有问题」等附加筛选。

## 4. 基础组件（新建 `src/components/ui/`）

所有组件：`<script setup lang="ts">`，props 有类型，样式只引用变量，不写死颜色。

### UiButton

- props：`variant: 'primary' | 'secondary' | 'ghost' | 'danger'`，`size: 'sm' | 'md' | 'lg'`（32/40/44），`icon?: IconName`，`iconOnly?: boolean`（需要 `aria-label`），`loading?: boolean`，`disabled?: boolean`。
- 形状：全圆角；内边距 sm 12 / md 16 / lg 20；图标与文字间距 8。
- **只用于单行按钮**。卡片、可选项、列表行这类多行可点击区域，一律用原生 `<button type="button">` 自己写样式（参考资源迁移的模式卡片），不要用 UiButton 包——UiButton 固定高度、单行、居中，包住多行内容会被压扁、内容溢出重叠。
- 状态表：

| 变体 | 默认 | 悬停 | 按下 | 文字 |
|---|---|---|---|---|
| primary | `--accent-primary` | `--accent-primary-hover` | `--accent-primary-active` | `--accent-ink`，800 |
| secondary | `--bg-control` | `--bg-control-hover` | `--bg-active` | `--text-primary`，700 |
| ghost | transparent | `#1f1f22` | `--bg-control` | `--text-muted-strong`，700 |
| danger | `--color-error-surface` | `--color-error-surface-hover` | `#451f1c` | `--color-error`，700 |

- 按下：`transform: scale(0.98)`；过渡 `var(--duration-fast)`。
- 键盘焦点：`:focus-visible { box-shadow: var(--focus-ring); outline: none; }`。
- 禁用：底 `--bg-input`（ghost 仍透明），文字 `--text-disabled`，`cursor: not-allowed`。
- 加载：左侧 12px 圆环（2px 边框，右边透明，旋转），文字改为传入的 `loadingText` 或 i18n `common.processing`，整体 `opacity: 0.8`，不可点击。

### UiChip（筛选片）

- 高 32，全圆角，13px / 700，内边距 0 12px，间距 6。
- 可带 7px 类型圆点、计数（`--text-tertiary`，500）。
- 状态：默认 `--bg-input` + `--text-muted-strong`；悬停 `--bg-control`；选中 `--text-primary` 底 + `--text-inverse` 字（反白），计数改为 `--text-disabled`；可移除的选中片在右侧带 20px 圆形 × 按钮。
- 用 `aria-pressed`。

### UiSegmented

- 外框 `--bg-input`，内边距 3，全圆角；选项高 34，选中项反白（同选中筛选片）；纯图标选项 40 宽，带 `aria-label`。

### UiInput

- 高 44，圆角 `--radius-md`，底 `--bg-input`，无边框，内边距 0 16px。
- 聚焦：`box-shadow: inset 0 0 0 2px var(--accent-primary)`。
- 错误：底 `rgba(255,122,110,.08)`，`inset 0 0 0 2px var(--color-error)`，下方 12px / 600 错误文字，`aria-invalid="true"`。
- 禁用：底 `#161618`，文字 `--text-disabled`。
- 搜索框变体：高 40，全圆角，左侧搜索图标，右侧可选快捷键提示（`Ctrl K`，26px 高的 `--bg-control` 胶囊）。

### UiSwitch / UiCheckbox / UiRadio

- 开关 40×24，开：强调色底 + 深色圆钮；关：`--bg-active` 底 + `--text-tertiary` 圆钮。
- 复选 20×20 圆角 6，选中强调色底 + 深色对勾；未选 2px `#3a3a3f` 描边。
- 单选 20×20 圆形，选中为 6px 强调色描边。
- 都必须是原生 `<input>` + `<label>`，样式化处理，保留键盘操作。

### UiMenu（下拉菜单 + 右键菜单共用）

- 浮层：`--bg-menu`，圆角 16，内边距 6，`box-shadow: var(--shadow-lg), inset 0 0 0 1px rgba(255,255,255,.06)`。
- 菜单项：高 36，圆角 10，13px / 600；悬停/高亮 `--bg-active`；危险项文字 `--color-error`；右侧可显示快捷键（11px，`--text-tertiary`）；单选菜单在选中项右侧显示强调色对勾。
- 分隔线 1px `--bg-active`，左右缩 8px。
- 支持 ↑↓ / Enter / Esc；`role="menu"`、`menuitem` / `menuitemradio`。
- 包卡片右键菜单项：启动场景（Enter）、打开所在位置（Ctrl O）、复制包 ID（Ctrl C）、分享…、提取预设…、补全依赖、——、移到回收站（Del，危险）。

### UiTooltip

- 反白：`--text-primary` 底，`--text-inverse` 字，高 30，圆角 10，12.5px / 700，可带快捷键；延迟 400ms 出现。
- **所有纯图标按钮必须同时有 tooltip 和 `aria-label`。**

### UiModal + useConfirm()

- 遮罩 `--bg-overlay`；弹窗 `--bg-elevated`，圆角 `--radius-xl`，内边距 22，`box-shadow: var(--shadow-lg), inset 0 0 0 1px rgba(255,255,255,.05)`。
- 结构：可选 44px 圆形图标（危险操作用 `--color-error-bg` 底 + 红色图标）→ 标题 18 / 800 → 说明 13.5 `--text-secondary` → 右对齐按钮组（取消 = secondary，确认 = primary 或 danger）。
- 危险确认按钮：底 `--color-error`，文字 `#2a0b08`。
- `role="dialog" aria-modal="true"`，打开时焦点进入、Esc 关闭、焦点锁定、关闭后焦点还原。
- 提供 `useConfirm()`：`await confirm({ title, message, danger, confirmText })` 返回 boolean；用它替换所有 `confirm()`。
- 现有弹窗（ShareModal、CleanupPreviewDialog、ExternalImportPrompt、图片预览、删除确认、结果弹窗等）全部改用 UiModal 外壳，内部业务内容不动。

### Toast（改造现有 `components/common/Toast.vue`）

- 右下角堆叠，高 52 的胶囊。
- 成功/普通：反白底（`--text-primary`），深字，左侧 22px 绿色圆形对勾，右侧可选一个深色动作按钮。4 秒后自动消失。
- 错误：底 `--color-error-surface`，`inset 0 0 0 1px #4a2522`，标题 + 说明两行，带动作按钮和关闭按钮，**不自动消失**，`role="alert"`。
- 用 `useNotification` 替换所有 `alert()`。

### UiProgress

- 线性：高 4，圆角 2，轨道 `--bg-active`，填充强调色（完成态用 `--color-success`）。
- 环形：36px，可用 `conic-gradient`，中心显示百分比（10px / 800）。

### EmptyState（改造现有）

- 面板底 `--bg-elevated`，圆角 24，居中；56px 圆形图标底 `--bg-control`；标题 16 / 800；说明 13 `--text-secondary`；**必须给一个主按钮作为下一步**。

### Icon

- 新建 `Icon.vue` + `icons.ts`（name → path 数据），全部为 24 视口、`stroke="currentColor"`、线宽 1.7 的描边图标。
- 用 `<Icon name="package" :size="18" />` 替换侧边栏的 `v-html` 图标和各处重复的内联 SVG。装饰性图标加 `aria-hidden="true"`。

## 5. 业务组件

### PackageCard（从 `ResourceDisplay.vue` 抽出）

- 宽度 `--resource-card-width`（255px）、整卡比例 `--resource-card-ratio`（45 / 32），目标高度约 181px；原卡片宽度 ×1.5、高度 ×0.8。圆角 `--radius-card`，`overflow: hidden`；图片区为扣除底部信息条后的固定空间，图片以 `object-fit: contain` 等比完整居中显示，不裁切、不拉伸，图片内在尺寸不得撑高预览区。有图时留白用 `--bg-base`；没有图时用 `--thumb-<type>`，中间放 34px 图片图标（`rgba(255,255,255,.16)`）。
- 底部信息条：高 `--resource-card-info-h`（56px），**实色** `--bg-elevated`（不要半透明），内边距 8px 12px：
  - 第 1 行：包名（不含作者前缀），13 / 700，单行省略；
  - 第 2 行：`作者 · v版本` 与右对齐的大小，11.5，`--text-tertiary`。
- 左上：类型标签，高 22，`--bg-on-image` 底，6px 类型色圆点 + 11px / 700 文字。
- 右上（互斥，优先级从高到低）：多选勾选框 → 缺依赖角标（`--color-warning` 底、`--color-warning-ink` 字、「缺 N」）→ 收藏星标（22px 圆，`--bg-on-image` 底，强调色实心星）。
- 状态：

| 状态 | 表现 |
|---|---|
| 默认 | 无边框、无阴影 |
| 悬停 | 仅提亮信息区背景，位置和尺寸保持不变；图片区显示启动场景（场景资源）和更多操作，收藏只在详情右上角切换 |
| 选中 | `box-shadow: var(--ring-selected)` |
| 多选模式 | 右上 22px 圆形勾选框；勾选后为强调色底 + 深色对勾，且同时显示选中外环 |
| 加载中 | 骨架：`--bg-elevated` 底，两条灰色圆角条，不显示转圈 |

- 整卡可点击（用覆盖全卡的 `<button>`，不要在 button 里嵌套 button）；右键打开 UiMenu。
- 网格：`grid-template-columns: repeat(auto-fill, minmax(0, min(100%, var(--resource-card-width)))); gap: var(--space-grid)`。包管理、仪表盘、预设、收藏场景、资源清理和 VaM Hub 共用尺寸；打开详情时网格宽度、列数和卡片位置保持不变。窄容器不足 255px 时缩小单列卡片，比例保持不变。

### 多选浮动操作栏

- 列表底部居中，固定定位，高 56，全圆角，反白底（`--text-primary`），`--shadow-lg`。
- 内容：「已选 N 个包」（14 / 800）+ 总大小（`--text-disabled`）→ 透明按钮：补全依赖、移动到…、收藏 → 红底白字按钮「移到回收站」（`#c8372b`）→ 40px 圆形取消按钮。

### PackageDetails（通用详情业务组件）

- 统一由 `UiInspector` 承载：右侧绝对定位的悬浮层，宽 `--resource-inspector-width`（440px，窄容器内不超过 100%），沿所在内容区上下贯通，`--bg-elevated`、`--radius-shell`、`--shadow-lg`。不占布局空间，不挤压或隐藏底下的卡片与列表；仪表盘、包管理、资源清理、预设、收藏场景和 Hub 共用此层。
- 内容独立滚动，关闭按钮固定可见。支持 Escape 关闭及返回打开它的控件；菜单或弹窗打开时优先由上层处理 Escape，忙碌时禁止关闭。背景内容仍可交互，不加全屏遮罩或键盘焦点锁。
- 包管理、仪表盘、资源清理与收藏场景都直接使用同一个 PackageDetails 组件；它统一管理图片浏览、元信息、操作、依赖、分享和角色提取。收藏页只传入资源元信息和选中的场景，不得自行组装详情或插入路径、改名、场景对象、原始 JSON 等专属内容。收藏场景的单击、双击及「更多」均打开同一右侧悬浮详情层。「添加收藏 / 移除收藏」统一为详情右上角的星形按钮，位于关闭按钮左侧，只对场景显示；卡片星标仅表示收藏状态，不提供直接切换收藏的按钮。包内收藏绑定当前选择的场景文件。
- 顶部 4:3 预览：左上类型标签；右上收藏 + 关闭（32px，`--bg-on-image`）；底部包内图片缩略图条（36px 圆角 9，当前图 2px 白环）+ 右侧「+N」。
- 正文（内边距 16px 18px 18px，间距 16）：
  1. 标题 20 / 800，副标题「作者 · v版本 · 大小 · 入库日期」13 `--text-tertiary`；
  2. 标签（Hub 标签）：高 26 胶囊，`#232327` 底；
  3. 操作：主按钮「启动场景」（lg，占满剩余宽度）+ 3 个 44px 图标按钮（打开位置、分享、更多）；「更多」里放 提取预设 / 复制 ID / 移到回收站；角色预设提取打开独立 UiModal，场景、角色、预设名和保存目录在弹窗内设置，底部为「取消 / 新增预设」，不在右侧详情内展开表单；
  4. 依赖列表：标题「依赖 N」+ 右侧缺失数（`--color-warning`）；每行高 44、圆角 12：32px 缩略图 + 名称（12.5 / 700）+ 作者（11）；已安装显示绿色「已安装」，缺失行底 `rgba(245,196,81,.08)` 并带黄色小按钮「补全」。

### PackageDetailModal（新：大图详情，双击卡片或按 Space 打开）

- UiModal，最大宽 1200，左右两栏（窄屏上下排）。
- 左栏：4:3 大图 + 左右切换（44px 圆按钮）+ 左下「3 / 12 · 文件名」+ 右下全屏按钮；下面 8 列缩略图（当前图为强调色外环）；底部提示「Esc 关闭 · ← → 切换图片」。
- 右栏：类型标签 +「最新版本」绿色标签 → 标题 28 / 800 → 作者·版本·大小 → 操作（启动场景、提取预设、分享、打开位置、删除）→ 分段 Tab：概览 / 依赖 N / 内容 N / 被依赖 N → 描述 → 3 列信息块（版本、文件、大小、入库、目录、许可；块底 `--bg-input`，圆角 12）→ 按类型的内容构成条（高 10，间隔 2px）+ 图例 → 缺依赖时显示黄色提示块 + 「一键补全」。

## 6. 页面

### 包管理（`PackagesView.vue` + `ResourceDisplay.vue`）

- PageHeader：标题「包管理」；副标题「N 个包 · 总大小 · ●N 分钟前已同步」；actions：搜索框（280 宽，Ctrl K）→ 排序按钮 → 视图分段（卡片 / 列表）→ 主按钮「扫描」。
- filters：类型筛选片（全部 + 8 种类型，带数量）；右侧「目录：全部 ▾」（替代现在常驻的文件夹树，点开为树形菜单）+「有问题 N」开关片。
- 主体：卡片网格，右侧 PackageDetailPanel 通过 `UiInspector` 悬浮展示；底部「已显示 x / N」+「加载更多」（保留现有分页逻辑也可以，但样式按本规范）。
- 列表视图：行高 52，40×30 缩略图，列：包名（名称 + 作者·版本两行）/ 类型 / 大小 / 依赖 / 状态 / 入库；选中行 `--accent-subtle` 底 + 左侧 3px 强调色内阴影。

### 仪表盘（`DashboardView.vue`）

- PageHeader：标题「仪表盘」，副标题为游戏目录路径；actions：快捷入库（secondary）、扫描（primary，扫描中为禁用 +「扫描中…」）。
- 扫描进度卡（扫描时出现）：36px 环形进度 +「正在扫描 X」+ 当前文件名 + 「642 / 1,284」+ 线性进度 + 取消按钮。
- 统计：4 个磁贴（`auto-fit, minmax(180px,1fr)`）：包总数 / 占用空间 / 场景 / 待处理问题；数字 30 / 800；问题数用 `--color-warning`；每块有一行副信息。
- 左：「最近入库」（与其他资源网格相同的 255 × 181px 宽卡，使用公共尺寸令牌，新入库带强调色「新」角标）+「快捷打开」（44 高胶囊，左侧 32px 圆形图标）。
- 右：「健康报告」面板：右上「N 个问题」黄色标签；三行（缺失依赖 → 一键补全[primary]、损坏的包 → 查看、重复资源 → 去重分析），每行 36px 圆形彩色图标 + 标题 14 / 800 + 说明；下面列出「缺失最多的依赖」。

### 其他页面

统计、在线资源、下载中心、预设、收藏夹、补全依赖、资源清理、去重分析、资源迁移、快捷入库、游戏设置、回收站、设置：

- 都换成 PageHeader + UiButton / UiChip / UiInput / UiModal；
- 卡片类内容复用 PackageCard；
- 下载/任务行：`--bg-elevated` 底，圆角 16，40px 缩略图 + 名称 + 状态（右对齐）+ 4px 进度条；
- 不重新设计信息架构，只替换组件和样式。

## 6.5 实现约定（v1.2，来自全页面排查）

1. **页面外壳只定义一次**：`.b2-page` / `.b2-page-body` 在 `base.css` 里全局定义（内边距 `--space-page-y` / `--space-page-x`，区块间距 `--space-section`）。页面组件里不要再复制这两个类，也不要在页面根元素上写 `max-width`、`margin`、`padding`、`gap`、`height`、`overflow`。表单类页面给 `.b2-page-body` 加 `is-form`，限制可读宽度（880px），左缘仍与页头对齐。
2. **scoped 样式里不要写元素选择器**：`button`、`select`、`input` 这类裸选择器会作用到子组件根元素上，覆盖 `UiButton` / `UiInput` 的样式。只给 class 写样式。
3. **要调整 `UiInput` 的宽度，选它的根元素 `.ui-input-field`**：内部的 `input` 在子组件里，父组件的 scoped 选择器选不到。
4. **卡片和面板用 `--bg-elevated`**：`--bg-surface` 是内容区底色，用它做卡片背景会和底色重合，看不出来。只有嵌在面板里的凹槽区域才用 `--bg-surface` / `--bg-input`。
5. **下拉框统一由 `base.css` 定义**：胶囊形、高 40，页面里只允许设置宽度。
6. **面板标题不靠 `text-transform` 改变文案**：标题写在 i18n 里，大小写以原文为准。

## 7. 约束

- **不改业务逻辑**：stores、router、`invoke(...)` 调用、i18n key 的含义一律不动；只动模板结构和样式。
- 新增文案全部走 i18n，`zh-CN.ts` 和 `en-US.ts` 都要加。
- 不引入 UI 组件库（Element / Naive / Tailwind 等），只用原生 CSS + 变量。
- 保留第 3 节列出的 class 和属性。
- 每个阶段结束都要能 `pnpm build`（含 `vue-tsc --noEmit`）和 `pnpm test` 通过。

## 8. 执行阶段（每阶段单独提交，完成后停下汇报）

1. **令牌与基础**：改 `variables.css`、`base.css`、`typography.css`，引入 Manrope，加 tabular-nums 和全局 `:focus-visible`。此阶段不改任何 `.vue`；汇报哪些页面因此明显变样。
2. **基础组件**：建 `src/components/ui/*` 和 `Icon`；改造 Toast、EmptyState；加 `useConfirm`。
3. **外壳**：AppLayout、AppSidebar（图标栏 + 工具菜单）、PageHeader。
4. **核心页面**：抽出 PackageCard / PackageDetailPanel / PackageDetailModal，重做包管理和仪表盘。
5. **其余页面**：逐页替换，每页去掉写死的颜色、字号和自定义按钮/弹窗 class。
6. **清理与验收**（见下）。

## 9. 验收

```bash
rg -n "#[0-9a-fA-F]{3,8}\b|rgba?\(" src --glob "*.vue"   # 目标 0；确需保留的逐条注明原因
rg -n "font-size:\s*[0-9]" src --glob "*.vue"            # 0，一律用 var(--text-*)
rg -n "border-radius:\s*[0-9]" src --glob "*.vue"        # 0，一律用 var(--radius-*)
rg -n "^\s*(padding|margin|gap)[a-z-]*:.*\b(3|5|6|7|9|10|14|18|22)px" src --glob "*.vue"  # 布局层应为 0
rg -n -U "<UiButton[^>]*>\s*<(div|h[1-6]|p)" src --glob "*.vue"   # 0，UiButton 不包多行内容
rg -n "\balert\(|\bconfirm\(" src                        # 0
rg -n "v-html=\"item.icon\"" src                         # 0
rg -c "<svg" src --glob "*.vue"                          # 大幅下降，剩余的只允许在 Icon.vue
```

- 键盘：Tab 能走遍侧边栏、头部、筛选片、卡片、详情操作，焦点环可见；菜单与弹窗支持 Esc。
- 所有纯图标按钮都有 `aria-label` 和 tooltip。
- 1280×800 窗口下，导航、头部、网格、详情面板都不溢出；头部控件过窄时换行。
