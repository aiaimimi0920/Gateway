const COMMERCIAL_IDC_REGIONS = new Set([
  "us-east-2", "us-east-1", "us-west-1", "us-west-2", "af-south-1", "ap-east-1",
  "ap-south-2", "ap-southeast-3", "ap-southeast-5", "ap-southeast-4", "ap-south-1",
  "ap-southeast-6", "ap-northeast-3", "ap-northeast-2", "ap-southeast-1", "ap-southeast-2",
  "ap-east-2", "ap-southeast-7", "ap-northeast-1", "ca-central-1", "ca-west-1",
  "eu-central-1", "eu-west-1", "eu-west-2", "eu-south-1", "eu-west-3", "eu-south-2",
  "eu-north-1", "eu-central-2", "il-central-1", "mx-central-1", "me-south-1",
  "me-central-1", "sa-east-1",
]);

export function quickAuthorizationUrl(value: string): string | null {
  if (!value || value.length > 8192 || /[\x00-\x20]/.test(value)) return null;
  try {
    const url = new URL(value);
    if (url.protocol !== "https:" || url.port || url.username || url.password || url.hash) {
      return null;
    }
    if (url.origin === "https://quick.aws.com" && url.pathname === "/sn") {
      return url.searchParams.has("qwSurface") ? url.href : null;
    }
    const match = /^oidc\.([a-z0-9-]+)\.amazonaws\.com$/.exec(url.hostname);
    if (match && COMMERCIAL_IDC_REGIONS.has(match[1]) && url.pathname === "/authorize") {
      return url.searchParams.get("response_type") === "code" ? url.href : null;
    }
    return null;
  } catch {
    return null;
  }
}
