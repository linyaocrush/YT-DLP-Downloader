<div align="center">

# YT-DLP Downloader

基于本地 **yt-dlp** 与 **ffmpeg** 的 Windows 视频下载工具

`WPF` · `.NET 10` · `MVVM` · `Win11 风格`

</div>

---

## 目录

- [简介](#简介)
- [功能特性](#功能特性)
- [环境要求](#环境要求)
- [快速开始](#快速开始)
- [使用说明](#使用说明)
  - [下载页](#下载页)
  - [设置页](#设置页)
  - [输出文件名模板](#输出文件名模板)
- [配置文件](#配置文件)
- [项目结构](#项目结构)
- [从源码构建](#从源码构建)
- [代码混淆](#代码混淆)
- [技术栈](#技术栈)
- [常见问题](#常见问题)
- [许可证](#许可证)
- [感谢项目名单](#感谢项目名单)

---

## 简介

**YT-DLP Downloader** 是一款轻量、简洁的 Windows 桌面下载工具。它本身不实现下载逻辑，而是调用你本机已安装的
`yt-dlp` 完成解析与下载、调用 `ffmpeg` 完成音视频合成，并提供一个现代化的图形界面。

程序采用 **MVVM** 架构，界面与逻辑彻底解耦，核心层（`Core`）不依赖任何 UI 框架，便于后续扩展与维护。

---

## 功能特性

| 功能 | 说明 |
| --- | --- |
| 链接解析 | 调用 `yt-dlp -J` 解析视频信息，展示标题、时长、上传者与封面 |
| 源列表 | 分别列出**视频源**（分辨率、帧率、码率、编码、容器）与**音频源**（码率、编码、容器） |
| 自动选择 | 自动挑选最高画质 + 最高音质，由 yt-dlp 合成为完整视频 |
| 手动选择 | 手动指定视频源与音频源，支持“音视频合一”单文件与“纯视频 + 纯音频”合成 |
| Cookie 文件 | 可选传入 Netscape 格式的 `cookies.txt`，用于会员 / 登录内容 |
| 多线程下载 | 通过 `--concurrent-fragments` 并发下载分片，可调 1–64 |
| 自定义文件名 | 模板字段拼装或固定文件名两种模式，扩展名自动补全 |
| 封面预览 | 封面仅在内存中加载，不落盘，关闭或重新解析即释放 |
| 设置持久化 | yt-dlp 路径、下载目录、Cookie、线程数、命名方式自动保存 |
| 现代化界面 | Win11 风格主题（配色、圆角、卡片、下划线标签页） |
| 发行混淆 | Release 构建自动调用 Obfuscar 混淆程序集 |

---

## 环境要求

| 项目 | 要求 |
| --- | --- |
| 操作系统 | Windows 10 / 11（x64） |
| 运行时 | 源码运行需 [.NET 10 SDK](https://dotnet.microsoft.com/)；单文件发行版无需安装 |
| 依赖程序 | **yt-dlp**（必需）、**ffmpeg**（合成音视频时必需） |

> `yt-dlp` 可在设置页指定，或放在系统 `PATH` 中自动识别；
> `ffmpeg` 请确保位于系统 `PATH` 中，否则无法完成音视频合成。

---

## 快速开始

### 1. 准备依赖

确保已安装 `yt-dlp` 与 `ffmpeg`，并将 `ffmpeg` 加入系统 `PATH`。

```powershell
yt-dlp --version
ffmpeg -version
```

### 2. 获取程序

**方式一：直接使用发行版**

下载并解压 `dist\win-x64\` 下的单文件程序，双击运行：

```text
YtDlpDownloader.exe
```

**方式二：从源码运行**

```powershell
dotnet run --project src\YtDlpDownloader.App\YtDlpDownloader.App.csproj
```

### 3. 首次使用

1. 打开 **设置** 页，确认 yt-dlp 路径（程序会自动在 `PATH` 中查找，未找到时手动选择）。
2. 回到 **下载** 页，粘贴视频链接，点击 **解析**。
3. 选择下载方式后点击 **开始下载**。

---

## 使用说明

### 下载页

| 区域 | 说明 |
| --- | --- |
| 视频链接 | 输入视频 URL，点击 **解析** 获取信息 |
| 保存到 | 选择下载目录，默认为系统“下载”文件夹 |
| 文件名 | 选择 **模板模式** 或 **文件名模式**（见下文） |
| Cookie 文件 | 可选，选择 Netscape 格式的 `cookies.txt` |
| 下载方式 | **自动选择** 或 **手动选择** |
| 视频源 / 音频源 | 解析后展示，手动模式下可逐项选择 |
| 进度 / 日志 | 实时进度条与 yt-dlp 原始日志 |

**自动选择**：使用表达式 `bv*+ba/b`，自动下载最高分辨率视频源与最高音质音频源并合成。

**手动选择**：

- 选择“音视频合一”的源 → 单文件直接下载，无需合成；
- 选择“纯视频”源 → 需再选择一个音频源，由 yt-dlp 合成。

### 设置页

| 项 | 说明 |
| --- | --- |
| yt-dlp 路径 | 优先在 `PATH` 中自动查找；未找到时手动浏览选择，支持“重新检测” |
| 下载目录 | 在下载页选择并自动记忆 |

> 设置文件与程序位于同一目录（`settings.json`），绿色免安装。

### 输出文件名模板

**模板模式**：点击字段按钮拼装文件名，再次点击可移除，扩展名 `.%(ext)s` 自动追加。

| 字段 | 含义 | 字段 | 含义 |
| --- | --- | --- | --- |
| `%(title)s` | 标题 | `%(extractor)s` | 站点 |
| `%(id)s` | 视频 ID | `%(format_id)s` | 格式 ID |
| `%(uploader)s` | 上传者 | `%(vcodec)s` | 视频编码 |
| `%(upload_date)s` | 上传日期 | `%(acodec)s` | 音频编码 |
| `%(resolution)s` | 分辨率 | `%(duration)s` | 时长 |
| `%(fps)s` | 帧率 | | |

**文件名模式**：直接输入文件名，无需填写扩展名，程序自动补全。

---

## 配置文件

程序会在**自身目录**生成 `settings.json`，所有设置自动持久化：

```json
{
  "YtDlpPath": "D:\\Tools\\YT-DLP\\yt-dlp.exe",
  "DownloadDirectory": "D:\\Videos",
  "CookieFile": "",
  "DownloadThreads": 4,
  "OutputNameMode": "Template",
  "OutputTemplate": "%(title)s",
  "OutputFileName": ""
}
```

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `YtDlpPath` | string | `""` | yt-dlp 可执行文件路径 |
| `DownloadDirectory` | string | `""` | 下载目录，留空则使用系统“下载”文件夹 |
| `CookieFile` | string | `""` | Netscape 格式 Cookie 文件路径 |
| `DownloadThreads` | int | `4` | 并发分片数（1–64） |
| `OutputNameMode` | enum | `Template` | 命名方式：`Template` / `Fixed` |
| `OutputTemplate` | string | `"%(title)s"` | 模板模式选中的字段（空格分隔，不含扩展名） |
| `OutputFileName` | string | `""` | 文件名模式下的固定文件名（不含扩展名） |

---

## 项目结构

```text
YtDlpDownloader/
├─ YtDlpDownloader.slnx              # 解决方案
├─ README.md
└─ src/
   ├─ YtDlpDownloader.Core/          # 核心层（net10.0，无 UI 依赖）
   │  ├─ Mvvm/                       # ObservableObject / RelayCommand / AsyncRelayCommand
   │  ├─ Models/                     # AppSettings、MediaInfo、VideoSource、AudioSource
   │  └─ Services/                   # 进程、解析、CLI、设置、路径查找
   │     ├─ ProcessRunner.cs         # 统一的进程调用（异步、可取消、UTF-8）
   │     ├─ MediaInfoParser.cs       # yt-dlp JSON → 领域模型
   │     ├─ YtDlpCli.cs              # yt-dlp 命令行封装
   │     ├─ SettingsService.cs       # settings.json 读写
   │     └─ YtDlpPathService.cs      # PATH 查找 + 设置回退
   └─ YtDlpDownloader.App/           # WPF 界面层（net10.0-windows）
      ├─ App.xaml(.cs)               # 资源字典与组合根（手动 DI）
      ├─ ViewModels/                 # Main / Download / Settings
      ├─ Views/                      # MainWindow / DownloadView / SettingsView
      ├─ Converters/                 # 值转换器
      ├─ Styles/Win11Theme.xaml      # Win11 主题
      └─ obfuscar.xml                # 混淆规则
```

**分层约定**

- `Core` 不引用任何 WPF 类型，可被测试或复用到其他宿主（控制台、CLI 等）。
- `App` 仅负责界面与交互，通过接口（`IYtDlpCli`、`ISettingsService` 等）使用核心层。
- 组合根集中在 `App.xaml.cs`，不引入第三方 DI 容器。

---

## 从源码构建

```powershell
# 还原并构建
dotnet build YtDlpDownloader.slnx -c Release

# 运行
dotnet run --project src\YtDlpDownloader.App\YtDlpDownloader.App.csproj
```

### 发布单文件版本

```powershell
dotnet publish src\YtDlpDownloader.App\YtDlpDownloader.App.csproj `
  -c Release -r win-x64 --self-contained true `
  -p:PublishSingleFile=true `
  -p:IncludeNativeLibrariesForSelfExtract=true `
  -p:EnableCompressionInSingleFile=true `
  -o dist\win-x64
```

产物位于 `dist\win-x64\YtDlpDownloader.exe`，为自包含、压缩的绿色单文件。

---

## 代码混淆

Release 构建/发布会自动调用 [Obfuscar](https://github.com/obfuscar/obfuscar) 混淆输出程序集，规则见
`src\YtDlpDownloader.App\obfuscar.xml`。

为保证 WPF 正常运行，以下内容被有意保留：

- **属性名与事件名**：WPF `Binding Path` 依赖属性名，必须保留。
- **`Views` / `Converters` 命名空间**：BAML 按类型名实例化，不能重命名。
- **`OutputNameMode` 枚举成员**：设置文件以字符串形式持久化。

```powershell
# 临时关闭混淆
dotnet publish ... -p:ObfuscateRelease=false
```

> 混淆在 `bin` 输出目录进行，不影响调试构建（`Debug`）。

---

## 技术栈

| 类别 | 选型 |
| --- | --- |
| 语言 / 框架 | C# / .NET 10 |
| 界面 | WPF（`net10.0-windows`），Win11 风格主题 |
| 架构 | MVVM（自实现 `ObservableObject` / `RelayCommand`） |
| 依赖注入 | 组合根手动装配，无第三方容器 |
| 进程调用 | `System.Diagnostics.Process`（异步读取、可取消） |
| 序列化 | `System.Text.Json` |
| 混淆 | Obfuscar 2.2.50 |

---

## 常见问题

**Q：解析失败或提示找不到 yt-dlp？**
A：到 **设置** 页确认 yt-dlp 路径；若未自动识别，请手动选择 `yt-dlp.exe`。

**Q：下载完成但没有画面/声音，或无法合成？**
A：请确认 `ffmpeg` 已安装并加入系统 `PATH`。

**Q：某些视频无法下载？**
A：可能需要登录。请导出 Netscape 格式的 `cookies.txt`，在下载页选择该 Cookie 文件。

**Q：重新构建时提示文件被占用？**
A：程序正在运行会锁定 `exe`，请先关闭程序再构建。

**Q：中文标题出现乱码？**
A：程序已通过 `--encoding utf-8` 统一输出编码，正常情况不会出现乱码。

---

## 许可证

本项目采用 [YT-DLP Downloader 非商业相同方式共享许可证 1.0](LICENSE)。

简单来说，你**可以自由使用、修改和分发**本项目，但必须满足以下条件：

| 条件 | 说明 |
| --- | --- |
| **署名（必须提及源项目）** | 保留版权声明，并在显著位置注明来源于 **YT-DLP Downloader** 项目 |
| **非商业** | 不得用于任何商业目的（销售、收费服务、广告变现等） |
| **相同方式共享（开源）** | 修改后的版本必须开源，并采用与本项目相同的许可证分发 |

完整条款以 [LICENSE](LICENSE) 文件为准。

---

## 感谢项目名单

本项目能够运行，离不开以下优秀的开源项目：

| 项目 | 说明 | 项目地址 |
| --- | --- | --- |
| **yt-dlp** | 视频解析与下载核心 | <https://github.com/yt-dlp/yt-dlp> |
| **FFmpeg** | 音视频合成 | <https://ffmpeg.org/> |
| **Obfuscar** | 发行时代码混淆 | <https://github.com/obfuscar/obfuscar> |
| **.NET / WPF** | 运行时与界面框架 | <https://dotnet.microsoft.com/> |

---

<div align="center">

Copyright © 2026 凛ふわ狐

</div>
