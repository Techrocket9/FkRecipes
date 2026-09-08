-- The strict engine stand-in for the mirror harness.
--
-- Every decision here is about being THE ENGINE rather than being convenient.
-- The recorded trap (FkLua's agents/testing.md) is a harness more forgiving
-- than the thing it stands in for: a guest emitting a prototype with no name
-- would pass a lenient stand-in and fail in the game, which turns a green gate
-- into a bug report from a player.
--
--   * data:extend VALIDATES: a table, entries that are tables, a string type
--     and a string name, error(..., 0) otherwise; and then the MEASURED
--     prototype rules a customizable recipe can now break, because a player
--     types the ingredient list and the library is what stands between a typo
--     and a load failure. Every message below is quoted from this repository's
--     own probe of Factorio 2.0.77 (agents/customizer-design.md's measured
--     table) except where a comment says it is engine-SHAPED.
--   * data.raw is PREPOPULATED in the shape the real prototypes have, with a
--     technology whose unit.ingredients is an array of arrays, so a CostOf
--     copy of it proves something about a deep structure rather than a flat
--     one.
--   * defines.prototypes.item carries EXACTLY the 21 keys the engine has
--     (measured, Factorio 2.0.77 build 84539), including "item" itself, so the
--     library's item-first probe and its derived walk are both exercised
--     through the packaged module.
--   * The canonical serialiser sorts keys, prints numbers at %.17g and keeps
--     every record on ONE LINE, so the transcript is a function of the VALUES
--     and never of a table's iteration order or a float's default formatting,
--     and so a line-oriented reader (diff, grep, the golden) sees one record
--     per line.
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
--
-- THE NEWLINE IS ESCAPED RATHER THAN WRITTEN, and that is not cosmetic. Lua's
-- own %q writes a newline as a backslash followed by a REAL newline, which is
-- valid Lua source and is exactly wrong here: the composed setting
-- descriptions this library emits carry "\n" between their presets, so one
-- record would land as four lines and every line-oriented reader downstream
-- (the golden diff, this script's own greps, a human) would be reading half a
-- record. Nothing else about %q's escaping changes.
local function serstr(s)
  return (string.format("%q", s):gsub("\\\n", "\\n"))
end

local function ser(v)
  local t = type(v)
  if t == "number" then return string.format("%.17g", v) end
  if t == "string" then return serstr(v) end
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

-- ---------------------------------------------------------------------------
-- The prototype rules, MEASURED. Each one is a load failure a customizable
-- recipe can now reach, because the ingredient list is a text a player types
-- and the library is the only thing between a typo and a broken game. A
-- regression in either half has to be a REFUSAL in the transcript rather than
-- a green line carrying a prototype the engine would have thrown out.
-- ---------------------------------------------------------------------------

-- is_item walks the derived item types exactly as the engine's own item lookup
-- does, because a science pack is a `tool` row and a plate is an `item` row and
-- both are items to a recipe.
local function is_item(name)
  for t in pairs(defines.prototypes.item) do
    if data.raw[t] and data.raw[t][name] then return true end
  end
  return false
end

local function is_tool(name)
  return data.raw.tool ~= nil and data.raw.tool[name] ~= nil
end

local function is_whole(n)
  return n == math.floor(n)
end

local function check_recipe(p)
  -- MEASURED: the rule keys on the category NAME and not on hand-craftability,
  -- and an absent category IS `crafting`.
  local category = p.category or "crafting"
  local seen = {}
  for i, ing in ipairs(p.ingredients or {}) do
    if type(ing) ~= "table" or type(ing.name) ~= "string" then
      error("the stand-in: recipe \"" .. p.name .. "\" ingredient " .. i ..
            " is not a named ingredient table", 0)
    end
    local kind = ing.type or "item"
    local key = kind .. "/" .. ing.name
    if seen[key] then
      -- MEASURED for two items. The fluid pair is engine-SHAPED: the probe
      -- named one item twice, so only that wording was captured.
      error("Duplicate item ingredients are not allowed (" .. ing.name ..
            " exists 2 or more times).", 0)
    end
    seen[key] = true
    if type(ing.amount) ~= "number" then
      error("the stand-in: recipe \"" .. p.name .. "\" ingredient " ..
            ing.name .. " has no numeric amount", 0)
    end
    if kind == "fluid" then
      if category == "crafting" then
        error("Recipe is in 'crafting' category but has a non-item ingredient '" ..
              ing.name .. "' (fluid).", 0)
      end
      if ing.amount <= 0 then
        error("amount must be larger than 0", 0)
      end
    else
      -- THE FRACTION IS THE LIBRARY'S RULE AND NOT THE ENGINE'S, so it gets
      -- the stand-in's own voice: MEASURED, an item amount of 1.5 LOADS and is
      -- dumped as 1.5. The library refuses it because half an item is not a
      -- thing to ship a player, and a half that leaked through would be a
      -- silent behaviour change rather than a load failure, which is precisely
      -- what a stand-in stricter than the engine is for.
      if not is_whole(ing.amount) then
        error("the stand-in: recipe \"" .. p.name .. "\" gives the item " ..
              ing.name .. " the fractional amount " ..
              string.format("%.17g", ing.amount) ..
              "; the engine loads it and this library refuses it", 0)
      end
      if ing.amount == 0 then
        error("Item ingredient can't have count of 0.", 0)
      end
      if ing.amount > 65535 or ing.amount < 0 then
        error("Value (" .. string.format("%.17g", ing.amount) ..
              ") outside of range. The data type allows values from 0 to 65535", 0)
      end
    end
  end
end

local function check_technology(p)
  if type(p.unit) ~= "table" then return end
  if p.unit.count ~= nil and p.unit.count < 1 then
    -- FkLua's measured M6b; the wording is engine-SHAPED, because that one's
    -- exact string was not captured.
    error("Error while loading technology prototype \"" .. p.name ..
          "\" (technology): unit.count must be larger than 0", 0)
  end
  if p.unit.time ~= nil and p.unit.time <= 0 then
    error("time must be positive.", 0)
  end
  for i, ing in ipairs(p.unit.ingredients or {}) do
    -- Both spellings: the short tuple form the library emits and the engine's
    -- own long form, so a stand-in rule cannot be dodged by a shape change.
    local name, amount = ing[1], ing[2]
    if type(ing) == "table" and type(ing.name) == "string" then
      name, amount = ing.name, ing.amount
    end
    if type(name) ~= "string" then
      error("the stand-in: technology \"" .. p.name .. "\" unit ingredient " ..
            i .. " names nothing", 0)
    end
    if not is_item(name) then
      error("Error in assignID: item with name '" .. name .. "' does not exist.", 0)
    end
    if not is_tool(name) then
      error("Invalid research unit (" .. name ..
            "). Research unit(s) can only be tool type items at the moment.", 0)
    end
    if amount == 0 then
      error("ResearchIngredient's amount must not be 0", 0)
    end
  end
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
    -- THE LEGALITY NUMBERS, all MEASURED rather than invented. The
    -- energy_required floor and its message are this repository's own probe
    -- (Factorio 2.0.77 build 84539): 0 and -1 refuse the load, 0.0011 and
    -- 0.002 do not. The recipe and technology rules are check_recipe and
    -- check_technology above. A regression in any of them would otherwise ship
    -- a mod that only fails on a player's machine.
    if p.type == "recipe" then
      if p.energy_required ~= nil and p.energy_required <= 0.001 then
        error("Error while loading recipe prototype \"" .. p.name ..
              "\" (recipe): energy_required can't be <= 0.001", 0)
      end
      check_recipe(p)
    end
    if p.type == "technology" then check_technology(p) end
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
  -- An intermediate a player can plausibly type into an ingredient field, and
  -- the one the mirror's flipped rivet list names in its rich-text tag.
  { type = "item", name = "iron-stick", icon = "__base__/iron-stick.png",
    icon_size = 64, stack_size = 100 },
  -- A science pack is a TOOL, not an item: the library's ItemExists has to
  -- walk the derived types to find it, which is the walk this row exists for,
  -- and a pack list asks ToolExists, which reads this table by name.
  --
  -- ALL FOUR PACKS THE BASE TECHNOLOGIES BELOW PRICE THEMSELVES IN. The
  -- stand-in now enforces the engine's measured research-unit rule (a unit
  -- ingredient is a tool or the load fails), so a base row naming a pack with
  -- no table entry would be an UNFAITHFUL stand-in rather than a deliberate
  -- gap: the engine has no such technology either.
  { type = "tool", name = "automation-science-pack",
    icon = "__base__/automation-science-pack.png", icon_size = 64,
    stack_size = 200, durability = 1 },
  { type = "tool", name = "logistic-science-pack",
    icon = "__base__/logistic-science-pack.png", icon_size = 64,
    stack_size = 200, durability = 1 },
  { type = "tool", name = "military-science-pack",
    icon = "__base__/military-science-pack.png", icon_size = 64,
    stack_size = 200, durability = 1 },
  { type = "tool", name = "chemical-science-pack",
    icon = "__base__/chemical-science-pack.png", icon_size = 64,
    stack_size = 200, durability = 1 },

  -- THE FLUIDS, because a player can now type one. An untagged name resolves
  -- as an item first and only then as a fluid, so water being here and NOT in
  -- data.raw.item is what makes "0.5 water" a fluid rather than a miss; steam
  -- is the second row, so a fluid lookup is a lookup rather than a table with
  -- one entry that any bug would still find.
  { type = "fluid", name = "water", icon = "__base__/water.png",
    icon_size = 64, default_temperature = 15, base_color = { b = 1 },
    flow_color = { b = 1 }, max_temperature = 100 },
  { type = "fluid", name = "steam", icon = "__base__/steam.png",
    icon_size = 64, default_temperature = 15, base_color = { r = 1, g = 1, b = 1 },
    flow_color = { r = 1, g = 1, b = 1 }, max_temperature = 1000 },

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
  -- A military-shaped technology: the other end of the example's cost ladder,
  -- and the one whose whole unit a CostOf copy brings across verbatim rather
  -- than rebuilding out of the fields the planner knows.
  { type = "technology", name = "military-4",
    icon = "__base__/military-4.png", icon_size = 128,
    prerequisites = { "steel-processing" },
    unit = { count = 250, time = 30,
             ingredients = { { "automation-science-pack", 1 },
                             { "logistic-science-pack", 1 },
                             { "military-science-pack", 1 } } } },
  -- THE CUSTOM ARM'S PREREQUISITE LADDER. hardened-tips carries the ladder
  -- military-2 then military under its Custom cost, because an arm with no
  -- source technology has nowhere else to take a position from. This mirror
  -- leaves that dropdown on the military PRESET, so the ladder is not walked
  -- here (the flipped in-game row walks it); the rungs are present so the
  -- stand-in can answer the question rather than answering "absent" by
  -- accident the day the mirror does flip it.
  { type = "technology", name = "military",
    icon = "__base__/military.png", icon_size = 128,
    unit = { count = 10, time = 15,
             ingredients = { { "automation-science-pack", 1 } } } },
  { type = "technology", name = "military-2",
    icon = "__base__/military-2.png", icon_size = 128,
    prerequisites = { "military", "steel-processing" },
    unit = { count = 20, time = 30,
             ingredients = { { "automation-science-pack", 1 },
                             { "logistic-science-pack", 1 } } } },
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
--
-- EVERY TEXT HERE IS AN EDITED ONE, never the reserved word default: the
-- in-game gate's default row already covers what default means, and a text
-- equal to it takes the author's declared list and leaves no line behind. What
-- this table is for is the other side of every one of those branches.
settings = { startup = {
  ["fkrecipes-example-hardened-tools"] = { value = false },
  ["fkrecipes-example-forging-time"] = { value = 7.5 },
  ["fkrecipes-example-quench-medium"] = { value = "oil" },
  -- EDITED, AND IGNORED, and that pair is the point. The dropdown above is on
  -- a preset, so this text is not live; the library says so in the log rather
  -- than letting a player edit a field and watch nothing happen. The flipped
  -- in-game row puts the same dropdown on custom and reads the same setting
  -- for real, so between the two gates both sides of the arm are covered.
  ["fkrecipes-example-quench-ingredients"] = { value = "2 steel-plate, 6 iron-stick" },
  -- Left ON, so the technology it gates comes out enabled with no hidden
  -- field at all: the other side of the switched-off branch above.
  ["fkrecipes-example-bonus-research"] = { value = true },
  -- The MILITARY ladder, which the game's own default does not take: the
  -- in-game gate runs on declared defaults and walks the projectile ladder
  -- instead, so the two gates cover one branch each.
  ["fkrecipes-example-tips-research-tier"] = { value = "military" },
  -- A NUMBER MOVED UNDER A TIER. tips-research-tier stays on military, so
  -- the custom cost is not live; the count is edited (its declared default is
  -- 30) and the seconds and the pack text are left alone, so the transcript
  -- carries exactly one ignored-number line and nothing for the other two.
  ["fkrecipes-example-tips-count"] = { value = 45 },
  -- Above the declared minimum of 0.5, so it is the value the player chose
  -- that reaches the recipe rather than any bound.
  ["fkrecipes-example-tempering-hold"] = { value = 4 },
  -- THE WHOLE LIST OF ONE RECIPE, written by the player into a setting with no
  -- dropdown in front of it, in two of the forms the language accepts that a
  -- mod author would never write: a glued sign and a rich-text tag. The
  -- canonical rendering in the log line is what says both were understood.
  ["fkrecipes-example-rivet-ingredients"] = { value = "3 steel-plate, [item=iron-stick] x2" },
  -- THE CUSTOM ARM OF A DROPDOWN, with a FLUID and a FRACTION in it. The chain
  -- recipe is crafting-with-fluid, which is what makes the fluid legal
  -- (measured: the crafting category refuses one), and the fraction is what
  -- says a fluid amount is a double all the way through rather than an item
  -- count wearing a decimal point.
  --
  -- IT NAMES NO ITEM OF THIS MOD'S OWN, and that is not a preference. This
  -- setting's own description offers the player "4 fkrecipes-example-steel-
  -- rivet" to copy, and typing that back REFUSES the load with "no item or
  -- fluid is named fkrecipes-example-steel-rivet": the text is resolved before
  -- the plan's own items are extended, so ItemExists cannot see them. That is
  -- a library defect rather than a harness one, and the mirror stays clear of
  -- it so no golden freezes it in.
  ["fkrecipes-example-chain-links"] = { value = "custom" },
  ["fkrecipes-example-chain-ingredients"] = { value = "2 steel-plate, 0.5 water" },
  -- A RESEARCH COST THE PLAYER PRICED: two packs in the text, and the count
  -- and the seconds from their own numeric settings, one of them fractional so
  -- the seconds are not an integer that any formatter would agree on.
  ["fkrecipes-example-chain-packs"] = { value = "2 automation-science-pack, 1 logistic-science-pack" },
  ["fkrecipes-example-chain-count"] = { value = 25 },
  ["fkrecipes-example-chain-seconds"] = { value = 12.5 },
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
