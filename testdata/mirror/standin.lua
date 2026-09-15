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
-- THE STAND-IN MODELS BOTH MEASURED ENGINES, one per run, because the science
-- pack stopped being a prototype TYPE between them. The second argument picks
-- which, and 2.0 is the default so every existing caller and the committed
-- golden mean what they always meant:
--
--   * 2.0 (Factorio 2.0.77 build 84539): a science pack is a data.raw.tool
--     entry, and a research unit naming anything else refuses with "Invalid
--     research unit (<name>). Research unit(s) can only be tool type items at
--     the moment."
--   * 2.1 (Factorio 2.1.17 build 87315): there is NO data.raw.tool at all. A
--     science pack is a data.raw.item entry with subgroup "science-pack", and
--     the research unit's own gate is LAB COVERAGE: a unit naming something no
--     lab accepts refuses with "Technology <name>: there is no lab that will
--     accept all of the science packs this technology requires.\nScience
--     packs: <names>", measured for an item outside the subgroup AND for a
--     fresh item inside it that no lab lists.
--
--   * AND A RECIPE'S CATEGORY IS SPELLED DIFFERENTLY: `category` on 2.0,
--     `categories` (a list) on 2.1, where the old spelling is REFUSED outright
--     rather than ignored. check_recipe holds each arm to its own spelling and
--     refuses the other, so a guest that got it the wrong way round fails here
--     rather than in a player's game.
--
-- Both engine arms demote military-science-pack the same way, so the guest has
-- the same pack to drop on both and the prototypes it emits must come out
-- byte-identical APART FROM THAT ONE FIELD: run-mirror.sh compares exactly
-- that, normalising the category spelling and asserting it separately.
--
-- Usage: lua52f standin.lua <packaged-mod-dir> [2.0|2.1]

local moddir = arg[1] or error("standin.lua needs the packaged mod directory", 0)
package.path = moddir .. "/?.lua"

local engine = arg[2] or "2.0"
if engine ~= "2.0" and engine ~= "2.1" then
  error("standin.lua takes 2.0 or 2.1 as its engine, not " .. tostring(engine), 0)
end
local packs_are_items = (engine == "2.1")

function log(s) print("LOG " .. s) end

-- MEASURED VERSIONS, both of them: the guest keys its science-pack probe on
-- base's own version, so a stand-in that reported something else would be
-- modelling an engine nobody ran.
mods = { base = (packs_are_items and "2.1.17" or "2.0.77"),
         ["fkrecipes-example"] = "0.1.0" }
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

-- WHAT A LAB TAKES, which is the 2.1 engine's own research-unit gate (see the
-- header). It is a SET THAT CHANGES rather than a constant, for the same
-- reason the tool table is one: base declares military-4 in the pack's science
-- days and a later mod demotes the pack, so a technology that was legal when
-- it was written stops being legal, and this library's data stage runs after
-- both. The demotion below takes the pack out of here on the 2.1 arm exactly
-- as it takes it out of data.raw.tool on the 2.0 arm.
--
-- The 2.0 arm never reads it; is_tool is that arm's gate.
local lab_accepts = {
  ["automation-science-pack"] = true,
  ["logistic-science-pack"] = true,
  ["military-science-pack"] = true,
  ["chemical-science-pack"] = true,
}

local function is_whole(n)
  return n == math.floor(n)
end

