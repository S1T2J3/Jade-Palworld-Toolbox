# Jade · 幻兽帕鲁工具箱

一款中文《幻兽帕鲁》桌面工具。用自己仓库里的帕鲁规划配种，按目标词条反查亲代，在图鉴、制作配方、物品掉落和世界地图之间直接跳转。也可以读取本机游戏存档，将同一世界的终端帕鲁按个体导入仓库。

## 下载和运行

打开 [GitHub Releases](https://github.com/S1T2J3/Jade-Palworld-Toolbox/releases)。私有仓库需要先登录有访问权的 GitHub 账号。

| 文件 | 用法 |
| --- | --- |
| `Jade-Palworld-Toolbox.exe` | Windows x64 桌面主程序。包含本地存档识别、帕鲁终端导入和桌面更新入口。 |
| `Jade-Palworld-Toolbox-Fast.exe` | 可选的快速启动器。放在与同一版本主程序相同的文件夹，保留两个文件的原名。 |
| 独立 HTML | 用浏览器打开即可离线查询。仓库存在当前浏览器配置中。读取本机存档和安装桌面更新请使用桌面版。 |
| `SHA256SUMS.txt` | 对照该版本资产的 SHA-256。Windows 可运行 `Get-FileHash .\Jade-Palworld-Toolbox.exe -Algorithm SHA256`。 |
| `Jade-Delta-from-….jdp` | 工具箱自动选择的增量更新文件。首次安装请下载主程序。 |

桌面版会优先复用本机已有的同版 Gecko。新电脑首次运行时会从 Mozilla 下载约 90.5 MB 并校验 SHA-512。离线安装时，把[简体中文 x64 官方安装包](https://archive.mozilla.org/pub/firefox/releases/153.6.0esr/win64/zh-CN/Firefox%20Setup%20153.6.0esr.exe) 按原名 `Firefox Setup 153.6.0esr.exe` 放在主程序旁即可。引擎准备完毕后，配种、仓库、地图和图鉴均可离线使用。

在「设置 → 公告与更新」查看公告和更新状态。桌面版可使用已登录的 GitHub CLI，或为这个私有仓库绑定仅具 Contents 读取权限的细粒度令牌。令牌由当前 Windows 用户凭据加密保存。

从 v1.0.5 起，开启自动下载后，工具箱会在启动后检查新版。匹配到当前主程序的差分时只下载变化的部分；没有合适的差分时下载完整包。校验通过后可以立即安装并重启，也可以选「下次启动时安装」，继续使用当前窗口。

## 源码构建

在仓库根目录打开 PowerShell。网页构建使用 Node.js 22 或更新版本，依赖已随源码提供。

```powershell
New-Item -ItemType Directory -Force dist | Out-Null
node work/build.cjs dist/Jade-Palworld-Toolbox.html
node work/build.cjs --desktop build/content
node --test work/inventory-planner.test.cjs work/inventory-store.test.cjs work/trait-solver.test.cjs work/warehouse-solver.test.cjs
node work/standalone/tests/release-bridge.mjs
.\work\standalone\tests\run-release-tests.ps1
```

Windows x64 桌面构建还需要 Python 3、Go 1.26.5 或更新版本，以及 Windows 自带的 .NET Framework 4.x 编译器。保持仓库目录结构，在 PowerShell 中运行：

```powershell
.\scripts\build-release.ps1 -Repository 'S1T2J3/Jade-Palworld-Toolbox'
```

生成的发行文件和散列清单位于 `dist/`。维护公告与发布新版的步骤见[发布说明](docs/RELEASING.md)。图鉴、地图、图库和解码器的来源以及第三方许可见[第三方说明](THIRD_PARTY.md)。
