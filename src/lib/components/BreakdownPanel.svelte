<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { BreakdownSection } from "$lib/engine.svelte";
  import { stripPobText } from "$lib/pobtext";
  import PobText from "./PobText.svelte";
  import { m } from "$lib/paraglide/messages";

  let { sections }: { sections: BreakdownSection[] } = $props();

  const rangeGuide = convertFileSrc("Assets/range_guide.png", "pobasset");
  const gameUi = convertFileSrc("Assets/game_ui_small.png", "pobasset");

  type TableSection = Extract<BreakdownSection, { type: "table" }>;

  function visibleColumns(section: TableSection) {
    return section.cols.filter((column) => section.rows.some((row) => stripPobText(row[column.key] ?? "").trim().length > 0));
  }

  function cellWraps(value: string) {
    const text = stripPobText(value).trim();
    return text.length > 32 || (text.length > 22 && text.includes(" "));
  }

  function plainLine(line: string) {
    return stripPobText(line).trim();
  }

  function isFormulaLine(line: string) {
    const text = plainLine(line);
    return /^[=×x÷+]/.test(text) || /\s(?:=|×|x|÷)\s/.test(text);
  }

  function isNoteLine(line: string) {
    const text = plainLine(line);
    return /^\(.+\)$/.test(text) || /^note:/i.test(text);
  }

  function radiusPath(radius: number) {
    if (!Number.isFinite(radius) || radius <= 0) return "";
    const points: string[] = [];
    const cos45 = Math.cos(Math.PI / 4);
    const cos35 = Math.cos(Math.PI * 0.195);
    const sin35 = Math.sin(Math.PI * 0.195);
    for (let degrees = 0; degrees <= 360; degrees += 2) {
      const angle = degrees / 180 * Math.PI;
      const x = Math.sin(angle) * radius;
      const y = Math.cos(angle) * radius;
      const cameraX = (x - y) * cos45;
      const cameraY = -5.33 - (y + x) * cos45 * cos35;
      const cameraZ = 122 + (y + x) * cos45 * sin35;
      const screenX = 240 + cameraX / cameraZ * 1.27 * 270;
      const screenY = 135 + cameraY / cameraZ * 1.27 * 270;
      points.push(`${degrees === 0 ? "M" : "L"}${screenX.toFixed(1)} ${screenY.toFixed(1)}`);
    }
    return `${points.join(" ")} Z`;
  }
</script>

<div class="bd">
  {#each sections as s}
    {#if s.type === "text"}
      <div class="txt" class:big={s.size >= 16}>
        {#each s.lines as line, i}
          <div
            class="tl"
            class:lead={i === 0 && s.lines.length > 1}
            class:formula={isFormulaLine(line)}
            class:note={isNoteLine(line)}
          ><PobText text={line} calcs /></div>
        {/each}
      </div>
    {:else if s.type === "table"}
      {@const columns = visibleColumns(s)}
      <div class="tbl">
        {#if s.label}<div class="tlabel"><PobText text={s.label} calcs /></div>{/if}
        <div class="table-scroll">
          <table>
            <thead>
              <tr>
                {#each columns as c}
                  <th class:r={c.right} scope="col"><PobText text={c.label} calcs /></th>
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each s.rows as row}
                <tr>
                  {#each columns as c}
                    <td class:r={c.right} class:wrap={cellWraps(row[c.key] ?? "")}><PobText text={row[c.key] ?? ""} calcs /></td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        {#if s.footer}<div class="tfoot"><PobText text={s.footer} calcs /></div>{/if}
      </div>
    {:else if s.type === "radius"}
      <figure class="radius">
        <div class="radius-visual">
          <img class="range-guide" src={rangeGuide} alt="" />
          <svg viewBox="0 0 480 270" aria-hidden="true">
            <path d={radiusPath(s.radius)} />
          </svg>
          <img class="game-ui" src={gameUi} alt="" />
        </div>
        <figcaption>{m.breakdown_radius({ radius: s.radius })}</figcaption>
      </figure>
    {/if}
  {/each}
  {#if sections.length === 0}
    <div class="dim small">{m.breakdown_none()}</div>
  {/if}
</div>

<style>
  .bd {
    display: flex;
    flex-direction: column;
    gap: 16px;
    font-size: var(--fs-xs);
    line-height: 1.5;
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 8px 10px;
    border: 1px solid var(--line-0);
    border-radius: var(--r-1);
    background: var(--bg-2);
  }
  .tl {
    color: var(--fg-1);
    white-space: pre-wrap;
  }
  .txt.big .tl:not(.note) {
    font-size: var(--fs-sm);
  }
  .tl.lead {
    margin-bottom: 3px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--line-1);
    color: var(--fg-0);
    font-weight: 700;
  }
  .tl.formula {
    padding-left: 8px;
    color: var(--fg-0);
    font-family: var(--font-mono);
  }
  .tl.note {
    margin-top: 3px;
    color: var(--fg-2);
    font-size: var(--fs-2xs);
    font-style: italic;
  }
  .tbl {
    margin-top: 3px;
  }
  .tlabel {
    margin: 0 2px 7px;
    color: var(--fg-0);
    font-size: var(--fs-sm);
    font-weight: 700;
  }
  .table-scroll {
    overflow-x: auto;
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    background: var(--bg-1);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
  }
  th {
    padding: 7px 9px;
    border-right: 1px solid var(--line-1);
    border-bottom: 1px solid var(--line-2);
    background: var(--bg-3);
    color: var(--fg-0);
    font-family: var(--font-ui);
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    text-align: left;
    white-space: nowrap;
  }
  td {
    padding: 6px 9px;
    border-top: 1px solid var(--line-0);
    border-right: 1px solid var(--line-0);
    background: var(--bg-1);
    color: var(--fg-1);
    white-space: nowrap;
    vertical-align: top;
  }
  td.wrap {
    min-width: 140px;
    max-width: 300px;
    white-space: normal;
    overflow-wrap: anywhere;
    line-height: 1.4;
  }
  tbody tr:first-child td {
    border-top: 0;
  }
  tbody tr:nth-child(even) td {
    background: var(--bg-2);
  }
  tbody tr:hover td {
    background: var(--bg-hover);
  }
  th:last-child,
  td:last-child {
    border-right: 0;
  }
  .r {
    text-align: right;
  }
  .tfoot {
    margin-top: 7px;
    padding: 7px 9px;
    border-left: 2px solid var(--line-2);
    background: var(--bg-2);
    color: var(--fg-2);
    white-space: pre-wrap;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .radius {
    margin: 0;
  }
  .radius-visual {
    position: relative;
    width: min(480px, 100%);
    aspect-ratio: 16 / 9;
    overflow: hidden;
    border: 1px solid var(--line-2);
    background: var(--bg-0);
  }
  .radius-visual img,
  .radius-visual svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .range-guide {
    filter: brightness(0.75);
  }
  .radius-visual svg path {
    fill: var(--focus);
    fill-opacity: 0.33;
  }
  .game-ui {
    pointer-events: none;
  }
  .radius figcaption {
    margin-top: 6px;
    color: var(--fg-1);
    font-size: var(--fs-xs);
    font-weight: 600;
  }
</style>
