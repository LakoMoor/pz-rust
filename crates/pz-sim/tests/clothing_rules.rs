use pz_sim::clothing::*;

fn rules() -> Option<Rules> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let p = std::path::PathBuf::from(dir).join("media/lua/shared/NPCs/BodyLocations.lua");
    Some(Rules::from_lua(&std::fs::read_to_string(p).ok()?))
}

fn g(loc: &str, part_loc: &str) -> Garment {
    let mut g = Garment::new(part_loc, 0.5, 0.5);
    g.location = norm_location(loc);
    g
}

#[test]
fn rules_parse_from_a_script_snippet() {
    let r = Rules::from_lua("group:getOrCreateLocation(ItemBodyLocation.HAT)\ngroup:getOrCreateLocation(ItemBodyLocation.FULL_HAT) -- x\ngroup:setExclusive(ItemBodyLocation.FULL_HAT, ItemBodyLocation.HAT)\ngroup:setMultiItem(ItemBodyLocation.BANDAGE, true)\n");
    assert!(r.is_exclusive("fullhat", "hat") && r.is_exclusive("hat", "fullhat"));
    assert!(r.is_multi("bandage") && !r.is_multi("hat"));
    assert_eq!(norm_location("base:full_hat"), "fullhat");
}

#[test]
fn wearing_replaces_the_same_slot_and_kicks_out_exclusive_items() {
    let r = Rules::from_lua("group:getOrCreateLocation(ItemBodyLocation.HAT)\ngroup:getOrCreateLocation(ItemBodyLocation.FULL_HAT)\ngroup:setExclusive(ItemBodyLocation.FULL_HAT, ItemBodyLocation.HAT)\n");
    let mut worn = vec![];
    assert!(r.wear(&mut worn, g("base:hat", "Head")).is_empty());
    let out = r.wear(&mut worn, g("base:hat", "Head"));
    assert_eq!((out.len(), worn.len()), (1, 1), "a second hat replaces the first");
    let out = r.wear(&mut worn, g("base:fullhat", "Head"));
    assert_eq!((out.len(), worn.len(), worn[0].location.as_str()), (1, 1, "fullhat"), "a full helmet pushes the hat off");
}

#[test]
fn the_games_own_body_locations_order_and_exclude_clothing() {
    let Some(r) = rules() else { return };
    // A parka (jacket) goes on over a shirt whatever order they are put on in.
    let mut worn = vec![];
    r.wear(&mut worn, g("base:jacket", "Jacket"));
    r.wear(&mut worn, g("base:tshirt", "Shirt"));
    let locs: Vec<&str> = worn.iter().map(|w| w.location.as_str()).collect();
    assert_eq!(locs, ["tshirt", "jacket"], "{locs:?}");
    // Real exclusions exist (e.g. a full helmet versus a hat) and bandages stack.
    assert!(r.is_exclusive("fullhat", "hat"));
    assert!(r.is_multi("bandage") && r.is_multi("wound"));
}
