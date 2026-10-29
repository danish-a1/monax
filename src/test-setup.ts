import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(cleanup);
URL.createObjectURL = vi.fn(() => "blob:thumb");
URL.revokeObjectURL = vi.fn();

// jsdom has no IntersectionObserver: by default every observed element is visible at once.
globalThis.IntersectionObserver = class {
  constructor(private cb: IntersectionObserverCallback) {}
  observe(target: Element) {
    this.cb([{ isIntersecting: true, target } as unknown as IntersectionObserverEntry], this as unknown as IntersectionObserver);
  }
  unobserve() {}
  disconnect() {}
} as unknown as typeof IntersectionObserver;
