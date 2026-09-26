function Get-LineManifests {
    $lines = @()
    foreach ($path in Get-ChildItem -LiteralPath $ManifestRoot -Recurse -File -Filter "*.json" | Sort-Object FullName) {
        $manifest = Get-Content -LiteralPath $path.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
        $lines += [pscustomobject]@{
            Id = [string]$manifest.id
            Path = $path.FullName
            Manifest = $manifest
        }
    }
    return @($lines | Sort-Object Id)
}

function Read-LiveTargets {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "TargetsPath does not exist: $Path"
    }
    $parsed = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($parsed -is [array]) {
        return @($parsed)
    }
    $targets = Get-OptionalProperty -Object $parsed -Name "targets"
    if ($null -ne $targets) {
        return @($targets)
    }
    throw "TargetsPath must contain either a JSON array or an object with a targets array."
}

function Select-LineManifests {
    param(
        [array]$Lines,
        [array]$LiveTargets
    )

    if ($All -and -not [string]::IsNullOrWhiteSpace($LineId)) {
        throw "LineId cannot be combined with -All."
    }

    $requestedIds = @()
    if ($All) {
        $requestedIds = @($Lines | ForEach-Object { $_.Id })
    } elseif (-not [string]::IsNullOrWhiteSpace($LineId)) {
        $requestedIds = @(
            $LineId -split "[,;]" |
                ForEach-Object { $_.Trim() } |
                Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        )
    } elseif ($AllowLiveProviderCalls) {
        $requestedIds = @(
            $LiveTargets |
                ForEach-Object { [string](Get-OptionalProperty -Object $_ -Name "provider_line") } |
                Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
                Sort-Object -Unique
        )
    } else {
        $requestedIds = $RepresentativeLineIds
    }

    if ($requestedIds.Count -eq 0) {
        throw "No Gateway provider lines were selected. Use -LineId, -All, or provide live targets with provider_line labels."
    }

    $byId = @{}
    foreach ($line in $Lines) {
        $byId[$line.Id] = $line
    }
    $selected = @()
    foreach ($requestedId in $requestedIds | Select-Object -Unique) {
        if (-not $byId.ContainsKey($requestedId)) {
            throw "Gateway line not found: $requestedId"
        }
        $selected += $byId[$requestedId]
    }
    return @($selected | Sort-Object Id)
}

function Get-VerificationCategory {
    param([object]$Line)

    $manifest = $Line.Manifest
    $profile = [string]$manifest.identity.protocolProfile
    $families = @($manifest.capabilities.families | ForEach-Object { [string]$_ })
    $executionMode = [string]$manifest.identity.executionMode
    if ($profile -match "websocket" -or $Line.Id -match "websocket") {
        return "websocket"
    }
    if ($families -contains "search") {
        return "search"
    }
    if (
        $families -contains "music" -or
        $families -contains "video" -or
        $families -contains "videos"
    ) {
        return "media"
    }
    if ($executionMode -eq "browser_backed") {
        return "browser_backed"
    }
    if ($executionMode -eq "direct_http_replay") {
        return "direct_http_replay"
    }
    return "official_http"
}

function Get-EndpointFamily {
    param([object]$Line)

    $category = Get-VerificationCategory -Line $Line
    if ($category -in @("websocket", "search", "media")) {
        return $category
    }
    $families = @($Line.Manifest.capabilities.families | ForEach-Object { [string]$_ })
    if ($families.Count -gt 0) {
        return $families[0]
    }
    return "unknown"
}
