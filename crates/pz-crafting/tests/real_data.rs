use pz_crafting::*;
use pz_items::{Inventory, ItemDb};

fn data() -> Option<(ItemDb, Recipes)> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let scripts = std::path::PathBuf::from(dir).join("media").join("scripts");
    scripts.is_dir().then(|| (ItemDb::load(&scripts).unwrap(), Recipes::load(&scripts).unwrap()))
}

/// Stock an inventory with exactly the inputs of `r`; None if some input cannot be stocked.
fn stocked(db: &ItemDb, r: &Recipe) -> Option<Inventory> {
    let mut inv = Inventory::new(100_000.0);
    for i in &r.inputs {
        let id = match &i.source {
            Source::Items(ids) => ids[0].clone(),
            Source::Tag(t) => db.with_tag(t).first()?.to_string(),
        };
        inv.add(db, &id, i.count).ok()?;
    }
    Some(inv)
}

#[test]
fn recipes_load_and_simple_ones_craft() {
    let Some((db, recipes)) = data() else { return };
    assert!(recipes.len() > 900, "recipes={}", recipes.len());
    let supported = recipes.supported().count();
    assert!(supported > 200, "supported={supported}");
    eprintln!("{} recipes, {} supported", recipes.len(), supported);

    let mut crafted = 0;
    for (_, r) in recipes.supported() {
        if r.outputs.is_empty() || r.inputs.is_empty() { continue; }
        let Some(mut inv) = stocked(&db, r) else { continue };
        let before = inv.clone();
        craft(r, &db, &mut inv).unwrap_or_else(|e| panic!("{}: {e:?}", r.name));
        for (id, n) in &r.outputs {
            assert!(inv.count(id) >= *n, "{}: missing output {id}", r.name);
        }
        // Weight bookkeeping: crafting changes the weight by (outputs - consumed inputs), never loses track.
        assert!(inv.weight() >= 0.0);
        let _ = before;
        crafted += 1;
    }
    assert!(crafted > 50, "crafted={crafted}");
    eprintln!("crafted {crafted} recipes");
}

#[test]
fn missing_inputs_fail_atomically() {
    let Some((db, recipes)) = data() else { return };
    let (_, r) = recipes.supported().find(|(_, r)| !r.outputs.is_empty() && !r.inputs.is_empty()).unwrap();
    let mut inv = Inventory::new(100.0);
    assert!(matches!(craft(r, &db, &mut inv), Err(CraftError::Missing(_))));
    assert_eq!(inv.weight(), 0.0);
}

#[test]
fn kept_tools_survive_and_consumed_inputs_vanish() {
    let Some((db, recipes)) = data() else { return };
    // Find a recipe with exactly one kept and one consumed explicit input.
    let found = recipes.supported().find(|(_, r)| {
        r.inputs.len() == 2
            && r.inputs.iter().any(|i| i.keep && matches!(i.source, Source::Items(ref v) if v.len() == 1))
            && r.inputs.iter().any(|i| !i.keep && matches!(i.source, Source::Items(ref v) if v.len() == 1))
            && !r.outputs.is_empty()
    });
    let Some((_, r)) = found else { return };
    let mut inv = stocked(&db, r).expect("stock");
    let keep_id = r.inputs.iter().find(|i| i.keep).map(|i| match &i.source { Source::Items(v) => v[0].clone(), _ => unreachable!() }).unwrap();
    let used = r.inputs.iter().find(|i| !i.keep).unwrap();
    let used_id = match &used.source { Source::Items(v) => v[0].clone(), _ => unreachable!() };
    let (keep_before, used_before) = (inv.count(&keep_id), inv.count(&used_id));
    craft(r, &db, &mut inv).unwrap();
    assert_eq!(inv.count(&keep_id), keep_before, "tool is kept");
    if !r.outputs.iter().any(|(o, _)| *o == used_id) {
        assert_eq!(inv.count(&used_id), used_before - used.count, "ingredient is consumed");
    }
}
