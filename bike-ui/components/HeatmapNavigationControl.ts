import { NavigationControl, type Map } from "../lib/maplibre";

export default class HeatmapNavigationControl extends NavigationControl {
  constructor(private readonly locate: () => void) {
    super({ showCompass: false });
  }

  override onAdd(map: Map): HTMLElement {
    const container = super.onAdd(map);
    const button = document.createElement("button");
    button.type = "button";
    button.title = "Use my location";
    button.setAttribute("aria-label", "Use my location");
    button.innerHTML =
      '<svg aria-hidden="true" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" style="display:block;margin:auto"><circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3m0 14v3M2 12h3m14 0h3"/></svg>';
    button.addEventListener("click", this.locate);
    container.append(button);
    return container;
  }
}
