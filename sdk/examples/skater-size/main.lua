-- Skater Size: visual scale of the skater and board (sdk.player.scale, SK-038).
local function apply() sdk.player.scale(sdk.settings.size) end

return {
    on_load = apply,
    on_settings = apply,
}
