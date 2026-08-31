-- The strict engine stand-in for the mirror harness.
--
-- Every decision here is about being THE ENGINE rather than being convenient.
-- The recorded trap (FkLua's agents/testing.md) is a harness more forgiving
-- than the thing it stands in for: a guest emitting a prototype with no name
-- would pass a lenient stand-in and fail in the game, which turns a green gate
-- into a bug report from a player.
--
--   * data:extend VALIDATES: a table, entries that are tables, a string type
--     and a string name, error(..., 0) otherwise.
--   * data.raw is PREPOPULATED in the shape the real prototypes have, with a
--     technology whose unit.ingredients is an array of arrays, so a CostOf
--     copy of it proves something about a deep structure rather than a flat
--     one.
--   * defines.prototypes.item carries EXACTLY the 21 keys the engine has
--     (measured, Factorio 2.0.77 build 84539), including "item" itself, so the
--     library's item-first probe and its derived walk are both exercised
--     through the packaged module.
--   * The canonical serialiser sorts keys and prints numbers at %.17g, so the
--     transcript is a function of the VALUES and never of a table's iteration
--     order or a float's default formatting.
--
-- Usage: lua52f standin.lua <packaged-mod-dir>

local moddir = arg[1] or error("standin.lua needs the packaged mod directory", 0)
package.path = moddir .. "/?.lua"

function log(s) print("LOG " .. s) end

mods = { base = "2.0.77", ["fkrecipes-example"] = "0.1.0" }
feature_flags = { space_travel = false, quality = true }

-- defines.prototypes in the engine's own base -> {derived -> 0} shape. The
-- item list is the measured 21, in the engine's own set (the order a guest
-- sees is fkdata's sort, not this table's).
defines = { prototypes = {
  item = {
    ammo = 0, armor = 0, blueprint = 0, ["blueprint-book"] = 0, capsule = 0,
    ["copy-paste-tool"] = 0, ["deconstruction-item"] = 0, gun = 0, item = 0,
    ["item-with-entity-data"] = 0, ["item-with-inventory"] = 0,
    ["item-with-label"] = 0, ["item-with-tags"] = 0, module = 0,
    ["rail-planner"] = 0, ["repair-tool"] = 0, ["selection-tool"] = 0,
    ["space-platform-starter-pack"] = 0, ["spidertron-remote"] = 0, tool = 0,
    ["upgrade-item"] = 0,
  },
  entity = { ["transport-belt"] = 0, furnace = 0, container = 0 },
  technology = { technology = 0 },
  recipe = { recipe = 0 },
} }

