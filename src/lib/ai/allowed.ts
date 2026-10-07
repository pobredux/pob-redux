/**
 * The subset of the MCP tools the chat panel may use. The full set costs
 * ~4,700 tokens on every turn and the user pays for it, so the panel gets the
 * ones that answer questions plus the reversible edits. Build lifecycle and
 * every `delete_*` stay out: they have no undo.
 *
 * External MCP clients still see every tool.
 */
export const ALLOWED = new Set([
  // read
  "get_character",
  "get_stats",
  "get_sidebar",
  "list_stat_keys",
  "sanity_check",
  "build_summary",
  "library",
  "checkpoint",
  "tree_suggest",
  "list_power_stats",
  "get_tree_state",
  "search_tree",
  "node_info",
  "node_path_cost",
  "path_plan",
  "get_items",
  "list_items",
  "get_skills",
  "skill_info",
  "get_config",
  "list_config_options",
  "get_notes",
  // Gems and gear cannot be chosen without looking them up first. add_gem's own
  // description points at list_gems, and equipping a real unique needs the
  // database rather than invented item text.
  "list_gems",
  "list_valid_supports",
  "search_item_db",
  // Scores every unique jewel against the build; the one call that answers
  // "which jewel", where equipping them one by one would take a dozen.
  "suggest_unique_jewels",
  "suggest_cluster_jewels",
  "list_timeless_options",
  "search_timeless_seeds",
  "list_bases",
  "list_affixes",
  // write, approval-gated
  "alloc_node",
  "alloc_path",
  "set_attribute_choice",
  "dealloc_node",
  "tree_undo",
  "set_level",
  "set_config",
  "set_main_skill",
  "add_socket_group",
  "set_socket_group",
  "add_gem",
  "set_gem",
  "remove_gem",
  "equip_item_raw",
  "equip_from_item_db",
  "set_item_variant",
  "craft_rare",
  "optimise_gear",
  "set_gem_levels",
  "unequip_item",
  "set_flask_active",
  "apply_cluster_jewel",
  "rollback",
  // Removing a whole skill setup is what "get rid of that skill" means, and
  // the approval gate plus checkpoint cover the missing undo.
  "remove_socket_group",
]);

/**
 * The tools loaded at the start of every conversation. The rest of ALLOWED is
 * reachable through `find_tools`, which adds what it finds for the rest of the
 * conversation.
 *
 * The whole set costs ~6,300 tokens of definitions. This is about a quarter of
 * that. The trade is one cache write per expansion against a smaller prompt on
 * every turn, which is a clear win on the first turn and for providers that do
 * not cache a prefix at all.
 *
 * tree_suggest, optimise_gear and suggest_unique_jewels are deliberately out:
 * their descriptions are the longest in the registry and a conversation that
 * never touches gear or jewels should not carry them.
 */
export const CORE = new Set([
  "get_character",
  "get_stats",
  "get_sidebar",
  "build_summary",
  "sanity_check",
  "library",
  "checkpoint",
  "rollback",
  "get_items",
  "get_skills",
  "get_config",
  "get_tree_state",
  "search_tree",
  "node_info",
  "path_plan",
  "alloc_path",
  "alloc_node",
  "dealloc_node",
  "set_level",
  "set_config",
]);