local function check_recipe(p)
  -- THE CATEGORY IS SPELLED DIFFERENTLY ON THE TWO ENGINES, and the 2.1 one
  -- REFUSES THE OLD SPELLING OUTRIGHT rather than ignoring it. Measured on
  -- Factorio 2.1.17 build 87315: a recipe carrying `category` stops the load
  -- with the sentence below, the same recipe carrying
  -- `categories = {"crafting-with-fluid"}` loads with exit 0, and base's own
  -- sulfuric-acid reads `categories = {"chemistry"}`. So this is the one field
  -- whose EMITTED SPELLING the two engine arms must differ on, and the arm
  -- that gets the wrong one has to say so here rather than let a mod that
  -- cannot load anywhere read as green.
  local category
  if packs_are_items then
    if p.category ~= nil then
      error("Error while loading recipe prototype \"" .. p.name ..
            "\" (recipe): In RecipePrototype, `category` and " ..
            "`additional_categories` got merged into `categories` table. " ..
            "Please use that instead.", 0)
    end
    if p.categories ~= nil then
      if type(p.categories) ~= "table" or type(p.categories[1]) ~= "string" then
        error("the stand-in: recipe \"" .. p.name ..
              "\" has a categories field that is not a list of names", 0)
      end
      category = p.categories[1]
    end
  else
    if p.categories ~= nil then
      error("the stand-in: recipe \"" .. p.name ..
            "\" carries the 2.1 spelling `categories` on a 2.0 engine", 0)
    end
    category = p.category
  end
  -- MEASURED: the rule keys on the category NAME and not on hand-craftability,
  -- and an absent category IS `crafting`.
  category = category or "crafting"
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
    -- THE ENGINE'S OWN GATE, and it is a different sentence on each of the two
    -- measured engines. On 2.0 the unit ingredient has to be the right TYPE;
    -- on 2.1 the type is gone and what refuses is lab coverage, which this
    -- stand-in models with the lab_accepts set below. Both are quoted from a
    -- real run: see the header.
    if packs_are_items then
      if not lab_accepts[name] then
        error("Technology " .. p.name .. ": there is no lab that will accept " ..
              "all of the science packs this technology requires.\nScience packs: " ..
              name, 0)
      end
    elseif not is_tool(name) then
      error("Invalid research unit (" .. name ..
            "). Research unit(s) can only be tool type items at the moment.", 0)
    end
    if amount == 0 then
      error("ResearchIngredient's amount must not be 0", 0)
    end
  end
end

-- THE SETTING PROTOTYPES, and this table is a lookup rather than a suffix
-- match on purpose. It is the whole range of settingTypeName (go/settings.go)
-- and setting_type_name (rust/src/settings.rs), both of whose default arm is
-- string-setting, so four is all a guest of this library can reach. A match on
-- a trailing "-setting" would exempt by SPELLING, which means a prototype type
-- named that way later would stop being checked with nobody deciding it; this
-- table stops being right loudly instead, by policing a row nobody has
-- measured yet. The engine's own color-setting is deliberately absent for that
-- reason: this library cannot emit one.
local setting_types = {
  ["bool-setting"] = true,
  ["double-setting"] = true,
  ["int-setting"] = true,
  ["string-setting"] = true,
}

