# Jade · 幻兽帕鲁工具箱

一款中文《幻兽帕鲁》桌面工具。用自己仓库里的帕鲁规划配种，按目标词条反查亲代，在图鉴、制作配方、物品掉落和世界地图之间直接跳转。也可以读取本机游戏存档，将同一世界的终端帕鲁按个体导入仓库。

## 下载和运行

工具箱已公开，可以直接从 [GitHub Releases](https://github.com/S1T2J3/Jade-Palworld-Toolbox/releases/latest) 下载安装包。

| 文件 | 用法 |
| --- | --- |
| `Jade-Palworld-Toolbox-Setup.exe` | 推荐首次安装。运行一个 Windows x64 安装向导即可添加工具箱和开始菜单入口，也可以创建桌面快捷方式。 |
| `Jade-Palworld-Toolbox.exe` | Windows x64 桌面主程序。包含本地存档识别、帕鲁终端导入和桌面更新入口。 |
| `Jade-Palworld-Toolbox-Fast.exe` | 可选的快速启动器。放在与同一版本主程序相同的文件夹，保留两个文件的原名。 |
| 独立 HTML | 用浏览器打开即可离线查询。仓库存在当前浏览器配置中。读取本机存档和安装桌面更新请使用桌面版。 |
| `SHA256SUMS.txt` | 对照该版本资产的 SHA-256。Windows 可运行 `Get-FileHash .\Jade-Palworld-Toolbox.exe -Algorithm SHA256`。 |
| `Jade-Delta-from-….jdp` | 工具箱自动选择的增量更新文件。首次安装请下载主程序。 |

安装版与便携版沿用当前 Windows 用户原有的世界档案、帕鲁仓库和设置。已有同版 Gecko 的电脑可以复用运行环境。从 v1.0.14 起，推荐的 Setup 安装包自带未经修改的官方运行环境安装文件，安装完成后首次启动无需连接 Mozilla。便携版首次运行时会依次尝试官方 CDN 和归档站下载约 90.5 MB，每个来源均校验 SHA-512，失败自动切换。下载安装包与在线更新仍需要访问 GitHub；也可通过 QQ 群转发完整 Setup 给无法联网的电脑。离线准备运行环境时，把[简体中文 x64 官方安装包](https://archive.mozilla.org/pub/firefox/releases/153.6.0esr/win64/zh-CN/Firefox%20Setup%20153.6.0esr.exe) 按原名 `Firefox Setup 153.6.0esr.exe` 放在安装后的主程序旁。引擎准备完毕后，配种、仓库、地图和图鉴均可离线使用。

在「设置 → 公告与更新」查看公告和更新状态。v1.0.7 起，官方桌面版会直接读取公开公告和更新。若旧版提示需要 GitHub 仓库权限，可从发布页下载新版安装包，沿用已有世界档案、仓库和设置。

从 v1.0.5 起，开启自动下载后，工具箱会在启动后检查新版。匹配到当前主程序的差分时只下载变化的部分；没有合适的差分时下载完整包。校验通过后可以立即安装并重启，也可以选「下次启动时安装」，继续使用当前窗口。

左侧栏的「全局搜索」（`Ctrl+K`）可按中文名、英文名或游戏编号查询已有资料，包括帕鲁、主动技能、被动词条、伙伴技能、物品、设施、单位、远征和手记等。结果可按类型筛选并直接跳转详情；首页优先显示，随后在空闲时分批准备地图和其他图鉴、搜索资料，支持离线使用。操作时优先响应当前页面，隐藏窗口暂停后台准备。

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

Windows x64 桌面构建使用 Python 3、Go 1.26.5 或更新版本，以及 Windows 自带的 .NET Framework 4.x 编译器。生成安装向导还需 Inno Setup 7，可通过 `JADE_ISCC` 指定 `ISCC.exe` 的完整路径。在仓库根目录执行以下命令。

```powershell
.\scripts\build-release.ps1 -Repository 'S1T2J3/Jade-Palworld-Toolbox'
```

生成的发行文件和散列清单位于 `dist/`。维护公告与发布新版的步骤见[发布说明](docs/RELEASING.md)。图鉴、地图、图库和解码器的来源以及第三方许可见[第三方说明](THIRD_PARTY.md)。
