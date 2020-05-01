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

function watcher(data)
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

add_mem_watch(0xf51500, 6, watcher)