-- check_localised is the 200-BYTE-PER-ELEMENT rule, MEASURED on Factorio
-- 2.0.77 build 84539 with a throwaway probe mod hanging a localised_description
-- on base's own prototypes: an element of 200 bytes loads, 201 refuses the load
-- with exit 1 and no dump. BYTES rather than characters (101 e-acutes is 101
-- characters and 202 bytes and refuses reporting 202), and NO AGGREGATE BUDGET
-- at all, sixteen elements of which fifteen were 199 bytes loading clean. Lua's
-- # on a string is its byte count, which is the number the engine compares.
--
-- THE INDEX THE MESSAGE CARRIES IS 0-BASED over the elements while Lua's are
-- 1-based, so the key at t[1] is the engine's [0] and the first parameter at
-- t[2] is its [1]. A nested localised string is a parameter and a localised
-- string in its own right, so its elements append a second bracket.
--
-- A SETTING PROTOTYPE IS NOT SUBJECT TO THE RULE AT ALL, measured on the same
-- binary: a string-setting whose localised_description and localised_name each
-- held 201, 400, 1000, 2000 and 5000-byte elements exits 0 every time, with no
-- message of any kind and every byte reaching mod-settings-dump.json. Setting
-- prototypes arrive through this same data:extend, so skipping them is not a
-- convenience: a check that policed them would refuse a load the engine
-- completes, which is the one thing this file may never do.
--
-- NOTHING ELSE IS ENFORCED HERE. The 20-parameter and 20-level ceilings are
-- real and the library answers them (maxLocalisedParams carries their
-- measurement), but this round measured only the byte rule, and a stand-in rule
-- wider than its measurement refuses what the engine accepts.
--
-- A BARE STRING is legal in both fields and is NOT measured, so it is passed.
-- The engine's sentence calls the offender a "key", which makes it likely the
-- rule applies there too, and likely is not measured. Nothing a consumer can
-- write reaches that form today: appendLocalised always emits the table form,
-- and checkExtra and its Rust twin refuse localised_name and
-- localised_description in Extra outright, on an item, a recipe and a
-- technology alike.
--
-- AND THIS SEES ONLY WHAT CROSSES data:extend. A field written straight into
-- data.raw by an OpSet never reaches here, so the stand-in would not police it;
-- the only splice this library performs today is a technology's prerequisites,
-- which carries no localised string, so the gap is inert. The Go and Rust walks
-- over a plan do cover Op::Set, so it is covered somewhere even when it stops
-- being inert.
local function check_localised_elements(v, ptype, pname, path)
  -- ipairs and not pairs: a localised string is the engine's own array, walked
  -- in index order, so nothing here can depend on a table's iteration order.
  for i, e in ipairs(v) do
    local where = path .. "[" .. (i - 1) .. "]"
    if type(e) == "string" then
      if #e > 200 then
        error("Error while loading " .. ptype .. " prototype \"" .. pname ..
              "\" (" .. ptype .. "): Localised string key is too large: " ..
              #e .. " > 200 (limit). in property tree at ROOT." .. ptype ..
              "." .. pname .. "." .. where, 0)
      end
    elseif type(e) == "table" then
      check_localised_elements(e, ptype, pname, where)
    end
  end
end

-- THE TWO NAMED FIELDS AND NOT A SHAPE GUESS. appendLocalised is this library's
-- one writer of localised strings and writes exactly localised_name and
-- localised_description. A walk that instead measured every field that LOOKED
-- like a localised string would reach ingredients, effects and unit, whose long
-- strings the engine's rule does not touch, so it would refuse loads the engine
-- completes.
local function check_localised(p)
  if setting_types[p.type] then return end
  if type(p.localised_name) == "table" then
    check_localised_elements(p.localised_name, p.type, p.name, "localised_name")
  end
  if type(p.localised_description) == "table" then
    check_localised_elements(p.localised_description, p.type, p.name, "localised_description")
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
    -- The property tree is parsed BEFORE the prototype is loaded, so the byte
    -- rule fires ahead of every legality number below.
    check_localised(p)
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

