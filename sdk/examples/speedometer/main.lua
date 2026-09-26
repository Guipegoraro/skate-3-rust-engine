-- Speedometer: horizontal speed from the skater's position change each physics tick,
-- so it reads the same on the board and on foot.

local SCALE = {['km/h'] = 3.6, mph = 2.23694, ['m/s'] = 1}
-- Larger steps in one tick are teleports or respawns, not speed.
local TELEPORT_STEP = 2
local last, speed, top, since_draw = nil, 0, 0, 0

local function reset() last, speed, top = nil, 0, 0 end

local function draw()
    if not sdk.settings.show then sdk.scene.remove('speed'); return end
    local units = sdk.settings.units
    local k = SCALE[units] or 3.6
    sdk.ui.text('speed', string.format('%.0f %s   max %.0f', speed * k, units, top * k))
end

local function sample(event)
    local p = sdk.player.read().position
    if last and event.dt > 0 then
        local dx, dz = p[1] - last[1], p[3] - last[3]
        local step = math.sqrt(dx * dx + dz * dz)
        if step < TELEPORT_STEP then
            -- Light smoothing so the number is readable.
            speed = speed + (step / event.dt - speed) * 0.2
            top = math.max(top, speed)
        end
    end
    last = {p[1], p[2], p[3]}
end

return {
    on_load = draw,
    on_settings = draw,
    on_fixed_update = sample,
    on_update = function(event)
        since_draw = since_draw + event.dt
        if since_draw >= 0.1 then since_draw = 0; draw() end
    end,
    on_event = function(e)
        -- World changes remove all mod objects; top speed belongs to the old map.
        if e.name == 'world_changed' then reset(); draw() end
    end,
}