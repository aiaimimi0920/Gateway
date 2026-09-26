import vm from "node:vm";

class Element {
  constructor() { this.events = []; this.isContentEditable = false; this.innerHTML = "old"; this.textContent = "old"; }
  dispatchEvent(event) { this.events.push({ ...event }); return true; }
}
class Input extends Element { value = ""; }
class Textarea extends Element { value = ""; }
class DomEvent { constructor(type, options = {}) { Object.assign(this, { type }, options); } }

export function composerFixture({ kind = "input", failedButtons = [], evaluateFailure = null, visibilityFailure = null, failedKey = null } = {}) {
  const node = kind === "input" ? new Input() : kind === "textarea" ? new Textarea() : new Element();
  if (kind === "contenteditable") node.isContentEditable = true;
  const calls = [], buttons = new Map();
  for (const key of ["role", "named", "icon"]) buttons.set(key, {
    key, first() { return this; }, last() { return this; },
    async waitFor(options) { calls.push(["wait", key, options]); },
    async click(options) { calls.push(["click", key, options]); if (failedButtons.includes(key)) throw new Error("fixture click failure"); },
  });
  const textbox = {
    first() { return this; },
    async waitFor(options) { calls.push(["textbox", options]); if (visibilityFailure) throw visibilityFailure; },
    async focus() { calls.push(["focus"]); },
    async evaluate(callback, value) {
      if (evaluateFailure) throw evaluateFailure;
      // Execute the serialized browser callback without host lexical bindings.
      return structuredClone(vm.runInNewContext(`(${callback.toString()})(__node, __value)`, {
        __node: node, __value: value, HTMLElement: Element, HTMLInputElement: Input,
        HTMLTextAreaElement: Textarea, InputEvent: DomEvent, Event: DomEvent,
      }, { timeout: 1000 }));
    },
  };
  const page = {
    locator(selector) {
      if (selector.startsWith('[role="textbox"]')) return textbox;
      if (selector.startsWith('button[aria-label')) return buttons.get("named");
      if (selector === "button:has(svg), button:has(i)") return buttons.get("icon");
      throw new Error("unexpected composer selector: " + selector);
    },
    getByRole(role, { name }) {
      if (role !== "button" || !name.test("Send") || !name.test("Submit")) throw new Error("unexpected composer role");
      return buttons.get("role");
    },
    keyboard: {
      async press(key) { calls.push(["press", key]); if (key === failedKey) throw new Error("fixture keyboard failure"); },
      async type(value, options) { calls.push(["type", value, options]); },
    },
    async waitForTimeout(milliseconds) { calls.push(["settle", milliseconds]); },
  };
  return { page, node, calls, buttons };
}
