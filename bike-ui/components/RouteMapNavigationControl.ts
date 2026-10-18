import { NavigationControl, type Map as MapLibreMap } from "../lib/maplibre";
import type { RouteMapOverlayLayers } from "./RouteMapTypes";

export default class RouteMapNavigationControl extends NavigationControl {
  private container?: HTMLElement;
  private button?: HTMLButtonElement;
  private panel?: HTMLDivElement;
  private inputs = new Map<string, HTMLInputElement>();

  constructor(private layers?: RouteMapOverlayLayers) {
    super({ showCompass: false, visualizePitch: false });
  }

  override onAdd(map: MapLibreMap): HTMLElement {
    this.container = super.onAdd(map);
    this.setLayers(this.layers);
    return this.container;
  }

  setLayers(layers?: RouteMapOverlayLayers) {
    this.layers = layers;
    if (layers && !this.button) this.addLayerPicker();
    if (this.button) this.button.style.display = layers ? "" : "none";
    for (const [id, input] of this.inputs) {
      input.checked = Boolean(layers?.[id as "segments" | "climbs"]);
    }
    if (!layers) this.setOpen(false);
  }

  override onRemove() {
    document.removeEventListener("pointerdown", this.dismissOutside);
    document.removeEventListener("keydown", this.handleKeyDown);
    this.container = undefined;
    this.button = undefined;
    this.panel = undefined;
    this.inputs.clear();
    super.onRemove();
  }

  private addLayerPicker() {
    if (!this.container) return;
    this.container.style.position = "relative";
    this.button = document.createElement("button");
    this.button.type = "button";
    this.button.title = "Map layers";
    this.button.setAttribute("aria-label", "Map layers");
    this.button.setAttribute("aria-expanded", "false");
    this.button.innerHTML =
      '<svg aria-hidden="true" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="#333" stroke-width="1.8" style="display:block;margin:auto"><path d="m3 8 9-5 9 5-9 5-9-5Zm0 4 9 5 9-5M3 16l9 5 9-5"/></svg>';
    this.button.addEventListener("click", () =>
      this.setOpen(this.panel?.hidden ?? false),
    );
    this.panel = document.createElement("div");
    this.panel.id = `map-layers-${crypto.randomUUID()}`;
    this.button.setAttribute("aria-controls", this.panel.id);
    this.panel.setAttribute("role", "group");
    this.panel.setAttribute("aria-label", "Map layers");
    this.panel.className =
      "absolute bottom-0 right-9 z-20 w-44 space-y-3 rounded-box border border-base-300 bg-base-100 p-3 text-sm text-base-content shadow-lg";
    this.panel.hidden = true;
    for (const id of ["segments", "climbs"] as const) this.addCheckbox(id);
    this.container.append(this.button, this.panel);
    document.addEventListener("pointerdown", this.dismissOutside);
    document.addEventListener("keydown", this.handleKeyDown);
  }

  private addCheckbox(id: "segments" | "climbs") {
    const label = document.createElement("label");
    label.className = "flex cursor-pointer items-center justify-between gap-3";
    const name = document.createElement("span");
    name.textContent = id === "segments" ? "Segments" : "Climbs";
    const input = document.createElement("input");
    input.type = "checkbox";
    input.className = "checkbox checkbox-sm";
    input.addEventListener("change", () =>
      this.layers?.onChange(id, input.checked),
    );
    label.append(name, input);
    this.panel?.append(label);
    this.inputs.set(id, input);
  }

  private setOpen(open: boolean) {
    if (this.panel) this.panel.hidden = !open;
    this.button?.setAttribute("aria-expanded", String(open));
  }

  private dismissOutside = (event: PointerEvent) => {
    if (!this.container?.contains(event.target as Node)) this.setOpen(false);
  };

  private handleKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || this.panel?.hidden) return;
    event.preventDefault();
    this.setOpen(false);
    this.button?.focus();
  };
}
