export const depCompletionZh = {
  // 页头
  recheck: '重新检查',
  checking: '检查中…',
  findDownloads: '查找下载',
  lastChecked: '上次检查：{time}',

  // 统计卡片
  stats: {
    missing: '缺少的依赖',
    missingHint: '所有资源缺少的依赖合计',
    packages: '需要补的资源',
    packagesHint: '同一个资源只算一次',
    affected: '受影响的资源',
    affectedHint: '缺少依赖，可能无法正常使用',
    outdated: '版本太旧',
    outdatedHint: '已安装，但版本低于需要的',
  },

  // 列表
  listTitle: '需要补的资源',
  listDesc: '每一行是一个缺少的资源，下方列出用到它的资源。',
  latestVersion: '最新版',
  none: '没有',
  meta: {
    required: '需要的版本：{version}',
    installed: '已安装：v{version}',
    available: '可下载版本：{version}',
    usedBy: '{count} 个资源用到',
    references: '共引用 {count} 次',
  },
  more: '还有 {count} 个',
  copyName: '复制名称',

  // 状态
  status: {
    missing: '未安装',
    outdated: '版本太旧',
    notFound: '找不到下载',
    failed: '查询失败',
    required: '需要的版本',
    newer: '更新的版本',
    latest: '最新版',
    noMatch: '没有合适的版本',
  },
  reason: {
    notFound: 'VaM Hub 上没有这个资源的下载',
    noMatch: 'VaM Hub 上只有比需要的更旧的版本',
    failed: '没有连上 VaM Hub，可以稍后重新查找。',
  },

  // 空状态
  emptyTitle: '没有缺少的依赖',
  emptyReady: '所有资源需要的依赖都已安装。',
  emptyNoRoot: '先选择游戏目录并扫描，才能检查缺少的依赖。',
  chooseDirectory: '选择游戏目录',

  // 补全对话框
  modal: {
    title: '补全缺少的依赖',
    description: '已在 VaM Hub 上查找这些资源。找不到下载的会留在列表中，但不会下载。',
    modeLabel: '下载哪个版本',
    modeRequired: '按需要的版本',
    modeLatest: '下载最新版',
    modeHint: '找不到下载的资源会被跳过。',
    available: '可以下载 {count} 个',
    notFound: '找不到下载 {count} 个',
    mismatch: '没有合适版本 {count} 个',
    failed: '查询失败 {count} 个',
    total: '共 {count} 个',
    resolving: '正在 VaM Hub 上查找… {current} / {total}',
    summary: '将下载 {count} 个',
    summarySkipped: '将下载 {count} 个，跳过 {skipped} 个',
    downloadRequired: '下载需要的版本',
    downloadLatest: '下载最新版',
  },

  // 提示
  toast: {
    needDirectory: '请先在设置里选择游戏目录',
    refreshed: '已重新检查缺少的依赖',
    refreshFailed: '检查失败：{error}',
    nothingToComplete: '没有需要补全的依赖',
    nothingToDownload: '没有可以下载的依赖',
    added: '已添加 {count} 个下载',
    skippedExisting: '{count} 个已经下载过，已跳过',
    addFailed: '添加下载失败：{error}',
    copied: '已复制 {name}',
    copyFailed: '复制失败：{error}',
  },
}

export const depCompletionEn: typeof depCompletionZh = {
  recheck: 'Check again',
  checking: 'Checking…',
  findDownloads: 'Find downloads',
  lastChecked: 'Last checked {time}',

  stats: {
    missing: 'Missing dependencies',
    missingHint: 'Total across all resources',
    packages: 'Packages to get',
    packagesHint: 'Each package counted once',
    affected: 'Affected resources',
    affectedHint: 'May not work until fixed',
    outdated: 'Outdated',
    outdatedHint: 'Installed, but older than required',
  },

  listTitle: 'Packages to get',
  listDesc: 'Each row is a missing package, followed by the resources that use it.',
  latestVersion: 'Latest',
  none: 'None',
  meta: {
    required: 'Required: {version}',
    installed: 'Installed: v{version}',
    available: 'Available: {version}',
    usedBy: 'Used by {count} resources',
    references: 'Referenced {count} times',
  },
  more: '+{count} more',
  copyName: 'Copy name',

  status: {
    missing: 'Not installed',
    outdated: 'Outdated',
    notFound: 'Not found',
    failed: 'Lookup failed',
    required: 'Required version',
    newer: 'Newer version',
    latest: 'Latest',
    noMatch: 'No suitable version',
  },
  reason: {
    notFound: 'Not available on VaM Hub',
    noMatch: 'VaM Hub only has versions older than required',
    failed: "Couldn't reach VaM Hub. Try again later.",
  },

  emptyTitle: 'No missing dependencies',
  emptyReady: 'Everything your resources need is installed.',
  emptyNoRoot: 'Choose your game folder and scan it to check for missing dependencies.',
  chooseDirectory: 'Choose game folder',

  modal: {
    title: 'Get missing dependencies',
    description: "These packages were looked up on VaM Hub. Any without a download stay in the list but won't be downloaded.",
    modeLabel: 'Version to download',
    modeRequired: 'Required version',
    modeLatest: 'Latest version',
    modeHint: 'Packages without a download are skipped.',
    available: '{count} available',
    notFound: '{count} not found',
    mismatch: '{count} without a suitable version',
    failed: "{count} couldn't be checked",
    total: '{count} total',
    resolving: 'Looking on VaM Hub… {current} / {total}',
    summary: '{count} to download',
    summarySkipped: '{count} to download, {skipped} skipped',
    downloadRequired: 'Download required versions',
    downloadLatest: 'Download latest versions',
  },

  toast: {
    needDirectory: 'Choose your game folder in Settings first',
    refreshed: 'Missing dependencies checked',
    refreshFailed: "Couldn't check dependencies: {error}",
    nothingToComplete: 'No missing dependencies to get',
    nothingToDownload: 'Nothing to download',
    added: 'Added {count} downloads',
    skippedExisting: 'Skipped {count} already downloaded',
    addFailed: "Couldn't add downloads: {error}",
    copied: 'Copied {name}',
    copyFailed: "Couldn't copy: {error}",
  },
}
