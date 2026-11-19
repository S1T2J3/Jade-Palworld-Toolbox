# Jade 幻兽帕鲁工具箱 Rust 2.0.0 源码

此压缩包包含对应版本 Windows 主程序和可选帕鲁助手内核的源码、构建脚本、锁定依赖清单、发行数据与许可。`work/rust-native/data` 是本版随包使用的游戏数据和索引。`work/rust-native/source-dependencies` 附有 `oozextract` 0.5.7 的源代码，`work/rust-native/crates/jade-saves/vendor` 附有存档解析器的源码。

## 构建 Windows x64

在 Windows 安装 PowerShell 7、Rust 1.99.1 的 `x86_64-pc-windows-gnu` 工具链和 LLVM-MinGW UCRT。以 `x86_64-pc-windows-gnu` 为当前 Rust 工具链的默认目标，将 `cargo`、`rust-lld.exe` 和 `llvm-windres` 放入 `PATH`。在解压后的目录运行：

```powershell
$pair = & ./work/rust-native/build-assistant.ps1
& ./work/rust-native/package.ps1 -BaseExecutable $pair.BaseExecutable -KernelExecutable $pair.KernelExecutable
```

Cargo 会根据 `Cargo.lock` 获取依赖。依赖已缓存时可向 `build-assistant.ps1` 传入 `-Offline`。构建脚本先编译助手内核，再把对应校验值嵌入主程序。打包时向 `package.ps1` 增加 `-InnoCompiler 'C:\Path\To\ISCC.exe'` 可生成安装包。便携版和可选助手组件使用同一完整版本号，打包输出同时归档到 `outputs/2.0.0/`。

许可证和第三方出处见 `work/rust-native/LICENSE`、`work/rust-native/THIRD_PARTY.md` 及 `work/rust-native/licenses`。
