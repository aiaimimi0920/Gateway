[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$BaseUrl,

    [string]$ManagementToken = $env:GATEWAY_MANAGEMENT_TOKEN,

    [ValidateSet("ManagementHeader", "Bearer")]
    [string]$AuthenticationMode = "ManagementHeader",

    [ValidateRange(1, 300)]
    [int]$TimeoutSec = 10,

    [switch]$AsJson
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-ResponseStatusCode {
    param(
        [Parameter(Mandatory = $true)]
        [System.Exception]$Exception
    )

    if ($null -eq $Exception.Response -or $null -eq $Exception.Response.StatusCode) {
        return $null
    }

    try {
        return [int]$Exception.Response.StatusCode
    }
    catch {
        return $null
    }
}

function Test-IsTransportTimeout {
    param(
        [Parameter(Mandatory = $true)]
        [System.Exception]$Exception
    )

    $current = $Exception
    while ($null -ne $current) {
        if ($current -is [System.TimeoutException]) {
            return $true
        }
        if ($current -is [System.Net.WebException] -and
            $current.Status -eq [System.Net.WebExceptionStatus]::Timeout) {
            return $true
        }
        if ($current.Message -match '(?i)timed?\s*out|timeout|operation was canceled|request was canceled') {
            return $true
        }
        $current = $current.InnerException
    }

    return $false
}

function Get-RequiredProperty {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Object,

        [Parameter(Mandatory = $true)]
        [string]$ObjectPath,

        [Parameter(Mandatory = $true)]
        [string]$PropertyName
    )

    if ($null -eq $Object) {
        throw "missing $ObjectPath"
    }
    $property = $Object.PSObject.Properties[$PropertyName]
    if ($null -eq $property) {
        throw "missing $ObjectPath.$PropertyName"
    }
    return $property.Value
}

function Assert-ValueType {
    param(
        [AllowNull()]
        [object]$Value,

        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [ValidateSet("object", "string", "boolean", "integer", "nullable-string")]
        [string]$ExpectedType
    )

    $valid = switch ($ExpectedType) {
        "object" {
            $null -ne $Value -and
                ($Value -is [System.Management.Automation.PSCustomObject] -or
                    $Value -is [System.Collections.IDictionary])
        }
        "string" { $Value -is [string] -and -not [string]::IsNullOrWhiteSpace($Value) }
        "nullable-string" { $null -eq $Value -or $Value -is [string] }
        "boolean" { $Value -is [bool] }
        "integer" {
            $Value -is [byte] -or $Value -is [sbyte] -or
                $Value -is [int16] -or $Value -is [uint16] -or
                $Value -is [int32] -or $Value -is [uint32] -or
                $Value -is [int64] -or $Value -is [uint64]
        }
    }

    if (-not $valid) {
        throw "$Path must be $ExpectedType"
    }
}

function Assert-RequiredField {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Object,

        [Parameter(Mandatory = $true)]
        [string]$ObjectPath,

        [Parameter(Mandatory = $true)]
        [string]$PropertyName,

        [Parameter(Mandatory = $true)]
        [ValidateSet("object", "string", "boolean", "integer", "nullable-string")]
        [string]$ExpectedType
    )

    $value = Get-RequiredProperty -Object $Object -ObjectPath $ObjectPath -PropertyName $PropertyName
    Assert-ValueType -Value $value -Path "$ObjectPath.$PropertyName" -ExpectedType $ExpectedType
    return $value
}

