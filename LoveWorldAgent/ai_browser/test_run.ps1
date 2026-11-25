# 启动 CEF 独立测试窗口：不带 --parent / --pipe，仅验证渲染与导航栏，不参与智能体控制。
# 用法：powershell -ExecutionPolicy Bypass -File ai_browser\test_run.ps1 -Url https://example.com
param(
    [string]$Url = "https://www.bing.com",
    [string]$Exe = ""
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $Exe) {
    $Exe = Join-Path $root "out\Release\browser_panel.exe"
}
if (-not (Test-Path $Exe)) {
    throw "未找到 $Exe，请先执行 build.ps1"
}
$proc = Start-Process -FilePath $Exe -ArgumentList @("--url=$Url") -WorkingDirectory (Split-Path -Parent $Exe) -PassThru
Write-Host ("测试窗口已启动 pid=" + $proc.Id + " url=" + $Url)
Write-Host "结束：Stop-Process -Name browser_panel"

