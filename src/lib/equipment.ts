import type { ItemSocket, SlotInfo, SocketGroup } from "./engine.svelte";

export const EQUIPMENT_LAYOUT: Record<string, string> = {
  "Weapon 1": "1 / 1 / 5 / 3",
  "Weapon 2": "1 / 7 / 5 / 9",
  Helmet: "1 / 4 / 3 / 6",
  "Body Armour": "3 / 4 / 6 / 6",
  Amulet: "3 / 6 / 4 / 7",
  "Ring 1": "4 / 3 / 5 / 4",
  "Ring 2": "4 / 6 / 5 / 7",
  "Ring 3": "3 / 3 / 4 / 4",
  Gloves: "5 / 2 / 7 / 4",
  Boots: "5 / 6 / 7 / 8",
  Belt: "6 / 4 / 7 / 6",
};

export const WICKERMAN_AREAS = ["1 / 4 / 3 / 6", "3 / 4 / 5 / 6", "3 / 2 / 5 / 4", "5 / 4 / 7 / 6"];

export const isWickermanSlot = (slot: SlotInfo) => slot.group === "wickerman";

export function equipmentArea(slot: string): string | undefined {
  return EQUIPMENT_LAYOUT[slot.replace(/ Swap$/, "")];
}

// Plain bases with art in poe-art's maps for each game, drawn faintly in empty slots.
const SLOT_ART: Record<"poe1" | "poe2", Record<string, string>> = {
  poe1: {
    "Weapon 1": "Copper Sword",
    "Weapon 2": "Plank Kite Shield",
    Helmet: "Iron Hat",
    "Body Armour": "Plate Vest",
    Gloves: "Iron Gauntlets",
    Boots: "Iron Greaves",
    Belt: "Leather Belt",
    Amulet: "Jade Amulet",
    Ring: "Iron Ring",
    "Flask 1": "Small Life Flask",
    "Flask 2": "Small Mana Flask",
    Flask: "Quicksilver Flask",
  },
  poe2: {
    "Weapon 1": "Broadsword",
    "Weapon 2": "Splintered Tower Shield",
    Helmet: "Rusted Greathelm",
    "Body Armour": "Rusted Cuirass",
    Gloves: "Stocky Mitts",
    Boots: "Iron Greaves",
    Belt: "Heavy Belt",
    Amulet: "Jade Amulet",
    Ring: "Iron Ring",
    "Flask 1": "Lesser Life Flask",
    "Flask 2": "Lesser Mana Flask",
    Charm: "Thawing Charm",
  },
};

export function slotArtBase(game: "poe1" | "poe2", slot: string): string | null {
  const table = SLOT_ART[game];
  const key = slot.replace(/ Swap$/, "");
  return table[key] ?? table[key.replace(/ \d+$/, "")] ?? null;
}

export function visibleEquipment(slots: SlotInfo[]): SlotInfo[] {
  return slots.filter((slot) => slot.shown && !slot.inactive && slot.nodeId == null);
}

export function assignedGems(slot: string, groups: SocketGroup[]) {
  return groups.filter((group) => group.slot === slot && !group.source && !group.grantedBy)
    .flatMap((group) => group.gems.filter((gem) => !gem.granted && (gem.name || gem.nameSpec))
      .map((gem) => ({ gem, groupIndex: group.index, enabled: group.enabled && gem.enabled })));
}

type AssignedGem = ReturnType<typeof assignedGems>[number];

export function socketedGems(slot: string, sockets: ItemSocket[], groups: SocketGroup[]): (AssignedGem | null)[] {
  const entries = assignedGems(slot, groups);
  const linkedGroups = [...new Set(entries.map((entry) => entry.groupIndex))]
    .map((id) => entries.filter((entry) => entry.groupIndex === id))
    .sort((a, b) => Number(b[0].enabled) - Number(a[0].enabled) || b.length - a.length);
  const components = [...new Set(sockets.map((socket) => socket.group))]
    .map((group) => sockets.flatMap((socket, index) => socket.group === group && socket.colour !== "A" ? [index] : []));
  const fits = (entry: AssignedGem, index: number) => {
    const colour = entry.gem.socketColour;
    return !colour || colour === "W" || sockets[index].colour === "W" || sockets[index].colour === colour;
  };
  type Placement = { index: number; entry: AssignedGem };
  const memo = new Map<string, Placement[]>();
  function place(groupIndex: number, used: Set<number>): Placement[] {
    if (groupIndex >= linkedGroups.length || used.size >= sockets.length) return [];
    const key = `${groupIndex}:${[...used].sort((a, b) => a - b).join(",")}`;
    const cached = memo.get(key);
    if (cached) return cached;
    const group = linkedGroups[groupIndex];
    let best: Placement[] = [];
    for (const component of components) {
      const free = component.filter((index) => !used.has(index));
      if (free.length < group.length) continue;
      function match(gemIndex: number, remaining: number[], placements: Placement[]) {
        if (gemIndex === group.length) {
          const nextUsed = new Set([...used, ...placements.map((p) => p.index)]);
          const candidate = [...placements, ...place(groupIndex + 1, nextUsed)];
          if (candidate.length > best.length) best = candidate;
          return;
        }
        for (const index of remaining) {
          if (fits(group[gemIndex], index)) {
            match(gemIndex + 1, remaining.filter((i) => i !== index), [...placements, { index, entry: group[gemIndex] }]);
          }
        }
      }
      match(0, free, []);
    }
    const skipped = place(groupIndex + 1, used);
    if (skipped.length > best.length) best = skipped;
    memo.set(key, best);
    return best;
  }
  const result: (AssignedGem | null)[] = sockets.map(() => null);
  for (const { index, entry } of place(0, new Set())) result[index] = entry;
  return result;
}

// Socket order snakes down each pair of rows, as in the game inventory.
export function socketPosition(index: number, count = 6) {
  const row = Math.floor(index / 2);
  return { x: count === 1 ? 24 : 12 + (row % 2 ? 1 - index % 2 : index % 2) * 24, y: 12 + row * 24 };
}
