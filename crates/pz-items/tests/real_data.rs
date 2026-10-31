use pz_items::*;

fn db() -> Option<ItemDb> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let scripts = std::path::PathBuf::from(dir).join("media").join("scripts");
    scripts.is_dir().then(|| ItemDb::load(&scripts).unwrap())
}

#[test]
fn database_and_inventory_weights() {
    let Some(db) = db() else { return };
    assert!(db.len() > 4000, "items={}", db.len());
    let spade = db.get("Base.SpadeHead").expect("SpadeHead");
    assert_eq!(spade.weight, 1.0);
    assert_eq!(spade.item_type, "base:weapon");

    let mut inv = Inventory::new(3.0);
    inv.add(&db, "Base.SpadeHead", 2).unwrap();
    assert_eq!(inv.count("Base.SpadeHead"), 2);
    assert!((inv.weight() - 2.0).abs() < 1e-4);
    assert_eq!(inv.add(&db, "Base.SpadeHead", 2), Err(AddError::TooHeavy));
    assert_eq!(inv.add(&db, "Base.NoSuchThing", 1), Err(AddError::UnknownItem));
    assert_eq!(inv.remove(&db, "Base.SpadeHead", 5), 2);
    assert!(inv.weight().abs() < 1e-4);
}

#[test]
fn baseball_bat_stats() {
    let Some(db) = db() else { return };
    // Melee weapons are silent on a swing (only the hit makes noise); firearms carry big radii.
    let bat = db.get("Base.BaseballBat").unwrap();
    assert_eq!((bat.sound_radius, bat.sound_volume), (0.0, 0.0));
    let pistol = db.get("Base.Pistol").unwrap();
    assert!(pistol.sound_radius >= 50.0 && pistol.sound_volume > 0.0, "{} {}", pistol.sound_radius, pistol.sound_volume);
}

#[test]
fn pistol_firearm_stats_come_from_the_scripts() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let scripts = std::path::PathBuf::from(dir).join("media").join("scripts");
    if !scripts.is_dir() { return; }
    let db = pz_items::ItemDb::load(&scripts).unwrap();
    let p = db.get("Base.Pistol").unwrap();
    assert!(p.is_firearm());
    assert_eq!((p.hit_chance, p.min_sight_range, p.max_sight_range, p.max_ammo, p.recoil_delay), (50.0, 2.0, 6.0, 15.0, 12.0));
    assert!(!db.get("Base.BaseballBat").unwrap().is_firearm());
}

#[test]
fn every_firearm_has_a_matching_ammo_item() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let scripts = std::path::PathBuf::from(dir).join("media").join("scripts");
    if !scripts.is_dir() { return; }
    let db = pz_items::ItemDb::load(&scripts).unwrap();
    assert_eq!(db.ammo_item("base:bullets_9mm"), Some("Base.Bullets9mm"));
    let guns: Vec<_> = db.all().filter(|d| d.is_firearm()).collect();
    assert!(guns.len() > 10);
    let mut missing = vec![];
    for g in &guns {
        let a = g.ammo_type.as_deref().unwrap();
        if db.ammo_item(a).is_none() { missing.push(format!("{} -> {a}", g.id)); }
    }
    assert!(missing.is_empty(), "no ammo item for: {missing:?}");
}

#[test]
fn clothing_scripts_carry_insulation_and_coverage() {
    let Some(db) = db() else { return };
    let hat = db.get("Base.Hat_WoolyHat").unwrap();
    assert_eq!((hat.insulation, hat.wind_resistance), (0.9, 0.9));
    assert_eq!(hat.blood_location, vec!["Head".to_string()]);
}
