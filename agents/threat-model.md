# Threat model for the FkRecipes customizer and its pilot

This document bounds what the customizer defends against, so that an assessment grades against a stated scope and the fix loop converges. It is the scope every assessment from the third onward grades to, and the scope every customizer design decision is reviewed against before it is implemented. Written 2026-09-13 after the second assessment; the design review of that day (the operator's briefs directory, design-review-cycle2.md) reviewed it and its four corrections are folded in.

## Who the player is, and where they look

The player is someone who installs the mod from the portal, changes settings on the Mod Settings screen, updates and sometimes rolls back through the portal or Steam, plays alone or on a server, and reads three things: the settings screen (labels, tooltips), the in-game tooltip of the recipe or technology they are looking at, and the changelog the portal shows on update. THE LOG IS NOT WHERE A PLAYER LOOKS. A log line is evidence for us and a courtesy for the curious; a disclosure that exists only in the log does not count as one.

## In scope: what must be CLEAN, or engine-owned and disclosed where a player looks

A. **Updating.** A player on any PUBLISHED release with any value that release offered stored, updating to the next release. Every value survives by name; nothing is lost silently; the screen shows what it showed.

B. **Customizing from the screen.** A player on head typing anything the field accepts and any number the widget accepts: typos, display names instead of internal names, a decimal comma, pasted text with invisible characters, rich-text tags, the reserved words in any case, an empty field, a very long text. The game always loads; the player can correct the value from the Mod Settings screen without leaving the game or losing any other value; what happened is stated where they look (the settings tooltip states the rule, the recipe or technology tooltip states the outcome).

C. **Rolling back and returning.** A player who runs the previous published release once (a Steam rollback, a modpack pin, a second machine) and comes back to head. Nothing they chose on head is lost silently. Values older releases never declared must survive the trip untouched; that is what the design relies on, and it is measured. The same scope covers a rollback of the GAME: a settings file stamped by a newer engine version is a file the engine's own writer produces, and what the older engine does with it (measured: it refuses the file and rewrites defaults) is engine-owned and disclosed, not out of scope.

D. **Saves.** A save made on the previous release loaded on head, and a head save loaded on the previous release, with the balancer researched, entities placed, items in inventories and on belts, a craft in progress: research, entities, belts, chests, progress and the output slot survive; the startup-settings sync prompt's default choice restores the save's values. A head save loaded on a head whose text was then changed: the same, minus what the engine destroys on any recipe change (engine-owned, disclosed).

E. **Multiplayer and determinism.** The same settings file on two machines produces the same prototype dump. A headless server with a text the library cannot use starts, with the degradation logged, because there is no dialog to click. A server and a client whose files differ are the engine's startup-settings sync, recorded not designed for; a real two-party join cannot be measured on this machine (there is no headless client), so it is reported NOT REACHABLE like L, never as a finding.

F. **Modpacks that change what the declarations reference, while leaving the base game loadable.** A pack that removes, renames or demotes (a science pack that is no longer a tool) an item, fluid or technology this mod's declarations or the player's text name. The load never stops on this mod's account; the library emits the nearest legal prototype, logs one ERROR line, and the recipe or technology tooltip says what was substituted. The declared fallback ladders are the author's answer to the common cases; degradation is the floor under them.

G. **Files the game writes.** Any mod-settings.dat the engine itself wrote, on any version of this mod or with other mods' values present: read correctly, unknown names ignored, nothing rewritten but what the engine rewrites.

## Out of scope: recorded in an appendix, never blocking

H. **Hand-edited or corrupted settings files**: bytes the engine's own writer never produces. A NaN, a NUL, invalid UTF-8, a wrong type node, a truncated file. (A newer version stamp is the engine's own writer's and belongs to C.) The library defends where it can cheaply (no double-typed setting, so a NaN has no field; every invisible byte refused by code point), the engine's behaviour is recorded in FkLua's agents/engine-findings.md, and none of it blocks. An out-of-list or out-of-range value reached by a legitimate rollback is case C and in scope by that route.

I. **Packs that break the base game itself** before they reach this mod (base's own recipes or technologies fail to load). Not ours.

J. **Mods that load after this one and rewrite or delete its prototypes.** The engine's staging model. The final-fixes verify gives the one signal that can exist; beyond it, not ours.

K. **Engine behaviours no mod can change**: the assembler input stacks destroyed on a recipe change, the silent reset of a value the engine considers invalid, the error dialog's missing route to Mod Settings, the settings screen's flat list with no conditional visibility, the NUL cut under 22 stored bytes, the abort on a stored NaN. Each is listed in FkLua's engine-findings.md, and each is graded CLEAN when the design has removed every path of ours that leads to it and, where a disclosure is possible, it is disclosed where a player looks; undisclosed where disclosure is possible, it is AWKWARD; a path of ours that still leads a player into one of them is graded as the outcome (a lock-out or an abort is BLOCKED). The NaN abort has no dialog to disclose in, so its CLEAN condition is path removal alone: the library declares no double-typed setting, and a consumer's own double setting is the consumer's exposure, named in docs/usage.md.

L. **A real 2.1 binary.** Not on this machine. Every head measurement is of the 2.0 recut; a difference that could only show on 2.1 is NOT REACHABLE, never a finding.

## The rubric, restated against the scope

- BLOCKED: an in-scope situation leaves the player unable to reach the game, or loses a value they chose, silently, or leaves the mod's own content unreachable in a game that plays (a technology that cannot be researched, a recipe that cannot be crafted) with nothing where a player looks saying why and what to change; with that disclosure it is AWKWARD.
- AWKWARD: in-scope, the game plays and the value is kept, but something changed that is not stated where a player looks.
- MISLED: a description, tooltip, changelog or doc states something false about an in-scope situation.
- CLEAN: none of the above, in scope.
- Each finding carries an OWNER (FkRecipes, BBB, FkLua, engine) and a SCOPE (A to L).
- Out-of-scope findings go to the appendix with their evidence and never carry a blocking grade.

## The stopping rule

The chain stops when an assessment reports nothing above CLEAN that is in scope and owned by FkRecipes, BBB or FkLua. Engine-owned items are disclosed and listed; out-of-scope items are recorded.

## What must be measured on a client before a design decision that touches B, C or D is adopted

The client checklist is not optional for those three: the error dialog and its routes, the Mod Settings screen with the composed tooltips rendered, typing into the field, the startup-settings sync prompt. Computer-use access is requested at the start of the round that needs it and released the moment the checks end.
