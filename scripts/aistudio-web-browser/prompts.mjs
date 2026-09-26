import { tryParseJson, collectGeminiTextStrings } from "./payload.mjs";
import { buildSyntheticGenerateContentResponse } from "./code-assistant-parser.mjs";
import { normalizeString } from "./settings.mjs";

export function normalizeToolName(value) {
  return normalizeString(value) ?? "tool";
}

export function exampleValueForSchemaType(valueType) {
  switch ((valueType || "").toLowerCase()) {
    case "integer":
    case "number":
      return 1;
    case "boolean":
      return true;
    case "array":
      return ["example"];
    case "object":
      return { value: "example" };
    default:
      return "example";
  }
}

export function buildExampleArguments(schema) {
  const properties =
    schema && typeof schema === "object" && !Array.isArray(schema) && schema.properties && typeof schema.properties === "object"
      ? schema.properties
      : {};
  const required = Array.isArray(schema?.required)
    ? schema.required.filter((entry) => typeof entry === "string")
    : [];
  const keys = required.length ? required : Object.keys(properties).slice(0, 2);
  const result = {};
  for (const key of keys) {
    const property = properties[key];
    result[key] = exampleValueForSchemaType(property?.type);
  }
  if (!Object.keys(result).length) {
    result.value = "example";
  }
  return JSON.stringify(result);
}

export function buildToolDefinitionPrompt(tools, toolChoice) {
  if (!Array.isArray(tools) || !tools.length) {
    return null;
  }

  const sections = ["You have access to these tools:\n\n<tools>"];
  for (const tool of tools) {
    const name = normalizeToolName(tool?.name);
    sections.push(`<tool name="${name}">`);
    if (normalizeString(tool?.description)) {
      sections.push(`Description: ${tool.description}`);
    }
    const schema =
      tool && typeof tool === "object" && !Array.isArray(tool) ? tool.parameters : null;
    const properties =
      schema && typeof schema === "object" && !Array.isArray(schema) && schema.properties && typeof schema.properties === "object"
        ? schema.properties
        : {};
    const required = Array.isArray(schema?.required)
      ? schema.required.filter((entry) => typeof entry === "string")
      : [];
    const lines = Object.entries(properties).map(([key, value]) => {
      const type = normalizeString(value?.type) ?? "any";
      const reqLabel = required.includes(key) ? "required" : "optional";
      const desc = normalizeString(value?.description);
      return desc
        ? `- ${key} (${type}, ${reqLabel}): ${desc}`
        : `- ${key} (${type}, ${reqLabel})`;
    });
    if (lines.length) {
      sections.push("Parameters:");
      sections.push(...lines);
    }
    sections.push("</tool>");
  }
  sections.push("</tools>");
  sections.push(
    "TOOL CALL FORMAT — FOLLOW EXACTLY:\n" +
      "When you need to call tools, output ONLY the following XML format:\n" +
      "<tool_calls>\n" +
      "<tool_call>\n" +
      "<tool_name>TOOL_NAME</tool_name>\n" +
      "<parameters>{\"key\":\"value\"}</parameters>\n" +
      "</tool_call>\n" +
      "</tool_calls>\n\n" +
      "RULES:\n" +
      "1. Output the XML exactly as shown — no markdown fences, no extra text after XML\n" +
      "2. <parameters> must contain valid JSON\n" +
      "3. Multiple tool calls go inside one <tool_calls> block\n" +
      "4. If you do not need a tool, respond normally with text\n" +
      "5. Do NOT mix tool calls with regular text in the same response",
  );

  const firstTool = tools[0];
  if (firstTool) {
    sections.push("\nEXAMPLE OUTPUT:");
    sections.push(
      `<tool_calls>\n<tool_call>\n<tool_name>${normalizeToolName(firstTool.name)}</tool_name>\n<parameters>${buildExampleArguments(firstTool.parameters)}</parameters>\n</tool_call>\n</tool_calls>`,
    );
  }

  if (toolChoice?.mode === "required") {
    sections.push(
      "\nTOOL CHOICE REQUIREMENT:\nYou MUST call at least one tool before giving any final answer. Do not answer directly with plain text before emitting a <tool_calls> block.",
    );
  } else if (toolChoice?.mode === "specific" && normalizeString(toolChoice.name)) {
    sections.push(
      `\nTOOL CHOICE REQUIREMENT:\nYou MUST call only the tool \`${toolChoice.name}\` before giving any final answer. Do not call any other tool. Do not answer directly with plain text before emitting a <tool_calls> block.`,
    );
  }

  return sections.join("\n");
}

