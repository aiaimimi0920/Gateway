/** Explicit Bedrock material, kept only in the dialog and SecretPatch draft, never public JSON. */
export const BEDROCK_REGIONS = ["us-east-1", "us-east-2", "us-west-2", "ap-south-1", "ap-south-2", "ap-northeast-1", "ap-northeast-2",
  "ap-northeast-3", "ap-southeast-1", "ap-southeast-2", "ca-central-1", "eu-central-1", "eu-central-2", "eu-west-1", "eu-west-2",
  "eu-west-3", "eu-south-1", "eu-south-2", "eu-north-1", "sa-east-1", "us-gov-east-1", "us-gov-west-1"] as const;
export const BEDROCK_FIPS_REGIONS: readonly string[] = ["us-east-1", "us-east-2", "us-west-2", "ca-central-1", "us-gov-east-1", "us-gov-west-1"];
export type BedrockAuthMode = "bedrock_bearer" | "aws_sigv4" | "aws_sigv4_temporary";
export type BedrockMaterial = { authMode: BedrockAuthMode | ""; region: string; fips: boolean;
  apiKey: string; accessKeyId: string; secretAccessKey: string; sessionToken: string };
export const record = (value: unknown): Record<string, unknown> => value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
export function isBedrockProvider(value: unknown): boolean {
  const p = record(value);
  return p.preset === "bedrock-converse" || p.adapter === "bedrock_converse_compatible";
}
export function awsMaterialField(key: string): string | undefined {
  const compact = key.replace(/[_-]/g, "").replace(/[A-Z]/g, (letter) => letter.toLowerCase());
  switch (compact) {
    case "awsaccesskeyid": return "access";
    case "awssecretaccesskey": return "secret";
    case "awssessiontoken": return "session";
    case "awsregion": case "region": return "region";
    default: return undefined;
  }
}
export function bedrockEndpoint(region: string, fips: boolean): string {
  if (!(BEDROCK_REGIONS as readonly string[]).includes(region) || fips && !BEDROCK_FIPS_REGIONS.includes(region)) {
    throw new Error("请选择本批支持的 AWS 公共 runtime 区域；FIPS 仅支持列出的六个区域。");
  }
  return `https://bedrock-runtime${fips ? "-fips" : ""}.${region}.amazonaws.com`;
}
export function bedrockMaterialDraft(provider: Record<string, unknown>, credential: Record<string, unknown> = {}, adding = false): BedrockMaterial {
  const mode = credential.auth_mode ?? credential.authMode ?? provider.auth_mode ?? provider.authMode;
  const extra = { ...record(provider.extra_body), ...record(credential.extra_body) };
  const regions = [...new Set(Object.entries(extra).filter(([key]) => awsMaterialField(key) === "region").map(([, value]) => value))];
  const base = String(credential.base_url ?? provider.base_url ?? "");
  const host = /^https:\/\/bedrock-runtime(-fips)?\.([a-z0-9-]+)\.amazonaws\.com\/?$/.exec(base);
  return { authMode: ["bedrock_bearer", "aws_sigv4", "aws_sigv4_temporary"].includes(String(mode)) ? mode as BedrockAuthMode : adding ? "bedrock_bearer" : "",
    region: regions.length ? regions.length === 1 && typeof regions[0] === "string" ? regions[0] : "" : host?.[2] ?? "",
    fips: Boolean(host?.[1]), apiKey: "", accessKeyId: "", secretAccessKey: "", sessionToken: "" };
}
export function withoutBedrockSecrets(material: BedrockMaterial): BedrockMaterial {
  return { ...material, apiKey: "", accessKeyId: "", secretAccessKey: "", sessionToken: "" };
}
export function validateBedrockMaterial(material: BedrockMaterial): string {
  const endpoint = bedrockEndpoint(material.region, material.fips);
  const valid = (value: string) => value.length > 0 && value.length <= 32768 && /^[\x21-\x7e]+$/.test(value);
  if (material.authMode === "bedrock_bearer") {
    if (!valid(material.apiKey) || material.accessKeyId || material.secretAccessKey || material.sessionToken) throw new Error("请填写原始 Bedrock API Key，不加 Bearer 前缀，不混入 IAM 材料。");
  } else if (material.authMode === "aws_sigv4" || material.authMode === "aws_sigv4_temporary") {
    if (material.apiKey || !valid(material.accessKeyId) || !valid(material.secretAccessKey)
      || (material.authMode === "aws_sigv4_temporary" || material.sessionToken) && !valid(material.sessionToken)) {
      throw new Error("请填写完整 AWS Access Key ID / Secret Access Key；临时 IAM 还必须有 Session Token。");
    }
  } else throw new Error("请选择显式 Bedrock 认证模式；存量配置不会自动转换。");
  return endpoint;
}