-- One science pack in the shape the engine under test gives it. The two shapes
-- differ in three fields and in nothing else, so the difference the guests see
-- is the difference the engine made and not one this stand-in invented.
--
-- THE SECOND ARGUMENT IS THE 2.1 ARM'S PROTOTYPE TYPE, and it is here so the
-- library's 2.1 probe has to WALK the item family rather than reading the
-- plain `item` table alone. On 2.1 a science pack is an item carrying subgroup
-- "science-pack", and `item` is one of 21 types in that family: base's own
-- packs are plain items and answer on the first probe, so a pack declared as
-- one of the other twenty is the only thing that exercises the walk, and a mod
-- may declare one. `capsule` is that row below. On the 2.0 arm the argument is
-- ignored: a pack there is a `tool` and nothing else.
local function pack_row(name, item_type)
  if packs_are_items then
    return { type = item_type or "item", name = name,
             icon = "__base__/" .. name .. ".png",
             icon_size = 64, stack_size = 200, subgroup = "science-pack" }
  end
  return { type = "tool", name = name, icon = "__base__/" .. name .. ".png",
           icon_size = 64, stack_size = 200, durability = 1 }
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
  -- THE SCIENCE PACKS, in whichever shape the engine under test has them.
  --
  -- On 2.0 a pack is a TOOL and not an item: the library's ItemExists has to
  -- walk the derived types to find it, which is the walk those rows exist for,
  -- and a pack list asks ToolExists, which reads that table by name. On 2.1
  -- there is no data.raw.tool at all and a pack is an ITEM carrying subgroup
  -- "science-pack", which is the leaf the library's 2.1 arm reads; the
  -- durability field goes with the type, because an item has none.
  --
  -- ALL FOUR PACKS THE BASE TECHNOLOGIES BELOW PRICE THEMSELVES IN. The
  -- stand-in enforces the engine's measured research-unit rule on both arms,
  -- so a base row naming a pack with no entry would be an UNFAITHFUL stand-in
  -- rather than a deliberate gap: the engine has no such technology either.
  pack_row("automation-science-pack"),
  -- A DERIVED ITEM TYPE, for the reason pack_row's comment gives. THIS PACK
  -- AND NOT ANOTHER: it is the one the guests' own ladders and their copied
  -- units both name, so on the 2.1 arm a probe that read the plain `item`
  -- table alone would drop it and the arm's prototype comparison would go red.
  -- A pack base alone prices itself in proves nothing, because the guest never
  -- asks about it.
  pack_row("logistic-science-pack", "capsule"),
  pack_row("military-science-pack"),
  pack_row("chemical-science-pack"),

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
  -- TWO MORE MILITARY TECHNOLOGIES, present so the stand-in answers the
  -- question rather than answering "absent" by accident: nothing in either
  -- gate hangs hardened-tips off them now that the tier places it, and a
  -- data.raw missing them would make a future ladder rung look absent when it
  -- is only unmodelled.
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

-- A MODPACK THAT DEMOTED A SCIENCE PACK, which is finding 13 of the consumer's
-- second migration assessment in the one shape a stand-in can hold it: the
-- prototype stays, as an ITEM, and stops being a TOOL.
--
-- IT HAPPENS AFTER THE BASE ROWS AND THAT IS THE POINT. base's own military-4
-- was declared while the pack was still a tool and is legal; the demotion is a
-- later mod's doing, and this library's data stage runs after both. So a
-- verbatim copy of that unit hands the engine a research priced in something it
-- refuses, with `Invalid research unit (military-science-pack). Research
-- unit(s) can only be tool type items at the moment.` and nothing naming this
-- mod. check_technology above enforces exactly that sentence, so the guests'
-- own filter is what keeps this run green.
--
-- ON THE 2.1 ARM THE PROTOTYPE IS ALREADY AN ITEM, so the demotion is the
-- other half of the same move: the pack loses the subgroup that made it a
-- science pack and the labs stop taking it. The engine's refusal there is the
-- lab-coverage sentence rather than the tool-type one, and check_technology
-- enforces that one on that arm. Either way the guest has one pack to drop
-- and the prototypes it emits must not move between the two.
if packs_are_items then
  data.raw.item["military-science-pack"].subgroup = nil
  lab_accepts["military-science-pack"] = nil
else
  data.raw.item["military-science-pack"] = data.raw.tool["military-science-pack"]
  data.raw.item["military-science-pack"].type = "item"
  data.raw.item["military-science-pack"].durability = nil
  data.raw.tool["military-science-pack"] = nil
