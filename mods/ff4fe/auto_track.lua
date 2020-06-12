function handle_key_item_state(id, bit, found_key_items, used_key_items) 
    local state = OBJECTIVE_LOCKED
    local mask = 1 << bit
    if (used_key_items & mask) ~= 0 then
        state = OBJECTIVE_COMPLETE
    elseif (found_key_items & mask) ~= 0 then
        state = OBJECTIVE_UNLOCKED
    end

    set_objective_state(id, state)
end

function key_item_watcher(data)
    local found_key_items = data:get_u24(0)
    local used_key_items = data:get_u24(3)

    handle_key_item_state("package", 0x0, found_key_items, used_key_items)
    handle_key_item_state("sand-ruby", 0x1, found_key_items, used_key_items)
    handle_key_item_state("legend-sword", 0x2, found_key_items, used_key_items)
    handle_key_item_state("baron-key", 0x3, found_key_items, used_key_items)
    handle_key_item_state("twin-harp", 0x4, found_key_items, used_key_items)
    handle_key_item_state("earth-crystal", 0x5, found_key_items, used_key_items)
    handle_key_item_state("magma-key", 0x6, found_key_items, used_key_items)
    handle_key_item_state("tower-key", 0x7, found_key_items, used_key_items)
    handle_key_item_state("hook", 0x8, found_key_items, used_key_items)
    handle_key_item_state("luca-key", 0x9, found_key_items, used_key_items)
    handle_key_item_state("darkness-crystal", 0xa, found_key_items, used_key_items)
    handle_key_item_state("rat-tail", 0xb, found_key_items, used_key_items)
    handle_key_item_state("adamant", 0xc, found_key_items, used_key_items)
    handle_key_item_state("pan", 0xd, found_key_items, used_key_items)
    handle_key_item_state("spoon", 0xe, found_key_items, used_key_items)
    handle_key_item_state("pink-tail", 0xf, found_key_items, used_key_items)
    handle_key_item_state("crystal", 0x10, found_key_items, used_key_items)
end

function handle_loc_state(id, bit, bit_field)
    if (bit_field & (1 << bit)) ~= 0 then
        set_objective_state(id .. "-key-item-check", OBJECTIVE_COMPLETE)
    end
end

function loc_watcher(data)
    -- Indexes 0x20 - 0x3f
    local bit_field = data:get_u32(4)
    handle_loc_state("intro", 0x0, bit_field)
    handle_loc_state("antlion", 0x1, bit_field)
    handle_loc_state("fabul-gauntlet", 0x2, bit_field)
    handle_loc_state("mt-ordeals", 0x3, bit_field)
    handle_loc_state("baron-inn", 0x4, bit_field)
    handle_loc_state("baron-castle", 0x5, bit_field)
    handle_loc_state("edward", 0x6, bit_field)
    handle_loc_state("magnes", 0x7, bit_field)
    handle_loc_state("zot", 0x8, bit_field)
    handle_loc_state("top-of-tower", 0x9, bit_field)
    handle_loc_state("super-cannon", 0xa, bit_field)
    handle_loc_state("d-castle", 0xb, bit_field)
    handle_loc_state("sealed-cave", 0xc, bit_field)
    handle_loc_state("feymarch", 0xd, bit_field)
    handle_loc_state("adamant-grotto", 0xe, bit_field)
    handle_loc_state("sheila1", 0xf, bit_field)
    handle_loc_state("sheila2", 0x10, bit_field)
    handle_loc_state("asura", 0x11, bit_field)
    handle_loc_state("leviathan", 0x12, bit_field)
    handle_loc_state("odin", 0x13, bit_field)
    handle_loc_state("sylph-cave", 0x14, bit_field)
    handle_loc_state("bahamut", 0x15, bit_field)
    handle_loc_state("pale-dim", 0x16, bit_field)
    handle_loc_state("wyvern", 0x17, bit_field)
    handle_loc_state("plague", 0x18, bit_field)
    handle_loc_state("ds-lunar1", 0x19, bit_field)
    handle_loc_state("ds-lunar2", 0x1a, bit_field)
    handle_loc_state("ogopogo", 0x1b, bit_field)
end

add_mem_watch(0xf51500, 6, key_item_watcher)
add_mem_watch(0xf51510, 0x10, loc_watcher)
