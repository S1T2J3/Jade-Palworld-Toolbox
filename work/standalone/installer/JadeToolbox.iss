; Build with scripts/build-release.ps1 or scripts/build-installer.ps1.
#define Product "Jade 幻兽帕鲁工具箱"
#define FullExe "Jade-Palworld-Toolbox.exe"
#define FastExe "Jade-Palworld-Toolbox-Fast.exe"
#define ProductId "JadePalworldToolbox"
#define BinDir "{app}\versions\" + AppVersion

[Setup]
AppId={#ProductId}
AppName={#Product}
AppVersion={#AppVersion}
AppPublisher=Jade
VersionInfoVersion={#AppVersion}.0
VersionInfoDescription={#Product} 安装程序
DefaultDirName={userpf}\Jade Palworld Toolbox
DefaultGroupName={#Product}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#DistPath}
OutputBaseFilename=Jade-Palworld-Toolbox-Setup
SetupIconFile={#ProductIcon}
UninstallDisplayIcon={#BinDir}\{#FullExe}
Compression=zip
SolidCompression=no
WizardStyle=modern
CloseApplications=no
RestartApplications=no

[Languages]
Name: "zhCN"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "快捷方式"; Flags: unchecked

[Files]
Source: "{#DistPath}\{#FullExe}"; DestDir: "{#BinDir}"; Flags: replacesameversion
Source: "{#DistPath}\{#FastExe}"; DestDir: "{#BinDir}"; Flags: replacesameversion
Source: "{#RuntimeInstaller}"; DestDir: "{#BinDir}"; DestName: "firefox-esr-x64.exe"; Flags: replacesameversion

[Icons]
Name: "{group}\{#Product}"; Filename: "{#BinDir}\{#FastExe}"; WorkingDir: "{#BinDir}"; AppUserModelID: "Jade.Palworld.Toolbox"
Name: "{userdesktop}\{#Product}"; Filename: "{#BinDir}\{#FastExe}"; WorkingDir: "{#BinDir}"; AppUserModelID: "Jade.Palworld.Toolbox"; Tasks: desktopicon

[Run]
Filename: "{#BinDir}\{#FastExe}"; Description: "打开{#Product}"; Flags: nowait postinstall skipifsilent

[Code]
function InitializeSetup(): Boolean;
var
  Prior: String;
  PriorVersion, NewVersion: Int64;
begin
  Result := True;
  if RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{#ProductId}_is1', 'DisplayVersion', Prior) then
    if StrToVersion(Prior, PriorVersion) and StrToVersion('{#AppVersion}', NewVersion) then
      if ComparePackedVersion(PriorVersion, NewVersion) > 0 then begin
        SuppressibleMsgBox('这台电脑已安装较新的 v' + Prior + '，请使用该版本或下载更新的安装包。', mbInformation, MB_OK, IDOK);
        Result := False;
      end;
end;
