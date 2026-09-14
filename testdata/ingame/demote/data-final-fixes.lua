-- AND THE BASE GAME IS REPAIRED AROUND IT, which is what keeps this row inside
-- scope F rather than in scope I. A pack that breaks base's own prototypes is
-- not this library's problem (the threat model says so by name); a pack that
-- demotes one science pack and re-prices base's own research around it is a
-- real overhaul pack, and it is the only shape in which the demotion can be
-- graded against this library at all.
--
-- AT data-final-fixes, AFTER EVERY MOD'S DATA STAGE, so the guest under test
-- has already seen the demoted world and the engine has not yet loaded a
-- prototype. Base's research is simply not priced in the demoted pack any
-- more: an empty research unit loads (measured on 2.0.77), and a technology
-- base leaves with fewer packs is base's own balance and not this gate's
-- subject.
-- ipairs OVER EVERY LIST AND pairs ONLY OVER THE KEYED TABLES, which is this
-- repository's standing rule one level out: Lua's pairs order over string keys
-- is seeded per run, so nothing may depend on it in the library OR in the
-- harness. The two outer walks build the same result in any order; the two
-- inner ones rebuild a LIST, where order is the content, so they are ipairs.
local demoted = "automation-science-pack"

-- EVERY TECHNOLOGY EXCEPT THE GUEST'S OWN, and the exclusion is the whole
-- reason this loop is not one line shorter. Repairing the guest's would empty a
-- unit the LIBRARY was supposed to empty itself, and every assertion about the
-- degradation would then pass over a dump this fixture had corrected. That is
-- not hypothetical: it is what the arm's own red proof found, with the
-- demotion disabled and both tooltip assertions still green.
--
-- BY PREFIX, AND A SNAPSHOT TAKEN IN data.lua WAS TRIED FIRST AND IS WRONG.
-- Listing the technologies that existed before this fixture ran misses every
-- one a mod loading LATER adds: space-age's data.lua runs after this fixture's
-- (measured in the gate's own log), so advanced-asteroid-processing was not in
-- the snapshot, kept the demoted pack, and refused the load with the engine's
-- own `Invalid research unit` sentence. The guest's prefix is the one thing
-- that names exactly the prototypes under test.
local guest = "fkrecipes-example-"

for name, tech in pairs(data.raw.technology) do
  local unit = tech.unit
  if name:sub(1, #guest) ~= guest and unit and unit.ingredients then
    local kept = {}
    for _, entry in ipairs(unit.ingredients) do
      if (entry[1] or entry.name) ~= demoted then
        kept[#kept + 1] = entry
      end
    end
    unit.ingredients = kept
  end
end

-- A LAB'S inputs ARE TOOL NAMES, so one naming a plain item is the same
-- refusal one rung down. Left in the list it would break the base game and
-- move this row out of scope.
for _, lab in pairs(data.raw.lab) do
  if lab.inputs then
    local kept = {}
    for _, name in ipairs(lab.inputs) do
      if name ~= demoted then
        kept[#kept + 1] = name
      end
    end
    lab.inputs = kept
  end
end