end

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
-- THIS TABLE IS THE COMPLEMENT OF THE IN-GAME GATE'S FLIPPED ROW, field by
-- field: where that row leaves a text alone this one types into it, where that
-- row types this one leaves the dropdown deciding, and the research cost that
-- is overridden whole there is overridden by ONE FIELD here. Between the two
-- gates every side of the switch is walked, and one of the two texts here is a
-- list the language REFUSES: that is a log line and the author's own list
-- rather than a load failure, and the composed line is pinned here.
settings = { startup = {
  ["fkrecipes-example-hardened-tools"] = { value = false },
  ["fkrecipes-example-forging-time"] = { value = 7.5 },
  -- THE TEXT WINS OVER A PRESET. The dropdown is on oil and the text setting
  -- beside it says something else, so the list the player wrote is what the
  -- recipe is made of and the log line carries the clause that says which
  -- choice was set aside. The in-game gate leaves this same text ALONE with the
  -- same dropdown on oil, so between the two gates both sides of one pair are
  -- covered.
  ["fkrecipes-example-quench-medium"] = { value = "oil" },
  ["fkrecipes-example-quench-ingredients"] = { value = "2 steel-plate, 6 iron-stick" },
  -- Left ON, so the technology it gates comes out enabled with no hidden
  -- field at all: the other side of the switched-off branch above.
  ["fkrecipes-example-bonus-research"] = { value = true },
  -- A PARTIAL CUSTOM COST. The MILITARY ladder is chosen, which the game's own
  -- default does not take, and the count alone is moved: the time and the packs
  -- come from that tier, and the line carries the clause that says so. The
  -- in-game gate moves all three of tips-count, tips-seconds and tips-packs on
  -- the same technology, so between the two gates the merge is covered whole
  -- and per field.
  --
  -- tips-seconds and tips-packs are ABSENT here on purpose, which the stand-in
  -- answers with nothing at all: an unreadable setting takes its declared
  -- default and says so, and the two lines that say it are what the engine
  -- never writes, because mod-settings.dat carries every setting's value.
  ["fkrecipes-example-tips-research-tier"] = { value = "military" },
  ["fkrecipes-example-tips-count"] = { value = 45 },
  -- Above the declared minimum of 0.5, so it is the value the player chose
  -- that reaches the recipe rather than any bound.
  ["fkrecipes-example-tempering-hold"] = { value = 4 },
  -- A TEXT THE LANGUAGE REFUSES, in a setting with no dropdown in front of it,
  -- and this is where the composed ERROR line is pinned byte for byte across
  -- the two halves: the language's own sentence, the shared prefix trimmed off
  -- it, and the instruction that names the screen the player fixes it on. The
  -- list that reaches the recipe is the mod's OWN declared one, because this
  -- field has no dropdown beside it, which is what "loaded as though that text
  -- had been left alone" means in the prototype rather than only in the line.
  --
  -- THE OTHER SIDE OF THIS SETTING IS THE IN-GAME GATE'S, where the same field
  -- carries a list the language accepts, so one gate covers the refusal and the
  -- other the success on exactly the arm with no dropdown in front of it.
  ["fkrecipes-example-rivet-ingredients"] = { value = "3 steel-plate, 2 iron-stik" },
  -- THE DROPDOWN DECIDES, because the text setting beside it is untouched. The
  -- long links are what the chain comes out of and nothing is logged about the
  -- text at all; the in-game gate types into this same field with the same
  -- dropdown on long, so between the two gates both sides of the switch are
  -- covered on one pair.
  ["fkrecipes-example-chain-links"] = { value = "long" },
  -- A RESEARCH COST THE PLAYER PRICED AND THE LIBRARY TOOK, with no dropdown in
  -- front of it: two packs in the text, so the unit carries two short tuples
  -- that came out of a TYPED list rather than out of a declaration, and the
  -- count and the seconds from their own numeric settings. The in-game gate
  -- puts a refused text in a PACK list instead, so between the two gates both
  -- of the language's list kinds are covered on both sides of the fallback.
  ["fkrecipes-example-chain-packs"] = { value = "2 automation-science-pack, 1 logistic-science-pack" },
  ["fkrecipes-example-chain-count"] = { value = 25 },
  ["fkrecipes-example-chain-seconds"] = { value = 12 },
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
