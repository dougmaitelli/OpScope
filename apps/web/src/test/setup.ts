import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Model popover visibility in jsdom; placement and top-layer behavior are
// verified separately in a browser.
if (!HTMLElement.prototype.showPopover) {
  HTMLElement.prototype.showPopover = function () {
    this.style.display = "block";
  };
  HTMLElement.prototype.hidePopover = function () {
    this.style.display = "none";
  };
}

// jsdom has no native dialog implementation. This models open/close state and
// events only; focus trapping, top-layer layout, and Escape need browser tests.
if (!HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    if (!this.open) return;
    this.open = false;
    this.dispatchEvent(new Event("close"));
  };
}

afterEach(() => {
  cleanup();
});
