export class GatewayApiError extends Error {
  readonly status: number;
  readonly code?: string;
  readonly errorType?: string;

  constructor(message: string, status: number, code?: string, errorType?: string) {
    super(message);
    this.name = "GatewayApiError";
    this.status = status;
    this.code = code;
    this.errorType = errorType;
  }
}

export class GatewayResponseValidationError extends Error {
  readonly path: string;
  readonly issues: readonly string[];

  constructor(path: string, issues: readonly string[]) {
    super(`Gateway response for ${path} did not match its schema.`);
    this.name = "GatewayResponseValidationError";
    this.path = path;
    this.issues = issues;
  }
}

export function isAuthenticationError(error: unknown): error is GatewayApiError {
  return error instanceof GatewayApiError && (error.status === 401 || error.status === 403);
}
