-- THE MEASURED MODPACK SHAPE OF FINDING 13, as a Lua-only mod: one science
-- pack that is no longer a tool, with the base game still loadable.
--
-- automation-science-pack moves from data.raw.tool to data.raw.item and keeps
-- every other field. That is what an overhaul pack does when it replaces the
-- early science chain: the prototype is still there, still craftable and still
-- named by everything that named it, and it is no longer a research unit. The
-- engine's own answer to a research priced in one is
-- `Invalid research unit (automation-science-pack). Research unit(s) can only
-- be tool type items at the moment.`, which names neither the mod nor the
-- setting, so the library has to see the demotion for itself: ToolExists is
-- the one probe that answers existence and toolness together.
--
-- THIS RUNS AT data.lua AND NOT LATER, because the guest's own data stage runs
-- at data.lua too and the whole point is that it sees the demoted world. The
-- gate adds `? fkrecipes-demote` to the packaged guest's own dependencies so
-- the order is the engine's rather than alphabetical luck.
local pack = data.raw.tool["automation-science-pack"]
if pack then
  data.raw.tool["automation-science-pack"] = nil
  pack.type = "item"
  data.raw.item["automation-science-pack"] = pack
end
