function Get-Utf8Sha256Hex {
  param([Parameter(Mandatory = $true)][string] $Text)

  $sha256 = [System.Security.Cryptography.SHA256]::Create()
  try {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Text)
    return [System.BitConverter]::ToString($sha256.ComputeHash($bytes)).Replace("-", "").ToLowerInvariant()
  } finally {
    $sha256.Dispose()
  }
}
function ConvertTo-JsonLiteral {
  param($Value)

  if ($null -eq $Value) {
    return "null"
  }
  return $Value | ConvertTo-Json -Depth 20 -Compress
}

function New-SeedRevisionMetadataJson {
  param([Parameter(Mandatory = $true)] $RevisionMetadata)

  $id = ConvertTo-JsonLiteral ([string]$RevisionMetadata.id)
  $sequence = ConvertTo-JsonLiteral ([int64]$RevisionMetadata.sequence)
  $parent = ConvertTo-JsonLiteral $RevisionMetadata.parent
  $actor = ConvertTo-JsonLiteral ([string]$RevisionMetadata.actor)
  $timestamp = ConvertTo-JsonLiteral ([string]$RevisionMetadata.timestamp)
  $documentDigest = ConvertTo-JsonLiteral ([string]$RevisionMetadata.documentDigest)
  $yamlDigest = ConvertTo-JsonLiteral ([string]$RevisionMetadata.yamlDigest)
  $message = ConvertTo-JsonLiteral $RevisionMetadata.message
  return "{`"id`":$id,`"sequence`":$sequence,`"parent`":$parent,`"actor`":$actor,`"timestamp`":$timestamp,`"documentDigest`":$documentDigest,`"yamlDigest`":$yamlDigest,`"message`":$message}"
}

function Install-SeedRouteRevisionArchive {
  param(
    [Parameter(Mandatory = $true)][string] $StateRoot,
    [Parameter(Mandatory = $true)][string] $RevisionId,
    [Parameter(Mandatory = $true)] $RevisionMetadata,
    [Parameter(Mandatory = $true)][string] $CanonicalDocumentJson,
    [Parameter(Mandatory = $true)][string] $CanonicalRoutesYaml
  )

  $metadataDocumentDigest = [string]$RevisionMetadata.documentDigest
  $metadataYamlDigest = [string]$RevisionMetadata.yamlDigest
  $documentDigest = Get-Utf8Sha256Hex -Text $CanonicalDocumentJson
  $yamlDigest = Get-Utf8Sha256Hex -Text $CanonicalRoutesYaml
  if ($metadataDocumentDigest -ne $documentDigest) {
    throw "Seed revision document digest mismatch: metadata=$metadataDocumentDigest generated=$documentDigest"
  }
  if ($metadataYamlDigest -ne $yamlDigest) {
    throw "Seed revision YAML digest mismatch: metadata=$metadataYamlDigest generated=$yamlDigest"
  }

  $archiveRoot = Join-Path $StateRoot "console\revisions\$RevisionId"
  $null = Ensure-Directory -Path $archiveRoot
  $metadataJson = New-SeedRevisionMetadataJson -RevisionMetadata $RevisionMetadata
  [System.IO.File]::WriteAllText(
    (Join-Path $archiveRoot "document.json"),
    $CanonicalDocumentJson,
    [System.Text.UTF8Encoding]::new($false)
  )
  [System.IO.File]::WriteAllText(
    (Join-Path $archiveRoot "routes.yaml"),
    $CanonicalRoutesYaml,
    [System.Text.UTF8Encoding]::new($false)
  )
  [System.IO.File]::WriteAllText(
    (Join-Path $archiveRoot "metadata.json"),
    $metadataJson,
    [System.Text.UTF8Encoding]::new($false)
  )
  Write-LiveLog "installed seed revision archive revision=$RevisionId"
}