-- The canonical serialiser. Sorted keys, numbers before strings, %.17g: the
-- transcript has to be a function of the values, never of a table's iteration
-- order.
local function ser(v)
  local t = type(v)
  if t == "number" then return string.format("%.17g", v) end
  if t == "string" then return string.format("%q", v) end
  if t ~= "table" then return tostring(v) end
  local ks = {}
  for k in pairs(v) do ks[#ks+1] = k end
  table.sort(ks, function(a, b)
    local ra = type(a) == "number" and 1 or 2
    local rb = type(b) == "number" and 1 or 2
    if ra ~= rb then return ra < rb end
    return a < b
  end)
  local parts = {}
  for _, k in ipairs(ks) do
    parts[#parts+1] = ser(k) .. "=" .. ser(v[k])
  end
  return "{" .. table.concat(parts, ",") .. "}"
end

data = { raw = {} }
local nextend = 0
function data:extend(list)
  nextend = nextend + 1
  if type(list) ~= "table" then error("data:extend takes a table", 0) end
  print("TRANSCRIPT extend#" .. nextend .. " " .. ser(list))
  for i, p in ipairs(list) do
    if type(p) ~= "table" then error("data:extend entry " .. i .. " is not a table", 0) end
    if type(p.type) ~= "string" then error("data:extend entry " .. i .. " has no type", 0) end
    if type(p.name) ~= "string" then error("data:extend entry " .. i .. " has no name", 0) end
    -- TWO LEGALITY NUMBERS, both MEASURED rather than invented. The
    -- energy_required floor and its message are this repository's own probe
    -- (Factorio 2.0.77 build 84539): 0 and -1 refuse the load, 0.0011 and
    -- 0.002 do not. unit.count = 0 being refused is FkLua's measured M6b; the
    -- wording below is engine-SHAPED rather than quoted, because that one's
    -- exact string was not captured. A regression in either would otherwise
    -- ship a mod that only fails on a player's machine.
    if p.type == "recipe" and p.energy_required ~= nil and p.energy_required <= 0.001 then
      error("Error while loading recipe prototype \"" .. p.name ..
            "\" (recipe): energy_required can't be <= 0.001", 0)
    end
    if p.type == "technology" and type(p.unit) == "table" and p.unit.count ~= nil and
       p.unit.count < 1 then
      error("Error while loading technology prototype \"" .. p.name ..
            "\" (technology): unit.count must be larger than 0", 0)
    end
    data.raw[p.type] = data.raw[p.type] or {}
    data.raw[p.type][p.name] = p
  end
end

-- Base's own prototypes. The technologies carry real-ish units in the short
-- tuple form, and the prerequisite chain steel-processing -> logistics-2 ->
-- logistics-3 is what the example's InsertBetween splices into.
data:extend{
  { type = "item", name = "iron-plate", icon = "__base__/iron-plate.png",
    icon_size = 64, stack_size = 100 },
  { type = "item", name = "steel-plate", icon = "__base__/steel-plate.png",
    icon_size = 64, stack_size = 100 },
  -- A science pack is a TOOL, not an item: the library's ItemExists has to
  -- walk the derived types to find it, which is the walk this row exists for.
  { type = "tool", name = "automation-science-pack",
    icon = "__base__/automation-science-pack.png", icon_size = 64,
    stack_size = 200, durability = 1 },

  { type = "technology", name = "steel-processing",
    icon = "__base__/steel-processing.png", icon_size = 128,
    unit = { count = 50, time = 5,
             ingredients = { { "automation-science-pack", 1 } } } },
  { type = "technology", name = "logistics-2",
    icon = "__base__/logistics-2.png", icon_size = 128,
    prerequisites = { "steel-processing" },
    unit = { count = 200, time = 30,
             ingredients = { { "automation-science-pack", 1 },
                             { "logistic-science-pack", 1 } } } },
  { type = "technology", name = "logistics-3",
    icon = "__base__/logistics-3.png", icon_size = 128,
    prerequisites = { "logistics-2" },
    unit = { count = 400, time = 60,
             ingredients = { { "automation-science-pack", 1 },
                             { "logistic-science-pack", 1 },
                             { "chemical-science-pack", 1 } } } },
  -- A multi-level technology: its cost is a FORMULA rather than a count, and
  -- its cap lives on the technology beside the unit rather than inside it. A
  -- CostOf copy has to bring both across.
  { type = "technology", name = "physical-projectile-damage-7",
    icon = "__base__/physical-projectile-damage-7.png", icon_size = 128,
    prerequisites = { "steel-processing" },
    max_level = "infinite",
    unit = { count_formula = "2^(L-7)*1000", time = 60,
             ingredients = { { "automation-science-pack", 1 },
                             { "logistic-science-pack", 1 } } } },
  -- A military-shaped technology: the other end of the example's cost ladder.
  -- It prices itself in a pack this stand-in carries no item row for, which is
  -- legal here for the same reason it is in the game: a copied unit is copied
  -- verbatim, not rebuilt out of the fields the planner knows.
  { type = "technology", name = "military-4",
    icon = "__base__/military-4.png", icon_size = 128,
    prerequisites = { "steel-processing" },
    unit = { count = 250, time = 30,
             ingredients = { { "automation-science-pack", 1 },
                             { "logistic-science-pack", 1 },
                             { "military-science-pack", 1 } } } },
  -- A research_trigger technology: no unit at all, the measured crash class
  -- CostOf refuses. Present so the packaged module meets one.
  { type = "technology", name = "steam-power",
    icon = "__base__/steam-power.png", icon_size = 128,
    research_trigger = { type = "craft-item", item = "offshore-pump" } },
}

print("--- SETTINGS ---")
-- `settings` DOES NOT EXIST at the settings stage: a mod's own startup
-- settings are not readable while they are being declared.
settings = nil
require("settings")

print("--- DATA ---")
-- What the player chose. hardened-tools is OFF, so the generated technology
-- has to come out hidden rather than absent, which is the design decision this
-- golden pins. rivet-batch is left unset, which is a setting nothing reads.
settings = { startup = {
  ["fkrecipes-example-hardened-tools"] = { value = false },
  ["fkrecipes-example-forging-time"] = { value = 7.5 },
  ["fkrecipes-example-quench-medium"] = { value = "oil" },
  -- Left ON, so the technology it gates comes out enabled with no hidden
  -- field at all: the other side of the switched-off branch above.
  ["fkrecipes-example-bonus-research"] = { value = true },
  -- The MILITARY ladder, which the game's own default does not take: the
  -- in-game gate runs on declared defaults and walks the projectile ladder
  -- instead, so the two gates cover one branch each.
  ["fkrecipes-example-tips-research-tier"] = { value = "military" },
} }
require("data")

local types = {}
for t in pairs(data.raw) do types[#types+1] = t end
table.sort(types)
for _, t in ipairs(types) do
  local names = {}
  for n in pairs(data.raw[t]) do names[#names+1] = n end
  table.sort(names)
  print("RAW " .. t .. ": " .. table.concat(names, " "))
end
print("FINAL " .. ser(data.raw))
