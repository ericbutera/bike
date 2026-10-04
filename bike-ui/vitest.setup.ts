import "@testing-library/jest-dom";
import { vi } from "vitest";

global.IntersectionObserver = class IntersectionObserver {
  constructor(
    _cb: IntersectionObserverCallback,
    _opts?: IntersectionObserverInit,
  ) {}
  observe() {
    return null;
  }
  unobserve() {
    return null;
  }
  disconnect() {
    return null;
  }
  takeRecords(): IntersectionObserverEntry[] {
    return [];
  }
  readonly root: Element | null = null;
  readonly rootMargin = "";
  readonly thresholds: ReadonlyArray<number> = [];
};
