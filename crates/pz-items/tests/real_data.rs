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
    let bat = db.get("Base.BaseballBat").expect("BaseballBat");
    assert_eq!((bat.min_damage, bat.max_damage, bat.max_range), (0.8, 1.1, 1.25));
    assert_eq!(bat.swing_time, 3.0);
}
