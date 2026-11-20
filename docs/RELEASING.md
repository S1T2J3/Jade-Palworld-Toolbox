# 公告与发行

仓库地址为 `S1T2J3/Jade-Palworld-Toolbox`，发布页、公告与安装包均可公开访问。Windows 首次正式发行标签为 `v1.0.0`。

## 本地版本归档

每次更新的交付文件都保存在原项目工作区的 `outputs/<完整版本号>/`。正式版和 Rust 版打包完成后会自动归档。单独制作的助手预览包也使用同一目录规则，例如 `outputs/2.0.0-beta.13+assistant.2/` 表示基于 beta.13 的第 2 个助手预览。用新的版本号保存有变化的构建，方便随时取回上一版。

手动打包的更新使用 `./scripts/archive-release.ps1 -Version $version -Files $assets` 归档，其中 `$assets` 是本版交付文件的路径列表。

### 帕鲁助手基础版和可选内核

准备好 Rust 版的 `data`、Rust 工具链和 LLVM-MinGW 后，在仓库根目录运行以下命令。依赖已缓存的机器可在第一条命令末尾加 `-Offline`。

```powershell
$built = & ./work/rust-native/build-assistant.ps1
& ./work/rust-native/package.ps1 -BaseExecutable $built.BaseExecutable -KernelExecutable $built.KernelExecutable -InnoCompiler 'C:\Path\To\ISCC.exe'
```

构建脚本先生成独立的助手内核，再用该文件的 SHA-256 编译基础版。打包时会核对基础版中嵌入的内核摘要，发现两个文件不配套会在生成发布文件前停止。每次更新内核都要重新构建配套的基础版并使用新的完整版本号。脚本运行结束时会恢复原先的构建环境。

基础安装包和便携包均可单独使用。自 Rust 2.0.1 起，玩家可在「设置 → 帕鲁助手」下载并启用内核，也可导入离线 ZIP。每个正式 Rust Release 必须以原名上传 `Jade-Pal-Assistant-Kernel-<完整版本号>-windows-x64.zip`，客户端按自身版本访问 `rust-v<完整版本号>` 标签下的这个附件。内核包根目录包含 `kernel.json` 和 `Jade-Pal-Assistant-Kernel.exe`。公开后要从客户端实际使用的地址取回内核并核验，确保版本和内核 SHA-256 与宿主配套。

## 修改公告

在默认分支的 [announcements.json](../announcements.json) 的 `announcements` 数组中添加记录。使用唯一的 `id`、简短的 `title`、纯文本 `body`，以及带 UTC 时区的 ISO 8601 时间 `publishedAt`。正文用 `\n` 换行。保留已发布公告的 ID，以便匹配随程序附带的公告。

要将一条公告设为弹窗，在这条记录中添加 `"popup": true`，提交到默认分支即可。省略该字段或填写 `false` 会在公告列表中正常展示。从 v1.0.4 起，Windows 用户收到公告后可自动看到弹窗，也可以在「公告与更新」关闭「弹窗提醒重要公告」。用户可点「检查更新」立即获取最新公告。

例如，可以将下面这条记录加入数组，发布时替换成自己的标题、正文、时间和唯一 ID。

```json
{
  "id": "notice-20261109-save-backup",
  "title": "世界档案备份提醒",
  "body": "更换电脑前，可以在「世界档案」中导出备份。",
  "publishedAt": "2026-11-09T12:00:00Z",
  "popup": true
}
```

同一份本机资料以 ID 记录已读和已弹状态。修正错字时沿用原 ID；需要再次提醒时，发布一条使用新 ID 的公告。

## 发布新版

本地打包和核验完成后先向用户说明本次内容，取得对该版本的明确发布确认，再推送标签、上传 GitHub 附件及发布公告。

每次更新的版本公告都设置 `"popup": true`。安装包构建时会为当前版本的公告自动补上此设置；客户端在弹窗成功显示后立即记为已阅，之后可在公告列表重读。

Rust 正式版使用 `rust-v主版本.次版本.修订号` 标签，上传同一构建的安装包、便携包、配套助手内核、源码、`rust-update.json` 和校验清单。发布时保留旧版的 GitHub Latest 标记，旧版更新程序仍用此入口。Rust 版公告维护在 `rust-announcements.json` 和 `work/rust-native/data/announcements.json`。2.0.0 是旧 Rust 测试版手动安装的升级起点。

### 旧版发行流程

1. 同步更新源码中的产品版本、桌面版本和内置公告。运行 README 中的测试，并使用打包产物验证首次安装、从旧资料升级和更新检查。
2. 下载上一正式版本的 `Jade-Palworld-Toolbox.exe`，对照该版发布页的 SHA-256，将它放在当前构建的 `dist/` 以外。从干净签出运行 `scripts/build-release.ps1 -DeltaBase 'C:\JadeBuild\previous\Jade-Palworld-Toolbox.exe'`。打包会生成新版主程序、快速启动器、HTML、增量文件与 `SHA256SUMS.txt`。可向 `-DeltaBase` 传入多个历史版本主程序，分别覆盖这些版本的用户。无基准的首次构建可省略参数。
3. 将本次源文件和公告提交到默认分支，用当前提交创建 `v主版本.次版本.修订号` 标签。
4. 在 GitHub Releases 创建对应草稿，上传 Windows 安装向导 `Jade-Palworld-Toolbox-Setup.exe`、两份便携版 EXE、HTML、校验清单及本次生成的所有 `.jdp` 文件。保留文件名，逐一核对 GitHub 附件的 SHA-256 后再正式发布，客户端才能同时获得差分和完整包。
5. 在未登录的环境中读取公告并从发布页下载安装包，对照 `SHA256SUMS.txt`。桌面更新还应从旧版本检查到新版本。

打包所需数据均为游戏资料或代码。开发机的世界存档、仓库备份、浏览器配置、授权凭据、用户目录和诊断记录应保留在个人设备上。

## 完整安装包的运行环境

v1.0.14 起，Setup 内附未经修改的官方 Firefox ESR 153.6.0 简体中文 x64 安装文件。构建前将 `JADE_RUNTIME_INSTALLER` 设为该文件的绝对路径（默认 `work/standalone/upstream/firefox-esr-x64.exe`）；构建脚本严格核对 90,514,512 字节和源码固定的 SHA-512，不接受其他版本。不得把此二进制提交到源码仓库。安装文件放到版本主程序旁，由工具箱校验并只解压运行环境，不安装 Firefox 浏览器。首次离线测试应使用全新数据目录及安装目录，并阻断所有外部网络。
