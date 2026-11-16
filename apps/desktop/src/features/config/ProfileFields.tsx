import { Field, Input, SegmentedControl, Select } from "@neuro/ui";
import { useId } from "react";
import type { GatewayProfile, GatewayProfileValidation } from "../../lib/types";

export function ProfileFields({ profile, errors, onChange }: {
  profile: GatewayProfile;
  errors: GatewayProfileValidation["fieldErrors"];
  onChange(patch: Partial<GatewayProfile>): void;
}) {
  const roleName = useId();
  return <div className="gateway-profile-fields">
    <Field label="Profile 名称" error={errors.name}>
      <Input value={profile.name} onChange={event => onChange({ name: event.currentTarget.value })} />
    </Field>
    <div className="gateway-profile-fields__wide">
      <div className="gateway-runtime-role--wide"><SegmentedControl label="Gateway runtime role" name={roleName} value={profile.runtimeRole} error={errors.runtimeRole}
        options={(["standalone", "splitter", "worker"] as const).map(value => ({ value, label: value }))}
        onValueChange={runtimeRole => onChange({ runtimeRole })} /></div>
      <div className="gateway-runtime-role--narrow"><Field label="Gateway runtime role" error={errors.runtimeRole}>
        <Select value={profile.runtimeRole} onChange={event => onChange({ runtimeRole: event.currentTarget.value as GatewayProfile["runtimeRole"] })}>
          {(["standalone", "splitter", "worker"] as const).map(role => <option key={role} value={role}>{role}</option>)}
        </Select>
      </Field></div>
    </div>
    <Field label="PORT" error={errors.port}>
      <Input min={1} max={65535} type="number" value={profile.port} onChange={event => onChange({ port: Number(event.currentTarget.value) })} />
    </Field>
    <div className="gateway-profile-fields__wide"><Field label="GATEWAY_MANAGEMENT_TOKEN" error={errors.gatewayManagementToken}>
      <Input type="password" autoComplete="off" value={profile.gatewayManagementToken ?? ""}
        onChange={event => onChange({ gatewayManagementToken: event.currentTarget.value })} />
    </Field></div>
    <div className="gateway-profile-fields__wide"><Field label="GATEWAY_REDIS_URL" error={errors.gatewayRedisUrl}>
      <Input value={profile.gatewayRedisUrl} onChange={event => onChange({ gatewayRedisUrl: event.currentTarget.value })} />
    </Field></div>
    <div className="gateway-profile-fields__wide"><Field label="GATEWAY_DATABASE_URL" error={errors.gatewayDatabaseUrl}>
      <Input placeholder="可选：PostgreSQL / operator 数据库连接" value={profile.gatewayDatabaseUrl ?? ""}
        onChange={event => onChange({ gatewayDatabaseUrl: event.currentTarget.value })} />
    </Field></div>
    <div className="gateway-profile-fields__wide"><Field label="GATEWAY_ROUTES_FILE">
      <Input placeholder="可选：routes.yaml 路径" value={profile.gatewayRoutesFile ?? ""}
        onChange={event => onChange({ gatewayRoutesFile: event.currentTarget.value })} />
    </Field></div>
    <Field label="RUST_LOG"><Input value={profile.logLevel ?? ""} onChange={event => onChange({ logLevel: event.currentTarget.value })} /></Field>
    <div className="gateway-profile-fields__wide"><Field label="Working directory">
      <Input placeholder="可选：默认使用 Gateway 子项目目录" value={profile.workingDirectory ?? ""}
        onChange={event => onChange({ workingDirectory: event.currentTarget.value })} />
    </Field></div>
  </div>;
}
