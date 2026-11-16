import { describe, expect, it, vi } from "vitest";
import { attach } from "./app/subscription";

/** A backend `subscribe` whose promise the test resolves or rejects by hand. */
function manualSubscribe() {
  let emit: ((event: number) => void) | undefined;
  let settle!: (release: () => void) => void;
  let fail!: (error: unknown) => void;
  const subscribe = (handler: (event: number) => void) => {
    emit = handler;
    return new Promise<() => void>((resolve, reject) => {
      settle = resolve;
      fail = reject;
    });
  };
  return {
    subscribe,
    emit: (event: number) => emit?.(event),
    settle: (release: () => void) => settle(release),
    fail: () => fail(new Error("no backend")),
  };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("attach", () => {
  it("delivers events until the effect is cleaned up, then releases", async () => {
    const source = manualSubscribe();
    const release = vi.fn();
    const seen: number[] = [];
    const ready = vi.fn();
    const detach = attach(source.subscribe, (event) => seen.push(event), {
      ready,
    });
    source.settle(release);
    await flush();
    expect(ready).toHaveBeenCalledTimes(1);
    source.emit(1);
    detach();
    source.emit(2);
    expect(seen).toEqual([1]);
    expect(release).toHaveBeenCalledTimes(1);
  });

  it("undoes a subscription that only finishes after the cleanup", async () => {
    const source = manualSubscribe();
    const release = vi.fn();
    const ready = vi.fn();
    const detach = attach(source.subscribe, () => undefined, { ready });
    detach();
    source.settle(release);
    await flush();
    expect(release).toHaveBeenCalledTimes(1);
    expect(ready).not.toHaveBeenCalled();
  });

  it("reports a failed subscription only while still mounted", async () => {
    const mounted = manualSubscribe();
    const failed = vi.fn();
    attach(mounted.subscribe, () => undefined, { failed });
    mounted.fail();
    await flush();
    expect(failed).toHaveBeenCalledTimes(1);

    const gone = manualSubscribe();
    const late = vi.fn();
    const detach = attach(gone.subscribe, () => undefined, { failed: late });
    detach();
    gone.fail();
    await flush();
    expect(late).not.toHaveBeenCalled();
  });

  it("tells `ready` when the component went away while it was working", async () => {
    const source = manualSubscribe();
    let stillActive: boolean | undefined;
    let finish!: () => void;
    const detach = attach(source.subscribe, () => undefined, {
      ready: async (active) => {
        await new Promise<void>((resolve) => {
          finish = resolve;
        });
        stillActive = active();
      },
    });
    source.settle(() => undefined);
    await flush();
    detach();
    finish();
    await flush();
    expect(stillActive).toBe(false);
  });
});
