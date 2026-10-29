// The build constant keeps the desktop settings display aligned with Rust's API port.
declare const __GATEWAY_LOCAL_PORT__: number | undefined;
export const LOCAL_GATEWAY_ADDRESS = `http://127.0.0.1:${typeof __GATEWAY_LOCAL_PORT__ === "undefined" ? 4200 : __GATEWAY_LOCAL_PORT__}`;
