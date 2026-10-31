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

#[test]
fn fmod_dll_holds_the_vorbis_setup_packets_and_the_first_stream_decodes() {
    use pz_sounds::fsb5::{locate_in_bank, Fsb5};
    use std::io::{Read, Seek, SeekFrom};
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let dir = std::path::PathBuf::from(dir);
    let (Ok(mut f), Ok(dll)) = (std::fs::File::open(dir.join("media/sound/banks/Desktop/ZomboidSound.bank")), std::fs::read(dir.join("fmod.dll"))) else { return };
    let len = f.metadata().unwrap().len();
    let mut read_at = |off: u64, n: usize| { f.seek(SeekFrom::Start(off))?; let mut v = vec![0; n.min((len - off) as usize)]; f.read_exact(&mut v)?; Ok(v) };
    let (off, _) = locate_in_bank(len, &mut read_at).unwrap().unwrap();
    let need = Fsb5::header_len(&read_at(off, 64).unwrap()).unwrap();
    let fsb = Fsb5::parse(&read_at(off, need + 1_000_000).unwrap()).unwrap();
    // Every distinct setup CRC used by the bank names a packet stored in fmod.dll (`05 "vorbis"` + codebooks).
    let crcs: std::collections::BTreeSet<u32> = fsb.streams.iter().filter_map(|s| s.vorbis_crc).collect();
    for c in &crcs {
        let setup = pz_sounds::vorbis::setup_packet(&dll, *c).expect("setup packet in fmod.dll");
        assert!(setup.len() > 1000 && &setup[1..7] == b"vorbis");
    }
    // The first stream decodes to exactly its sample count of real audio (block sizes 1024 / 2048). The later
    // streams' boundaries are not understood yet, so only this one is checked.
    let s = &fsb.streams[0];
    let data = read_at(off + fsb.data_start as u64, 4164).unwrap();
    let setup = pz_sounds::vorbis::setup_packet(&dll, s.vorbis_crc.unwrap()).unwrap();
    let (pcm, _) = pz_sounds::vorbis::decode(setup, s.channels as u8, s.frequency, (10, 11), &data, 0, s.samples).unwrap();
    assert_eq!(pcm.len() as u64, s.samples * s.channels as u64);
    let peak = pcm.iter().map(|x| x.unsigned_abs()).max().unwrap();
    assert!(peak > 1000 && peak < 32767, "peak {peak}");
}
