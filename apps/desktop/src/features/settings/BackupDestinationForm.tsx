import { Field, Input } from "@neuro/ui";
import type { BackupDestination } from "./gatewayBackupApi";

const fields: Record<BackupDestination["kind"], [string, string, boolean?][]> = {
  webdav: [["url", "备份文件地址（HTTPS）"], ["username", "用户名"], ["password", "密码", true]],
  s3: [["endpoint", "服务地址（HTTPS）"], ["region", "区域"], ["bucket", "存储桶"], ["key", "备份文件路径"], ["accessKeyId", "Access Key ID"], ["secretAccessKey", "Secret Access Key", true], ["sessionToken", "Session Token（可选）", true]],
  github: [["owner", "仓库所有者"], ["repository", "仓库名称"], ["branch", "分支"], ["path", "备份文件路径"], ["token", "访问令牌", true]],
};
export function emptyDestination(kind: BackupDestination["kind"]): BackupDestination {
  return Object.fromEntries([["kind", kind], ...fields[kind].map(([key]) => [key, key === "branch" ? "main" : key === "region" ? "us-east-1" : ""])]) as BackupDestination;
}
export function BackupDestinationForm({ value, disabled, onChange }: {
  value: BackupDestination; disabled: boolean; onChange: (value: BackupDestination) => void;
}) {
  return <>{fields[value.kind].map(([key, label, secret]) => <Field key={key} label={label}>
    <Input compact aria-label={label} type={secret ? "password" : "text"} value={value[key] ?? ""} disabled={disabled}
      autoComplete="off" spellCheck={false} maxLength={secret ? 8192 : 4096}
      onChange={event => onChange({ ...value, [key]: event.target.value })} />
  </Field>)}</>;
}
