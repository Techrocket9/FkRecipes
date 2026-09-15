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
-- THAT REFUSAL AND THIS WHOLE FIXTURE ARE 2.0's. On 2.1 the tool type carries
-- no prototype at all and a research unit takes items, so the demotion an
-- overhaul pack performs there is a different move on a different table; see
-- the guard below and run-mirror.sh's 2.1 engine arm.
--
-- THIS RUNS AT data.lua AND NOT LATER, because the guest's own data stage runs
-- at data.lua too and the whole point is that it sees the demoted world. The
-- gate adds `? fkrecipes-demote` to the packaged guest's own dependencies so
-- the order is the engine's rather than alphabetical luck.
--
-- AND IT REFUSES RATHER THAN DOING NOTHING when there is no tool table at all,
-- which is every 2.1 engine: measured on Factorio 2.1.17 build 87315, data.raw
-- has no `tool` key and base's science packs are data.raw.item entries with
-- subgroup "science-pack". scripts/run-ingame.sh keys on the engine series and
-- SKIPS this whole arm on anything but 2.0, so reaching this line on a later
-- engine means the skip did not fire. A fixture that quietly demoted nothing
-- would leave the arm green while proving nothing, which is the failure the
-- gate rules here call a skipped gate reading like a pass.
--
-- THE GUARD IS REACHABLE BECAUSE THE GATE STAMPS THIS MOD'S factorio_version
-- from the binary's own series on copy, the way it stamps the packaged guest's.
-- The committed value below is 2.0, the series this fixture is for; measured on
-- 2.1.17, a mod declaring 2.0 is refused at game start with `Incompatible
-- Factorio version (current: 2.1, required: 2.0)` before a line of it runs, so
-- without the stamp this guard could never fire and the whole mod set would go
-- down for a reason that is not the one this arm is about.
if data.raw.tool == nil then
  error("fkrecipes-demote: this engine has no data.raw.tool, so there is no " ..
        "tool-typed science pack to demote and this fixture proves nothing. " ..
        "run-ingame.sh is supposed to skip the demote arm on such an engine.", 0)
end
local pack = data.raw.tool["automation-science-pack"]
if pack then
  data.raw.tool["automation-science-pack"] = nil
  pack.type = "item"
  data.raw.item["automation-science-pack"] = pack
end