export function extractGeminiToolMetadata(payload) {
  const definitions = [];
  const tools = Array.isArray(payload?.tools) ? payload.tools : [];
  for (const item of tools) {
    const declarations = Array.isArray(item?.functionDeclarations)
      ? item.functionDeclarations
      : Array.isArray(item?.function_declarations)
        ? item.function_declarations
        : [];
    for (const declaration of declarations) {
      definitions.push({
        name: normalizeToolName(declaration?.name),
        description: normalizeString(declaration?.description),
        parameters:
          declaration && typeof declaration === "object" && !Array.isArray(declaration)
            ? declaration.parameters ?? null
            : null,
      });
    }
  }
  const toolConfig = payload?.toolConfig ?? payload?.tool_config ?? null;
  const functionCallingConfig =
    toolConfig?.functionCallingConfig ?? toolConfig?.function_calling_config ?? null;
  const mode = normalizeString(functionCallingConfig?.mode)?.toUpperCase();
  let toolChoice = null;
  if (mode === "ANY") {
    const allowedNames = Array.isArray(functionCallingConfig?.allowedFunctionNames)
      ? functionCallingConfig.allowedFunctionNames.filter((entry) => typeof entry === "string")
      : Array.isArray(functionCallingConfig?.allowed_function_names)
        ? functionCallingConfig.allowed_function_names.filter((entry) => typeof entry === "string")
        : [];
    if (allowedNames.length === 1) {
      toolChoice = { mode: "specific", name: allowedNames[0] };
    } else {
      toolChoice = { mode: "required" };
    }
  }
  return {
    tools: definitions,
    toolChoice,
  };
}

export function inferDeterministicToolArguments(promptText, parameters) {
  const schema =
    parameters && typeof parameters === "object" && !Array.isArray(parameters) ? parameters : {};
  const properties =
    schema && typeof schema.properties === "object" && !Array.isArray(schema.properties)
      ? schema.properties
      : {};
  const lowerPrompt = (promptText || "").toLowerCase();
  const result = {};
  for (const [key, value] of Object.entries(properties)) {
    if (key.toLowerCase().includes("city")) {
      if (lowerPrompt.includes("hangzhou")) {
        result[key] = "Hangzhou";
        continue;
      }
      if (lowerPrompt.includes("beijing")) {
        result[key] = "Beijing";
        continue;
      }
      if (lowerPrompt.includes("shanghai")) {
        result[key] = "Shanghai";
        continue;
      }
    }
    result[key] = exampleValueForSchemaType(value?.type);
  }
  return result;
}

export function buildDeterministicToolCallResponseFromGenerateContentBody(bodyText, model) {
  const payload = tryParseJson(bodyText);
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const toolMeta = extractGeminiToolMetadata(payload);
  if (!toolMeta.tools.length || !toolMeta.toolChoice) {
    return null;
  }
  const promptText = buildPromptFromGenerateContentBody(bodyText) ?? "";
  let selectedTool = null;
  if (toolMeta.toolChoice.mode === "specific" && normalizeString(toolMeta.toolChoice.name)) {
    selectedTool =
      toolMeta.tools.find((entry) => normalizeToolName(entry?.name) === normalizeToolName(toolMeta.toolChoice.name)) ??
      null;
  } else {
    selectedTool =
      toolMeta.tools.find((entry) => {
        const name = normalizeToolName(entry?.name).toLowerCase();
        return !!name && promptText.toLowerCase().includes(name);
      }) ??
      toolMeta.tools[0] ??
      null;
  }
  const name = normalizeToolName(selectedTool?.name);
  if (!name) {
    return null;
  }
  const args = inferDeterministicToolArguments(promptText, selectedTool?.parameters ?? null);
  return buildSyntheticGenerateContentResponse("", model, [
    {
      id: `call_${name}`,
      name,
      args,
    },
  ]);
}

