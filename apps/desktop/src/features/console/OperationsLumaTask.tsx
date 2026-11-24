import type { ConsoleRequestAudit } from "../../api/contracts";
import { lumaTaskAuditSchema } from "../../api/console/luma-task";
import { LumaTaskFacts } from "./LumaTaskFacts";
import { lumaCreationTaskSchema } from "../../api/console/luma-creation";
import { LumaMcpCreationFacts } from "./LumaMcpCreationFacts";

export function OperationsLumaTask({ row }: { row: ConsoleRequestAudit }) {
  const parsed = lumaTaskAuditSchema.safeParse(row.routeTrace?.lumaAgentsGeneration);
  const mcp = lumaCreationTaskSchema.safeParse(row.routeTrace?.lumaMcpCreation);
  return parsed.success ? <LumaTaskFacts task={parsed.data} /> : mcp.success ? <LumaMcpCreationFacts task={mcp.data} /> : null;
}
