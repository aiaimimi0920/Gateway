import vm from "node:vm";

export function locator(options = {}) {
  return {
    first() { return this; }, filter() { return this; },
    async waitFor() {}, async click() {}, async count() { return 1; },
    async isVisible() { return false; }, ...options,
  };
}

export const missingLocator = () => locator({
  async waitFor() { throw new Error("fixture control missing"); }, async count() { return 0; },
});

export class ElementFixture {
  constructor(values = {}) {
    Object.assign(this, {
      tagName: "IMG", innerText: "", alt: "", naturalWidth: 400, naturalHeight: 400,
      currentSrc: "", src: "", clickable: false, hidden: false, parentElement: null,
      attributes: {}, events: [], nativeClicks: 0,
    }, values);
  }
  getAttribute(name) { return this.attributes[name] ?? null; }
  getBoundingClientRect() { return { width: this.hidden ? 0 : 400, height: this.hidden ? 0 : 400 }; }
  matches() { return this.clickable; }
  scrollIntoView() {}
  dispatchEvent(event) {
    this.events.push(event.type);
    if (event.type === "click") this.onClick?.();
    return true;
  }
  click() { this.nativeClicks += 1; this.dispatchEvent({ type: "click" }); }
}

export function domPage({ images = [], controls = [], bodyText = "" } = {}) {
  const document = { body: { innerText: bodyText }, querySelectorAll: (selector) => selector === "img" ? images : controls };
  const waits = [];
  const page = {
    async evaluate(callback, argument) {
      // Serialize the actual callback so host closure references are unavailable.
      const value = vm.runInNewContext(`(${callback.toString()})(__argument)`, {
        document, __argument: argument, HTMLElement: ElementFixture,
        MouseEvent: class { constructor(type) { this.type = type; } },
        window: { getComputedStyle: (element) => ({ display: element.hidden ? "none" : "block", visibility: "visible", cursor: element.clickable ? "pointer" : "default" }) },
      }, { timeout: 1000 });
      return structuredClone(value);
    },
    async waitForTimeout(milliseconds) { waits.push(milliseconds); },
  };
  return { page, document, waits };
}
