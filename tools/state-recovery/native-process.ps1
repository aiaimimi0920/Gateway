# Native capture and Windows argument quoting for state recovery tools.
# Callers retain path validation and provide Resolve-FullPath.

function Resolve-NativeExecutable {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string]$Name
    )

    if (Test-Path -LiteralPath $Command -PathType Leaf) {
        return (Resolve-FullPath -Path $Command -RequireExisting)
    }
    $resolved = Get-Command $Command -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $resolved) {
        throw "$Name executable was not found. Supply an explicit command path."
    }
    return $resolved.Source
}

function ConvertTo-ProcessArgument {
    param([AllowEmptyString()][string]$Value)

    if ($Value.Length -gt 0 -and $Value -notmatch '[\s"]') {
        return $Value
    }

    $builder = [System.Text.StringBuilder]::new()
    [void]$builder.Append('"')
    $backslashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq '\') {
            $backslashes += 1
            continue
        }
        if ($character -eq '"') {
            [void]$builder.Append(('\' * (($backslashes * 2) + 1)))
            [void]$builder.Append('"')
            $backslashes = 0
            continue
        }
        if ($backslashes -gt 0) {
            [void]$builder.Append(('\' * $backslashes))
            $backslashes = 0
        }
        [void]$builder.Append($character)
    }
    if ($backslashes -gt 0) {
        [void]$builder.Append(('\' * ($backslashes * 2)))
    }
    [void]$builder.Append('"')
    return $builder.ToString()
}

function Invoke-NativeCapture {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Operation,
        [byte[]]$InputBytes = $null
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.Arguments = (($Arguments | ForEach-Object {
        ConvertTo-ProcessArgument -Value ([string]$_)
    }) -join " ")
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.RedirectStandardInput = $null -ne $InputBytes

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $inputEncoding = $null
    try {
        if ($null -ne $InputBytes) {
            # PowerShell 5.1 derives redirected stdin encoding from the console.
            # Suppress its preamble before any bytes reach the child process.
            $inputEncoding = [Console]::InputEncoding
            [Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
        }
        if (-not $process.Start()) {
            throw "$Operation failed to start."
        }
    } finally {
        if ($null -ne $inputEncoding) {
            [Console]::InputEncoding = $inputEncoding
        }
    }

    $output = [System.IO.MemoryStream]::new()
    $outputTask = $process.StandardOutput.BaseStream.CopyToAsync($output)
    $errorTask = $process.StandardError.ReadToEndAsync()
    if ($null -ne $InputBytes) {
        $process.StandardInput.BaseStream.Write($InputBytes, 0, $InputBytes.Length)
        $process.StandardInput.BaseStream.Flush()
        $process.StandardInput.Close()
    }

    $null = $process.WaitForExit()
    $null = $outputTask.GetAwaiter().GetResult()
    $null = $errorTask.GetAwaiter().GetResult()
    $exitCode = $process.ExitCode
    $bytes = $output.ToArray()
    $output.Dispose()
    $process.Dispose()

    if ($exitCode -ne 0) {
        throw "$Operation failed with exit code $exitCode. Endpoint details were redacted."
    }

    return [pscustomobject]@{
        Bytes = $bytes
        Text = [System.Text.Encoding]::UTF8.GetString($bytes)
    }
}
