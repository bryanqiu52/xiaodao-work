# 打一个「纯净免安装版」的 zip：主程序 + md 查看器 + 便携标志 + 使用说明。
#
# **为什么要单独跑这个脚本，而不是直接把工作目录压出去**：
# 便携版的数据写在 exe 同目录的 `data\` 下 —— 直接压工作目录，等于把自己的
# 待办、账本、壁纸一起发给别人。所以这里每次**从零建一个空目录**，只放该放的。
# 打完还会自查一遍 zip 里有没有混进 `data\`。
#
# 用法：
#   .\scripts\make-portable.ps1              # 先编译再打包
#   .\scripts\make-portable.ps1 -SkipBuild   # 用现有产物，快

param(
  [switch]$SkipBuild,
  [string]$OutDir = ''
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'src-tauri\target\release\xiaodao_work.exe'
# md 查看器：作者本机的工具目录。换机器打包时改这个常量，或把查看器放到该路径
$viewer = 'C:\Users\123\.agents\tools\md-preview\MD-Preview.exe'
if ($OutDir -eq '') { $OutDir = Join-Path $root '发布' }

# 打包资源：tauri.conf.json 的 bundle.resources 指向 `src-tauri\tools\MD-Preview.exe`，
# 安装版才会带着它一起装。少了这步，装完点 md 产出会退回"打开所在文件夹" ——
# 兜底逻辑找的是"主程序旁边的查看器"，而装到 Program Files 后旁边并没有
$resDir = Join-Path $root 'src-tauri\tools'
New-Item -ItemType Directory -Path $resDir -Force | Out-Null
Copy-Item $viewer (Join-Path $resDir 'MD-Preview.exe') -Force

if (-not $SkipBuild) {
  Write-Host '[1/5] 编译（要几分钟）...' -ForegroundColor Cyan
  Push-Location $root
  npm run tauri:build
  $code = $LASTEXITCODE
  Pop-Location
  if ($code -ne 0) { throw '编译失败，没打包' }
} else {
  Write-Host '[1/5] 跳过编译（用现有产物）' -ForegroundColor DarkGray
}

if (-not (Test-Path $exe)) { throw "找不到主程序，先编译一次：$exe" }
if (-not (Test-Path $viewer)) { throw "找不到 md 查看器：$viewer" }

# 版本号从 tauri.conf.json 取，跟着版本走，省得两边手动同步
$conf = Get-Content (Join-Path $root 'src-tauri\tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$stamp = "$($conf.version)-$(Get-Date -Format 'yyyyMMdd')"

$stage = Join-Path $OutDir "小刀工作台-便携版-$stamp"
if (Test-Path $stage) {
  # 上一次打的目录里那个 exe 可能正被占着（进程没退干净、病毒扫描、资源管理器预览）。
  # 硬删会抛"拒绝访问"把整个打包打断 —— 那就换个名字继续，别卡在这件事上
  try {
    Remove-Item $stage -Recurse -Force -ErrorAction Stop
  } catch {
    $stage = "$stage-$(Get-Date -Format 'HHmmss')"
    Write-Host "  （上一份删不掉，改用新目录）" -ForegroundColor DarkGray
  }
}
New-Item -ItemType Directory -Path $stage -Force | Out-Null

Write-Host '[2/5] 放文件...' -ForegroundColor Cyan
Copy-Item $exe $stage
Copy-Item $viewer $stage

# 内置壁纸：随程序走的图库。便携模式下资源目录**就是 exe 所在目录**，
# 所以放 exe 旁边的 Wallpaper\ —— 不放的话设置里那些卡片全是空的
$wpOut = Join-Path $stage 'Wallpaper'
New-Item -ItemType Directory -Path $wpOut -Force | Out-Null
$wpSrc = Join-Path $root 'src-tauri\Wallpaper'
if (Test-Path $wpSrc) {
  Get-ChildItem $wpSrc -File |
    Where-Object { $_.Extension -in '.png', '.jpg', '.jpeg', '.webp', '.bmp', '.gif' } |
    Copy-Item -Destination $wpOut -Force
}
# **便携标志**：exe 同目录有这个空文件，数据才会写在旁边的 `data\` 里。
# 少了它，程序会去写 %APPDATA%，就不叫便携版了
New-Item -ItemType File -Path (Join-Path $stage 'portable') -Force | Out-Null

$readme = @"
小刀工作台 · 便携版 $($conf.version)
=====================================

免安装：解压到任意目录（U 盘也行），双击 xiaodao_work.exe 就能用。
数据（待办、记账、设置、备份）都存在本目录的 data\ 里，
整个文件夹换台电脑、换个盘符都还能接着用。

文件说明
--------
  xiaodao_work.exe   主程序
  MD-Preview.exe     看 md / txt 用的小工具 —— 在待办里点产出就靠它打开
  Wallpaper\         内置壁纸，设置 → 外观 → 背景 里直接挑
  portable           便携标志，有它数据才写在目录里（**别删**）
  data\              第一次运行后自动生成，你的数据都在这

第一次用
--------
  1. 双击 xiaodao_work.exe
  2. 弹出的向导第二页有一段提示词，点复制、发给你的 AI，
     它就会配合着装好技能 —— 之后用说话的方式记待办、记账都行
  3. 待办文件默认在 data\ 下；想放进自己的笔记库里，
     到「设置 → 待办」改路径

想卸载
------
  直接删掉整个文件夹。数据都在里面，删了就是全清。

打不开的话
----------
  提示缺 WebView2 → 装一下微软的 WebView2 Runtime（Win11 一般自带）
"@
Set-Content -Path (Join-Path $stage '使用说明.txt') -Value $readme -Encoding UTF8

Write-Host '[3/5] 压缩...' -ForegroundColor Cyan
$zip = "$stage.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $stage -DestinationPath $zip -CompressionLevel Optimal

# 自查：zip 里**绝不能**有 data\ —— 混进去就是把作者自己的账本发出去了。
# 这条比打包本身重要：错了不会报错，只会在别人机器上默默泄露数据
Add-Type -AssemblyName System.IO.Compression.FileSystem
$z = [System.IO.Compression.ZipFile]::OpenRead($zip)
try {
  $leak = $z.Entries | Where-Object { $_.FullName -match '/data/' }
  if ($leak) { throw "打包里混进了数据目录，别发出去：$($leak[0].FullName)" }
} finally {
  $z.Dispose()
}

# ── 安装版 ────────────────────────────────────────────────────────────────
# NSIS 安装包由 tauri build 生成在 bundle\nsis\ 下。
# 走了 -SkipBuild 且从没打过安装包时会找不到 —— 那只提示，不算失败：
# 便携版是必出的，安装版是"有了就一起给"
Write-Host '[4/5] 安装版...' -ForegroundColor Cyan
$nsisDir = Join-Path $root 'src-tauri\target\release\bundle\nsis'
$installer = Get-ChildItem $nsisDir -Filter '*.exe' -EA SilentlyContinue |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
$installerOut = ''
if ($installer) {
  $installerOut = Join-Path $OutDir "小刀工作台-安装版-$stamp.exe"
  Copy-Item $installer.FullName $installerOut -Force
} else {
  Write-Host '  （没找到 NSIS 安装包，跳过 —— 首次打包或用了 -SkipBuild 时会这样）' -ForegroundColor DarkGray
}

$mb = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host '[5/5] 完成' -ForegroundColor Green
Write-Host "  便携版目录：$stage"
Write-Host "  便携版压缩包：$zip  ($mb MB)"
if ($installerOut -ne '') {
  $imb = [math]::Round((Get-Item $installerOut).Length / 1MB, 1)
  Write-Host "  安装版：$installerOut  ($imb MB)"
}
Write-Host '  已自查：便携版包里没有 data\ 目录'
