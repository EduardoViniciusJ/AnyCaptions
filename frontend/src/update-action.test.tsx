import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { createElement } from "react";
import { UpdateAction } from "./components/UpdateAction";
import type { Backend, UpdateCheck, UpdateProgress } from "./contracts";

afterEach(cleanup);

const check = (installable: boolean): UpdateCheck => ({
  current: "0.2.0",
  latest: "0.3.0",
  available: true,
  installable,
});

function setup(installable: boolean, install: () => Promise<void> = () => Promise.resolve()) {
  let emit: ((progress: UpdateProgress) => void) | undefined;
  const backend = {
    installUpdate: vi.fn(install),
    openReleasesPage: vi.fn().mockResolvedValue(undefined),
    subscribeUpdateProgress: vi.fn(async (listener: (progress: UpdateProgress) => void) => {
      emit = listener;
      return vi.fn();
    }),
  } as unknown as Backend;
  const report = vi.fn();
  const view = render(
    createElement(UpdateAction, { backend, check: check(installable), id: "action", report }),
  );
  return { backend, report, view, emit: (progress: UpdateProgress) => act(() => emit!(progress)) };
}

const flush = () => act(async () => {});

describe("UpdateAction", () => {
  it("opens the releases page when this build cannot install by itself", () => {
    const { backend, view } = setup(false);
    const button = view.container.querySelector<HTMLButtonElement>("#action")!;
    expect(button.textContent).toBe("Baixar");
    fireEvent.click(button);
    expect(backend.openReleasesPage).toHaveBeenCalledOnce();
    expect(backend.installUpdate).not.toHaveBeenCalled();
  });

  it("installs only on click and shows the progress in place of the button", async () => {
    const { backend, view, emit } = setup(true);
    await flush();
    expect(backend.installUpdate).not.toHaveBeenCalled();
    const button = view.container.querySelector<HTMLButtonElement>("#action")!;
    expect(button.textContent).toBe("Atualizar");
    fireEvent.click(button);
    expect(backend.installUpdate).toHaveBeenCalledOnce();
    expect(view.container.querySelector("#action")).toBeNull();

    emit({ phase: "downloading", receivedBytes: 45, totalBytes: 100 });
    expect(view.container.querySelector("#action-progress")?.textContent).toContain("45%");
    emit({ phase: "installing", receivedBytes: 100, totalBytes: 100 });
    expect(view.container.querySelector("#action-progress")?.textContent).toContain("Instalando");
  });

  it("explains a failed installation and offers the releases page", async () => {
    const { backend, view, emit } = setup(true);
    await flush();
    fireEvent.click(view.container.querySelector("#action")!);
    emit({ phase: "failed", receivedBytes: 0, totalBytes: 0, reason: "signature" });
    expect(view.container.querySelector("#action-failed")?.textContent).toContain("Nada mudou");
    const button = view.container.querySelector<HTMLButtonElement>("#action")!;
    expect(button.textContent).toBe("Baixar");
    fireEvent.click(button);
    expect(backend.openReleasesPage).toHaveBeenCalledOnce();
  });

  it("recovers when the install command itself is refused", async () => {
    const refusal = { code: "update-busy" };
    const { view, report } = setup(true, () => Promise.reject(refusal));
    await flush();
    fireEvent.click(view.container.querySelector("#action")!);
    await flush();
    expect(report).toHaveBeenCalledWith(refusal);
    expect(view.container.querySelector("#action-failed")).not.toBeNull();
  });
});
