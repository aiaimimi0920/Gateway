export function resolve(specifier, context, nextResolve) {
  if (specifier === "playwright-core") {
    return { url: new URL("./chataibot-browser.mjs", import.meta.url).href, shortCircuit: true };
  }
  if (specifier === "node:fs/promises" &&
      /chataibot-session(?:-worker\.mjs|\/profile\.mjs)$/.test(context.parentURL ?? "")) {
    return { url: new URL("./chataibot-profile-io.mjs", import.meta.url).href, shortCircuit: true };
  }
  return nextResolve(specifier, context);
}
