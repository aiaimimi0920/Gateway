function Start-LiveOpenAiCompatibleUpstream {
  param(
    [Parameter(Mandatory = $true)][int] $Port,
    [Parameter(Mandatory = $true)][string] $ScriptPath,
    [Parameter(Mandatory = $true)][string] $RequestLogPath,
    [Parameter(Mandatory = $true)][string] $StdoutPath,
    [Parameter(Mandatory = $true)][string] $StderrPath
  )

  $scriptLines = @(
    'import http from "node:http";',
    'import fs from "node:fs";',
    '',
    'const port = Number.parseInt(process.argv[2] ?? "", 10);',
    'const requestLogPath = process.argv[3];',
    'if (!Number.isInteger(port) || port <= 0 || !requestLogPath) {',
    '  console.error("usage: node openai-compatible-upstream.mjs <port> <request-log-path>");',
    '  process.exit(2);',
    '}',
    '',
    'function sendJson(response, statusCode, payload) {',
    '  const body = typeof payload === "string" ? payload : JSON.stringify(payload);',
    '  response.writeHead(statusCode, {',
    '    "content-type": "application/json; charset=utf-8",',
    '    "content-length": Buffer.byteLength(body),',
    '  });',
    '  response.end(body);',
    '}',
    '',
    'function appendRequestLog(entry) {',
    '  fs.appendFileSync(requestLogPath, `${JSON.stringify(entry)}\n`, "utf8");',
    '}',
    '',
    'const server = http.createServer((request, response) => {',
    '  const host = request.headers.host ?? `127.0.0.1:${port}`;',
    '  const url = new URL(request.url ?? "/", `http://${host}`);',
    '',
    '  if (request.method === "GET" && url.pathname === "/healthz") {',
    '    sendJson(response, 200, { status: "ok" });',
    '    return;',
    '  }',
    '',
    '  if (request.method !== "POST" || url.pathname !== "/v1/chat/completions") {',
    '    sendJson(response, 404, { error: { message: "not found" } });',
    '    return;',
    '  }',
    '',
    '  let body = "";',
    '  request.setEncoding("utf8");',
    '  request.on("data", (chunk) => {',
    '    body += chunk;',
    '  });',
    '  request.on("error", (error) => {',
    '    sendJson(response, 500, { error: { message: error.message } });',
    '  });',
    '  request.on("end", () => {',
    '    let parsed;',
    '    try {',
    '      parsed = body.length > 0 ? JSON.parse(body) : {};',
    '    } catch (error) {',
    '      sendJson(response, 400, { error: { message: `invalid JSON: ${error.message}` } });',
    '      return;',
    '    }',
    '',
    '    const requestedModel = typeof parsed.model === "string" ? parsed.model.trim() : "";',
    '    const model = requestedModel.length > 0 ? requestedModel : "unknown-model";',
    '    appendRequestLog({',
    '      timestamp: new Date().toISOString(),',
    '      method: request.method,',
    '      path: url.pathname,',
    '      authorization: request.headers.authorization ?? null,',
    '      model,',
    '      body: parsed,',
    '    });',
    '',
    '    sendJson(response, 200, {',
    '      id: "chatcmpl-live-fixture",',
    '      object: "chat.completion",',
    '      created: 1720000000,',
    '      model,',
    '      choices: [',
    '        {',
    '          index: 0,',
    '          message: {',
    '            role: "assistant",',
    '            content: `live upstream ok: ${model}`,',
    '          },',
    '          finish_reason: "stop",',
    '        },',
    '      ],',
    '      usage: {',
    '        prompt_tokens: 1,',
    '        completion_tokens: 1,',
    '        total_tokens: 2,',
    '      },',
    '    });',
    '  });',
    '});',
    '',
    'server.listen(port, "127.0.0.1", () => {',
    '  console.log(JSON.stringify({ status: "ready", port }));',
    '});',
    '',
    'function shutdown() {',
    '  server.close(() => process.exit(0));',
    '}',
    '',
    'process.on("SIGTERM", shutdown);',
    'process.on("SIGINT", shutdown);'
  )
  [System.IO.File]::WriteAllText(
    $ScriptPath,
    ($scriptLines -join "`n") + "`n",
    [System.Text.UTF8Encoding]::new($false)
  )

  try {
    $nodeExecutable = Resolve-Executable -Command "node.exe"
  } catch {
    $nodeExecutable = Resolve-Executable -Command "node"
  }
  $process = Start-Process `
    -FilePath $nodeExecutable `
    -ArgumentList @($ScriptPath, [string]$Port, $RequestLogPath) `
    -WorkingDirectory (Split-Path -Parent $ScriptPath) `
    -RedirectStandardOutput $StdoutPath `
    -RedirectStandardError $StderrPath `
    -PassThru `
    -WindowStyle Hidden

  return [pscustomobject]@{
    Process = $process
    BaseUrl = "http://127.0.0.1:$Port"
    ScriptPath = $ScriptPath
    RequestLogPath = $RequestLogPath
    StdoutPath = $StdoutPath
    StderrPath = $StderrPath
  }
}

function Wait-ForLiveOpenAiCompatibleUpstream {
  param(
    [Parameter(Mandatory = $true)] $Process,
    [Parameter(Mandatory = $true)][string] $BaseUrl,
    [Parameter(Mandatory = $true)][string] $StderrPath,
    [ValidateRange(1, 60)][int] $TimeoutSeconds = 15
  )

  $deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
  $health = $null
  while ([DateTimeOffset]::UtcNow -lt $deadline) {
    if ($Process.HasExited) {
      $stderrTail = if (Test-Path -LiteralPath $StderrPath) {
        (Get-Content -LiteralPath $StderrPath -Tail 50 -Encoding UTF8) -join "`n"
      } else {
        ""
      }
      throw "OpenAI-compatible fixture upstream exited before readiness: $stderrTail"
    }

    $health = Invoke-SmokeHttpRequest -Method "GET" -Uri "$BaseUrl/healthz" -RetryCount 3
    if ($health.StatusCode -eq 200) {
      break
    }
    Start-Sleep -Milliseconds 250
  }
  Assert-StatusCode -Name "fixture /healthz" -Expected 200 -Response $health
}
