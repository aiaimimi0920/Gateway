// Rsbuild and Vitest import these inert image assets as bundled URL strings.
declare module "*.svg" {
  const url: string;
  export default url;
}

declare module "*.png" {
  const url: string;
  export default url;
}
