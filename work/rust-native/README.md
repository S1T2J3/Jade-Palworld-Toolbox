# Jade 幻兽帕鲁工具箱 · Rust 原生测试版

Windows x64 原生桌面应用。界面、配种与计算器、存档读取、地图、备份和更新均由 Rust 实现；运行时不启动 Gecko、Chromium、JavaScript、Go 或 C# 进程。界面使用 eframe/egui + OpenGL。

## 使用

安装 `Jade-Palworld-Rust-2.0.0-beta.4-Setup.exe`，或解压同版本 Windows ZIP 后运行 `Jade-Palworld-Rust.exe`。保留相邻的 `data` 目录。安装与离线资料查询不需要连接 GitHub；检查公告或下载更新时需要网络。

beta.2 按 v1.0.17 恢复窄侧栏、图标与辉月伊配色，配种/仓库的密集卡片和分页，以及图鉴详情、原版筛选和关联跳转。地图恢复侧栏筛选、浮动点位详情和档案弹窗；计算器分别记忆输入。仍使用 Rust 原生界面，保留后台预加载和长列表虚拟化。

测试版独立安装，数据位于 `%LOCALAPPDATA%\JadePalworldRustBeta`。正式版保持不变。如需迁移工具箱记录：在正式版的世界档案导出 JSON，在测试版的世界档案导入并确认预览。也可直接扫描游戏存档；原始游戏存档只读。不要把真实存档提交到公开仓库。

界面支持 Ctrl+K 搜索、Alt+左/右返回与前进、Ctrl+N 新窗口。地图支持拖动、滚轮、方向键和缩放按钮。默认再次启动激活已有窗口。

自动更新只接收 `rust-v...` 测试发行版。打开自动下载后，优先使用适用的文件增量包；首次发行或没有适用增量时下载完整包。下载验证后可立即安装重启，或在下次启动安装。失败会保留或恢复上一版。首次测试版与正式版程序格式不同，不能相互增量替换。

beta.3 稳定地图平移时的点位聚类与遮挡顺序，补齐图鉴、搜索与工作效率的图片，并修复仓库多词条卡片布局。

beta.4 逐项恢复顶部页签与窗口矢量按钮、公告双卡和弹窗、牧场图片与反查、工作适应性分级头像、词条等级图颜色及负阶级方向、手术台和可配子代图片。

## 配置范围

Windows 10/11 x64，显卡驱动需支持 OpenGL 2.0 或更高。没有独立浏览器或额外下载运行时的要求。图片按需解码，资料与默认地图在首页可用后分批准备；纹理大小服从设备上限。CPU 核心限制测试仅是模拟，不能代替真实旧电脑测试。Windows 基本显示驱动仅提供 OpenGL 1.1 时可能无法打开，需安装显卡厂商驱动。

## 从源码构建

推荐 Rust 1.99.1，`x86_64-pc-windows-gnu` 目标与 LLVM-MinGW UCRT；让 `llvm-windres` 可从 PATH 找到。中文路径下使用 LLVM 的链接器：

```powershell
$env:RUSTFLAGS='-C linker=rust-lld.exe -C linker-flavor=ld.lld'
node work/build.cjs --desktop build/rust-data-source
node work/rust-native/export-data.cjs build/rust-data-source
cargo build --release --locked --manifest-path work/rust-native/Cargo.toml
```

Node 仅用于开发时把现有经过关联处理的资料转换为 JSON；最终应用不需要 Node。将编译产物与生成的 `data` 放在同一目录即可运行。使用 Inno Setup 6 可构建独立安装包：

```powershell
pwsh -File work/rust-native/package.ps1 -InnoCompiler 'C:\Path\To\ISCC.exe'
```

`Cargo.lock` 锁定依赖版本。`cargo test --workspace --locked --manifest-path work/rust-native/Cargo.toml` 运行核心与 UI 状态测试；需要 Windows 常规桌面权限的本地互斥锁/文件替换测试不应在禁止这些 API 的隔离环境运行。存档公共样本测试另见 `crates/jade-saves/README.md`；测试不能依赖用户真实世界。

软件采用 GPL-3.0-only；`jade-core` 为 MIT。第三方组件与数据出处见 `THIRD_PARTY.md`、`licenses`、原仓库的数据说明及 `crates/jade-saves/NOTICE.md`。本项目为非官方玩家工具，游戏美术和文本权利属于原作者。

QQ 交流群：1126734443。