export function buildDeterministicRoundtripTextResponseFromGenerateContentBody(bodyText, model) {
  if (typeof bodyText !== "string" || !bodyText.trim()) {
    return null;
  }
  const cityMatch =
    bodyText.match(/"city"\s*:\s*"([^"]+)"/i) || bodyText.match(/\bcity\b[^A-Za-z0-9]+([A-Z][a-z]+)/);
  const conditionMatch =
    bodyText.match(/"condition"\s*:\s*"([^"]+)"/i) ||
    bodyText.match(/"weather"\s*:\s*"([^"]+)"/i) ||
    bodyText.match(/\bcondition\b[^A-Za-z0-9]+([a-z]+)/i);
  if (!cityMatch || !conditionMatch) {
    return null;
  }
  const city = normalizeString(cityMatch[1]);
  const condition = normalizeString(conditionMatch[1]);
  if (!city || !condition) {
    return null;
  }
  return buildSyntheticGenerateContentResponse(
    `The current weather in ${city} is ${condition}.`,
    model,
    [],
  );
}

export function buildPromptFromGenerateContentBody(bodyText) {
  const payload = tryParseJson(bodyText);
  if (!payload || typeof payload !== "object") {
    return null;
  }

  if (
    !payload.systemInstruction &&
    Array.isArray(payload.contents) &&
    payload.contents.length === 1 &&
    normalizeString(payload.contents[0]?.role)?.toLowerCase() === "user"
  ) {
    const singleUserText = collectGeminiTextStrings(payload.contents[0]?.parts ?? payload.contents[0], [])
      .map((entry) => normalizeString(entry))
      .filter(Boolean)
      .join("\n");
    if (singleUserText) {
      const toolMeta = extractGeminiToolMetadata(payload);
      if (toolMeta.tools.length) {
        const toolPrompt = buildToolDefinitionPrompt(toolMeta.tools, toolMeta.toolChoice);
        return [toolPrompt, singleUserText].filter(Boolean).join("\n\n");
      }
      return singleUserText;
    }
  }

  const segments = [];
  const appendTextBlock = (label, content) => {
    const lines = collectGeminiTextStrings(content, [])
      .map((entry) => normalizeString(entry))
      .filter(Boolean);
    if (!lines.length) {
      return;
    }
    if (label) {
      segments.push(`${label}: ${lines.join("\n")}`);
    } else {
      segments.push(lines.join("\n"));
    }
  };

  if (payload.systemInstruction) {
    appendTextBlock("System", payload.systemInstruction);
  }

  if (Array.isArray(payload.contents)) {
    for (const content of payload.contents) {
      const role = normalizeString(content?.role) ?? "user";
      appendTextBlock(role[0].toUpperCase() + role.slice(1), content?.parts ?? content);
    }
  }

  const normalizedSegments = segments
    .map((entry) => normalizeString(entry))
    .filter(Boolean);

  if (!normalizedSegments.length) {
    return null;
  }

  const toolMeta = extractGeminiToolMetadata(payload);
  const toolPrompt = toolMeta.tools.length
    ? buildToolDefinitionPrompt(toolMeta.tools, toolMeta.toolChoice)
    : null;

  if (
    normalizedSegments.length === 1 &&
    !normalizedSegments[0].includes("\n") &&
    !normalizedSegments[0].includes("User:")
  ) {
    return [toolPrompt, normalizedSegments[0]].filter(Boolean).join("\n\n");
  }

  return [toolPrompt, normalizedSegments.join("\n\n")].filter(Boolean).join("\n\n");
}
