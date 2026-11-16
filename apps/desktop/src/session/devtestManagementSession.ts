// Only explicitly marked manual-test builds may automatically authenticate locally.
// Management APIs still verify the key; remote hosts never receive this default.
declare const __GATEWAY_MANAGEMENT_LOGIN_REQUIRED__: boolean | undefined;

export function devtestManagementToken(apiOrigin: string): string | null {
  if (
    typeof __GATEWAY_MANAGEMENT_LOGIN_REQUIRED__ === "undefined" ||
    __GATEWAY_MANAGEMENT_LOGIN_REQUIRED__ !== false
  ) {
    return null;
  }
  const origin = new URL(apiOrigin);
  if (
    !["http:", "https:"].includes(origin.protocol) ||
    !["127.0.0.1", "localhost", "[::1]"].includes(origin.hostname)
  ) {
    return null;
  }
  return "11011101";
}
