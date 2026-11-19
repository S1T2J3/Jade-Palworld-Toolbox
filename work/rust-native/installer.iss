[Setup]
AppId=JadePalworldRustBeta
AppName=Jade 幻兽帕鲁工具箱
AppVersion={#Version}
AppPublisher=Jade
DefaultDirName={userpf}\Jade Palworld Rust Beta
DefaultGroupName=Jade 幻兽帕鲁工具箱
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#DistDir}
OutputBaseFilename=Jade-Palworld-Rust-{#Version}-Setup
SetupIconFile=crates\jade-app\assets\atlas.ico
UninstallDisplayIcon={app}\Jade-Palworld-Rust.exe
Compression=lzma2/fast
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "zhCN"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; Flags: unchecked

[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{group}\Jade 幻兽帕鲁工具箱"; Filename: "{app}\Jade-Palworld-Rust.exe"; AppUserModelID: "JadePalworldRustBeta"
Name: "{userdesktop}\Jade 幻兽帕鲁工具箱"; Filename: "{app}\Jade-Palworld-Rust.exe"; AppUserModelID: "JadePalworldRustBeta"; Tasks: desktopicon

[Run]
Filename: "{app}\Jade-Palworld-Rust.exe"; Description: "打开 Jade 幻兽帕鲁工具箱"; Flags: nowait postinstall skipifsilent
