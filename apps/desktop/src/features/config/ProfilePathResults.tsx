import { Badge, Notice } from "@neuro/ui";
import type { GatewayDependencyCheckItem, GatewayProfilePathCheck, GatewayProfilePathCheckItem } from "../../lib/types";

function PathResult({ label, item }: { label: string; item?: GatewayProfilePathCheckItem }) {
  if (!item) return null;
  return <div className="gateway-path-result">
    <span>{label}</span><Badge tone={item.ok ? "success" : "warning"}>{item.ok ? "OK" : "CHECK"}</Badge>
    <code>{item.resolvedPath || "(not configured)"}</code>
    <small>{item.message} · expected {item.expectedKind} · exists {String(item.exists)}</small>
  </div>;
}
function DependencyResult({ item }: { item?: GatewayDependencyCheckItem }) {
  if (!item) return null;
  return <div className="gateway-path-result">
    <span>{item.name}</span><Badge tone={item.ok ? "success" : "warning"}>{item.ok ? "OK" : item.required ? "REQUIRED" : "CHECK"}</Badge>
    <small>{item.message}</small>
  </div>;
}
export function ProfilePathResults({ result }: { result: GatewayProfilePathCheck }) {
  return <div className="gateway-path-results" aria-label="Profile path check result">
    <PathResult label="Gateway sidecar" item={result.sidecar} />
    <PathResult label="Working directory" item={result.workingDirectory} />
    <PathResult label="GATEWAY_ROUTES_FILE" item={result.gatewayRoutesFile} />
    <DependencyResult item={result.redis} /><DependencyResult item={result.database} />
    {result.preflightMessages.length ? <Notice tone="warning"><strong>Preflight</strong><ul>
      {result.preflightMessages.map(message => <li key={message}>{message}</li>)}
    </ul></Notice> : null}
  </div>;
}
