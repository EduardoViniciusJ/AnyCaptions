import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { DictationWindow } from "./DictationWindow";
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), release: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ listen: mocks.listen }) }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const event = { phase: "recording", text: "Live dictation", error: null, soundCue: true, activation: "hold", showLiveText: true, maxDurationSecs: 60 };
it("renders the latest event instead of an older startup snapshot", async () => {
  mocks.listen.mockImplementation(async (_name, callback) => {
    callback({ payload: { revision: 2, uiLanguage: "en", event } });
    return mocks.release;
  });
  mocks.invoke.mockResolvedValue({ revision: 1, uiLanguage: "en", event: { ...event, text: "Old text" } });
  const view = render(<DictationWindow />);
  await act(async () => {});
  expect(screen.getByRole("status").textContent).toBe("Live dictation");
  expect(view.container.querySelector(".toolbar")).toBeNull();
  view.unmount();
  expect(mocks.release).toHaveBeenCalledOnce();
});
it("hides the panel content after dictation ends", async () => {
  let emit: (value: unknown) => void = () => {};
  mocks.listen.mockImplementation(async (_name, callback) => { emit = callback; return mocks.release; });
  mocks.invoke.mockResolvedValue({ revision: 1, uiLanguage: "en", event });
  const view = render(<DictationWindow />);
  await act(async () => {});
  act(() => emit({ payload: { revision: 2, uiLanguage: "en", event: { ...event, phase: "idle", text: "" } } }));
  expect(view.container.querySelector<HTMLElement>("#dictation-indicator")?.hidden).toBe(true);
});
