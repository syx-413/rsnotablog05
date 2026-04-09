$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$templatesDir = Join-Path $repoRoot "templates"
$publicDir = Join-Path $repoRoot "public"

function Copy-DirectoryContents {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SourceDir,
        [Parameter(Mandatory = $true)]
        [string]$DestinationDir
    )

    if (-not (Test-Path -LiteralPath $SourceDir)) {
        return
    }

    New-Item -ItemType Directory -Force -Path $DestinationDir | Out-Null
    Copy-Item -Path (Join-Path $SourceDir "*") -Destination $DestinationDir -Recurse -Force
}

New-Item -ItemType Directory -Force -Path $publicDir | Out-Null

$mainCss = Join-Path $templatesDir "main.css"
if (Test-Path -LiteralPath $mainCss) {
    Copy-Item -LiteralPath $mainCss -Destination (Join-Path $publicDir "main.css") -Force
}

Copy-DirectoryContents -SourceDir (Join-Path $templatesDir "css") -DestinationDir (Join-Path $publicDir "css")
Copy-DirectoryContents -SourceDir (Join-Path $templatesDir "fonts") -DestinationDir (Join-Path $publicDir "fonts")
Copy-DirectoryContents -SourceDir (Join-Path $templatesDir "assets") -DestinationDir (Join-Path $publicDir "assets")

Write-Host "Preview assets synced to public/"
Write-Host "If you changed Tera templates or Notion content, run cargo run instead."
