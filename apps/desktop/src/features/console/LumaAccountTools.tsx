import { LumaFilesPanel } from "./LumaFilesPanel";
import { LumaTaskPanel } from "./LumaTaskPanel";
import type { LumaFilesProps } from "./useLumaFiles";

export function LumaAccountTools(props: LumaFilesProps) {
  return <><LumaTaskPanel {...props} /><LumaFilesPanel {...props} /></>;
}
