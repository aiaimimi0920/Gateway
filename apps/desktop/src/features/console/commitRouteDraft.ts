import { GatewayApiError } from "../../api/errors";
import type { ConsoleRouteConfigCommitRequest, ConsoleRouteConfigResponse } from "../../api/contracts";
import type { ConsoleActionRequest } from "./useConsoleActionIdentity";
import { rebaseRouteDraft } from "./rebaseRouteDraft";

const revisionConflict = (cause: unknown) => cause instanceof GatewayApiError && cause.status === 409 && cause.code === "console_revision_conflict";

/** One conflict retry only, against a freshly merged snapshot and the original session. */
export async function commitRouteDraft(action: ConsoleActionRequest, base: ConsoleRouteConfigResponse | null,
  draft: ConsoleRouteConfigCommitRequest, isCurrent: () => boolean) {
  const send = (request: ConsoleRouteConfigCommitRequest) => action.secretGrant
    ? action.api.commitRouteConfig(action.managementToken, request, action.secretGrant)
    : action.api.commitRouteConfig(action.managementToken, request);
  try { return await send(draft); }
  catch (cause) {
    if (!revisionConflict(cause) || !base) throw cause;
    if (!isCurrent()) return null;
    const latest = await action.api.getRouteConfig(action.managementToken);
    if (!isCurrent()) return null;
    const rebased = rebaseRouteDraft(base.routeConfig, draft, latest.routeConfig);
    try { return await send(rebased); }
    catch (retryError) {
      if (revisionConflict(retryError)) throw new Error("配置仍在更新，本次修改尚未保存；请稍后重试。你的修改仍保留。");
      throw retryError;
    }
  }
}
