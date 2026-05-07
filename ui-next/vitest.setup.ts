import "@testing-library/jest-dom";
import { vi } from "vitest";

vi.mock("leaflet", () => {
  class LayerGroupMock {
    addTo() {
      return this;
    }

    clearLayers() {
      return this;
    }
  }

  class PolylineMock {
    addTo() {
      return this;
    }

    getBounds() {
      return {
        isValid: () => true,
      };
    }
  }

  class CircleMarkerMock {
    addTo() {
      return this;
    }
  }

  const mapInstance = {
    setView: () => mapInstance,
    fitBounds: () => mapInstance,
    invalidateSize: () => mapInstance,
    remove: () => mapInstance,
  };

  return {
    default: {
      map: () => mapInstance,
      tileLayer: () => ({ addTo: () => mapInstance }),
      layerGroup: () => new LayerGroupMock(),
      polyline: () => new PolylineMock(),
      circleMarker: () => new CircleMarkerMock(),
    },
  };
});

vi.mock("leaflet/dist/leaflet.css", () => ({}));

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
