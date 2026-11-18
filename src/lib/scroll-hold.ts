import { tick } from "svelte";

type Hold = { pane: HTMLElement; content: HTMLElement; top: number };

// A min-height floor keeps an item pane from collapsing to the top while an edit rebuilds its content.
export function createScrollHold(contentSelector = ".detailcontent") {
  let hold: Hold | null = null;
  const floor = (h: Hold) => {
    h.content.style.minHeight = `calc(100% + ${Math.ceil(h.top)}px)`;
  };

  function capture(pane: HTMLElement | undefined): Hold | null {
    if (!pane) return null;
    if (hold?.pane !== pane) hold = { pane, content: pane.querySelector<HTMLElement>(contentSelector)!, top: 0 };
    hold.top = pane.scrollTop;
    floor(hold);
    return hold;
  }

  function follow(event: Event) {
    if (!hold || hold.pane !== event.currentTarget) return;
    hold.top = hold.pane.scrollTop;
    floor(hold);
  }

  function keep(pane: HTMLElement | undefined, current: () => boolean): () => void {
    const held = capture(pane);
    if (!held) return () => {};
    return () => {
      void tick().then(() => {
        if (current() && pane?.isConnected && hold === held) pane.scrollTop = held.top;
      });
    };
  }

  function release(pane: HTMLElement | undefined = hold?.pane) {
    if (!hold || hold.pane !== pane) return;
    hold.content.style.minHeight = "";
    hold = null;
  }

  return { capture, follow, keep, release };
}
