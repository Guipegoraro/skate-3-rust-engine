-- D-pad Tools: controller shortcuts for save/return spot and push speed.

-- Controller bindings framework. Copy this block into other mods: SDK 1 loads
-- a single entry file, so there is no require(). Native action IDs come from
-- crates/skate-data/src/input_config.rs (GAMEPLAY table), not the doc summary.
local pad = {
    LB = 72, RB = 73,
    UP = 74, DOWN = 75, LEFT = 76, RIGHT = 77,
    X = 78, Y = 79, A = 80, B = 81,
}
local bindings, held = {}, {}
-- bind(action, callback, unless): callback fires once per press; skipped while
-- any action in `unless` is held (keeps native combos such as LB + D-pad).
local function bind(action, callback, unless)
    bindings[#bindings + 1] = {action = action, callback = callback, unless = unless or {}}
end
local function poll_bindings()
    local now = {}
    for _, binding in ipairs(bindings) do
        local id = binding.action
        if now[id] == nil then now[id] = sdk.input.action(id) > 0.5 end
        local blocked = false
        for _, other in ipairs(binding.unless) do
            if sdk.input.action(other) > 0.5 then blocked = true end
        end
        if now[id] and not held[id] and not blocked then binding.callback() end
    end
    held = now
end

-- Mod state.
local MIN_PUSH, MAX_PUSH = 0.25, 4
local push, spot = 1, nil

local function notice(text)
    sdk.ui.text('notice', text)
    sdk.time.after('notice', 2.5, function() sdk.scene.remove('notice') end)
end
local function draw()
    if not sdk.settings.show_hud then
        sdk.scene.remove('hud'); sdk.scene.remove('hint'); return
    end
    sdk.ui.text('hud', string.format('REMADA %.2fx%s', push, spot and ' | spot salvo' or ''))
    sdk.ui.text('hint', 'D-pad: < salvar  > voltar  ^ remada+  v remada-')
end
local function apply()
    sdk.trainer.apply({push_speed = push, push_power = sdk.settings.push_power and push or 1})
    draw()
end
local function change_push(direction)
    local next_push = math.max(MIN_PUSH, math.min(MAX_PUSH, push + direction * sdk.settings.push_step))
    if next_push == push then
        notice(direction > 0 and 'Remada ja no maximo (4x)' or 'Remada ja no minimo (0.25x)')
        return
    end
    push = next_push
    apply()
    notice(string.format('Remada %.2fx', push))
end
local function busy(p)
    return p.bailing or p.state == 702
end
local function save_spot()
    local p = sdk.player.read()
    if busy(p) then notice('Espere o skatista levantar para salvar'); return end
    local pos = p.position
    spot = {position = {pos[1], pos[2], pos[3]}, heading = p.heading or 0, on_board = p.on_board}
    sdk.scene.cube('spot', {pos[1], pos[2] + 0.5, pos[3]}, {0.15, 1, 0.15}, {0.1, 0.85, 0.85})
    notice('Spot salvo')
    draw()
end
local function return_to_spot()
    if not spot then notice('Salve um spot primeiro (D-pad esquerda)'); return end
    if busy(sdk.player.read()) then notice('Espere o skatista levantar'); return end
    sdk.player.teleport(spot.position, spot.heading, spot.on_board)
    notice('Voltando ao spot')
end

bind(pad.LEFT, save_spot)
bind(pad.RIGHT, return_to_spot)
bind(pad.UP, function() change_push(1) end, {pad.LB})
bind(pad.DOWN, function() change_push(-1) end, {pad.LB})

return {
    on_load = function() apply(); notice('D-pad Tools pronto') end,
    on_settings = apply,
    on_update = poll_bindings,
    on_event = function(e)
        if e.name == 'world_changed' then
            -- World changes remove all mod objects; a saved spot belongs to the old map.
            spot = nil; held = {}
            apply()
        end
    end,
}