function Assert-OperatorSummarySchema {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Payload
    )

    $summary = Assert-RequiredField -Object $Payload -ObjectPath "response" -PropertyName "summary" -ExpectedType "object"
    $schemaVersion = Assert-RequiredField -Object $summary -ObjectPath "summary" -PropertyName "schemaVersion" -ExpectedType "integer"
    if ($schemaVersion -ne 1) {
        throw "summary.schemaVersion must equal 1"
    }

    [void](Assert-RequiredField $summary "summary" "generatedAt" "string")
    $build = Assert-RequiredField $summary "summary" "build" "object"
    [void](Assert-RequiredField $build "summary.build" "name" "string")
    [void](Assert-RequiredField $build "summary.build" "version" "string")
    $target = Assert-RequiredField $build "summary.build" "target" "object"
    [void](Assert-RequiredField $target "summary.build.target" "os" "string")
    [void](Assert-RequiredField $target "summary.build.target" "arch" "string")
    [void](Assert-RequiredField $build "summary.build" "debugAssertions" "boolean")

    $runtime = Assert-RequiredField $summary "summary" "runtime" "object"
    [void](Assert-RequiredField $runtime "summary.runtime" "role" "string")
    [void](Assert-RequiredField $runtime "summary.runtime" "processId" "integer")
    [void](Assert-RequiredField $runtime "summary.runtime" "port" "integer")

    $lifecycle = Assert-RequiredField $summary "summary" "lifecycle" "object"
    [void](Assert-RequiredField $lifecycle "summary.lifecycle" "state" "string")
    [void](Assert-RequiredField $lifecycle "summary.lifecycle" "draining" "boolean")
    [void](Assert-RequiredField $lifecycle "summary.lifecycle" "activeRequests" "integer")
    [void](Assert-RequiredField $lifecycle "summary.lifecycle" "drainStartedAt" "nullable-string")
    [void](Assert-RequiredField $lifecycle "summary.lifecycle" "drainReason" "nullable-string")
    [void](Assert-RequiredField $lifecycle "summary.lifecycle" "shutdownRequested" "boolean")
    [void](Assert-RequiredField $lifecycle "summary.lifecycle" "shutdownRequestedAt" "nullable-string")
    [void](Assert-RequiredField $lifecycle "summary.lifecycle" "shutdownReason" "nullable-string")

    $readiness = Assert-RequiredField $summary "summary" "readiness" "object"
    [void](Assert-RequiredField $readiness "summary.readiness" "ok" "boolean")
    $dependencies = Assert-RequiredField $readiness "summary.readiness" "dependencies" "object"
    foreach ($dependencyName in @("redis", "postgresql", "objectStorage")) {
        $dependency = Assert-RequiredField $dependencies "summary.readiness.dependencies" $dependencyName "object"
        [void](Assert-RequiredField $dependency "summary.readiness.dependencies.$dependencyName" "configured" "boolean")
        [void](Assert-RequiredField $dependency "summary.readiness.dependencies.$dependencyName" "required" "boolean")
        [void](Assert-RequiredField $dependency "summary.readiness.dependencies.$dependencyName" "ready" "boolean")
        [void](Assert-RequiredField $dependency "summary.readiness.dependencies.$dependencyName" "timedOut" "boolean")
    }
    [void](Assert-RequiredField $dependencies.objectStorage "summary.readiness.dependencies.objectStorage" "driver" "string")
    $configuration = Assert-RequiredField $readiness "summary.readiness" "configuration" "object"
    [void](Assert-RequiredField $configuration "summary.readiness.configuration" "apiKeySecret" "boolean")
    [void](Assert-RequiredField $configuration "summary.readiness.configuration" "publicBaseUrl" "boolean")

    $routing = Assert-RequiredField $summary "summary" "routing" "object"
    [void](Assert-RequiredField $routing "summary.routing" "configured" "boolean")
    foreach ($field in @("providerCount", "routeCount", "publishedModelCount")) {
        [void](Assert-RequiredField $routing "summary.routing" $field "integer")
    }
    $credentialCache = Assert-RequiredField $summary "summary" "credentialCache" "object"
    [void](Assert-RequiredField $credentialCache "summary.credentialCache" "entryCount" "integer")

    $requestMetrics = Assert-RequiredField $summary "summary" "requestMetrics" "object"
    foreach ($field in @(
            "requestsTotal", "requestErrorsTotal", "requestDrainRejectionsTotal",
            "requestInFlight", "requestDurationMsCount", "requestDurationMsSum",
            "rateLimitChecksTotal", "rateLimitRejectionsTotal", "rateLimitStoreFailuresTotal"
        )) {
        [void](Assert-RequiredField $requestMetrics "summary.requestMetrics" $field "integer")
    }

    $providerStats = Assert-RequiredField $summary "summary" "providerStats" "object"
    foreach ($field in @("activeProviders", "coolingProviders", "disabledProviders")) {
        [void](Assert-RequiredField $providerStats "summary.providerStats" $field "integer")
    }

    return $summary
}

if ([string]::IsNullOrWhiteSpace($ManagementToken)) {
    throw "A management token is required. Pass -ManagementToken or set GATEWAY_MANAGEMENT_TOKEN."
}

$endpoint = "{0}/v1/internal/gateway/operations/summary" -f $BaseUrl.Trim().TrimEnd("/")
$headers = @{}
if ($AuthenticationMode -eq "Bearer") {
    $headers["Authorization"] = "Bearer $ManagementToken"
}
else {
    $headers["x-management-token"] = $ManagementToken
}

try {
    $response = Invoke-WebRequest `
        -UseBasicParsing `
        -Method Get `
        -Uri $endpoint `
        -Headers $headers `
        -TimeoutSec $TimeoutSec `
        -ErrorAction Stop
}
catch {
    if (Test-IsTransportTimeout -Exception $_.Exception) {
        throw "Gateway operator summary transport-timeout after $TimeoutSec seconds."
    }
    $statusCode = Get-ResponseStatusCode -Exception $_.Exception
    if ($null -ne $statusCode) {
        throw "Gateway operator summary request failed with HTTP $statusCode."
    }
    throw "Gateway operator summary request failed before receiving an HTTP response."
}

$statusCode = [int]$response.StatusCode
if ($statusCode -lt 200 -or $statusCode -ge 300) {
    throw "Gateway operator summary request failed with HTTP $statusCode."
}

try {
    $payload = $response.Content | ConvertFrom-Json -ErrorAction Stop
}
catch {
    throw "Gateway operator summary response was not valid JSON."
}

try {
    $summary = Assert-OperatorSummarySchema -Payload $payload
}
catch {
    throw "Gateway operator summary schema validation failed: $($_.Exception.Message)"
}

if ($AsJson) {
    $payload | ConvertTo-Json -Depth 32
    return
}

[pscustomobject]@{
    GeneratedAt = $summary.generatedAt
    Version = $summary.build.version
    RuntimeRole = $summary.runtime.role
    ProcessId = $summary.runtime.processId
    Ready = $summary.readiness.ok
    Lifecycle = $summary.lifecycle.state
    ActiveRequests = $summary.lifecycle.activeRequests
    RedisReady = $summary.readiness.dependencies.redis.ready
    PostgreSqlConfigured = $summary.readiness.dependencies.postgresql.configured
    PostgreSqlReady = $summary.readiness.dependencies.postgresql.ready
    ObjectStorageReady = $summary.readiness.dependencies.objectStorage.ready
    RoutesConfigured = $summary.routing.configured
    ProviderCount = $summary.routing.providerCount
    RouteCount = $summary.routing.routeCount
    PublishedModelCount = $summary.routing.publishedModelCount
    CredentialCacheEntries = $summary.credentialCache.entryCount
    RequestsTotal = $summary.requestMetrics.requestsTotal
    RequestErrorsTotal = $summary.requestMetrics.requestErrorsTotal
    ProviderActive = $summary.providerStats.activeProviders
    ProviderCooling = $summary.providerStats.coolingProviders
    ProviderDisabled = $summary.providerStats.disabledProviders
}
