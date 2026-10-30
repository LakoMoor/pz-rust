use pz_sounds::SoundDb;

fn db() -> Option<SoundDb> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let scripts = std::path::PathBuf::from(dir).join("media").join("scripts");
    scripts.is_dir().then(|| SoundDb::load(&scripts).unwrap())
}

#[test]
fn sound_scripts_load_and_name_fmod_events() {
    let Some(db) = db() else { return };
    assert!(db.len() > 2500, "sounds={}", db.len());
    let t = db.get("Thunder").unwrap();
    assert_eq!(t.category, "World");
    assert_eq!(t.clips.len(), 1);
    assert_eq!(t.clips[0].event.as_deref(), Some("World/Weather/Thunder"));
    assert!(db.get("RumbleThunder").is_some());
    // Looping ambience is flagged.
    let amb = db.get("WorldAmbiance").unwrap();
    assert!(amb.looped);
    assert_eq!(amb.clips[0].event.as_deref(), Some("World/Ambiance"));
    // Every clip plays either a bank event or a loose file; events dominate.
    let (mut events, mut files, mut neither) = (0, 0, 0);
    for s in db.all() { for c in &s.clips {
        match (&c.event, &c.file) { (Some(_), _) => events += 1, (None, Some(_)) => files += 1, _ => neither += 1 }
    } }
    assert!(events > 2800 && files < 20, "events {events} files {files} neither {neither}");
    assert_eq!(db.get("MetaPistol1").unwrap().clips[0].distance_max, Some(1000.0));
}
