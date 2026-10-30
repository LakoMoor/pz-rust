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

#[test]
fn sound_bank_streams_can_be_listed() {
    use pz_sounds::fsb5::{locate_in_bank, Codec, Fsb5};
    use std::io::{Read, Seek, SeekFrom};
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let path = std::path::PathBuf::from(dir).join("media/sound/banks/Desktop/ZomboidSound.bank");
    let Ok(mut f) = std::fs::File::open(&path) else { return };
    let len = f.metadata().unwrap().len();
    let mut read_at = |off: u64, n: usize| { f.seek(SeekFrom::Start(off))?; let mut v = vec![0; n.min((len - off) as usize)]; f.read_exact(&mut v)?; Ok(v) };
    let (off, _size) = locate_in_bank(len, &mut read_at).unwrap().expect("SND chunk with FSB5");
    let first = read_at(off, 64).unwrap();
    let need = Fsb5::header_len(&first).unwrap();
    let blob = read_at(off, need).unwrap();
    let fsb = Fsb5::parse(&blob).unwrap();
    assert_eq!(fsb.codec, Codec::Vorbis);
    assert!(fsb.streams.len() > 10_000, "streams {}", fsb.streams.len());
    let ok = fsb.streams.iter().filter(|s| matches!(s.frequency, 22050 | 44100 | 48000) && matches!(s.channels, 1 | 2) && s.samples > 0).count();
    assert!(ok * 100 > fsb.streams.len() * 99, "sane streams {ok}/{}", fsb.streams.len());
    // Offsets run monotonically through the data section and stay inside it.
    let mut last = 0;
    for s in &fsb.streams { assert!(s.data_offset >= last && s.data_offset < fsb.data_size as u64); last = s.data_offset; }
    let missing = fsb.streams.iter().filter(|s| s.vorbis_crc.is_none()).count();
    assert!(missing * 100 < fsb.streams.len(), "streams without a setup CRC: {missing}/{}", fsb.streams.len());
}
