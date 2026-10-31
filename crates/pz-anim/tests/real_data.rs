use pz_anim::*;
use std::path::PathBuf;

fn media() -> Option<PathBuf> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let m = PathBuf::from(dir).join("media");
    m.is_dir().then_some(m)
}

#[test]
fn x_duration_reads_the_last_key_over_the_tick_rate() {
    let x = "AnimTicksPerSecond  {\n 4800;\n}\nAnimationSet S {\n Animation {\n  { Bip }\n  AnimationKey R {\n   0;\n   2;\n   0;4;1,0,0,0;;,\n   9600;4;1,0,0,0;;;\n  }\n }\n}\n";
    assert_eq!(x_duration(x), Some(2.0));
    assert_eq!(x_duration("nothing here"), None);
}

#[test]
fn nodes_inherit_and_override_events() {
    let bang = parse_node("<animNode><m_Name>DoorBang</m_Name><m_AnimName>Zombie_DoorBang</m_AnimName><m_SpeedScale>0.80</m_SpeedScale><m_Events x_name=\"a\"><m_EventName>ThumpFrame</m_EventName><m_TimePc>0.85</m_TimePc></m_Events></animNode>", None);
    assert_eq!((bang.anim_name.as_str(), bang.speed_scale, bang.events.len()), ("Zombie_DoorBang", 0.8, 1));
    let door = parse_node("<animNode x_extends=\"DoorBang.xml\"><m_Name>door</m_Name><m_AnimName>Zombie_Door</m_AnimName><m_Events x_name=\"a\"><m_TimePc>0.55</m_TimePc></m_Events></animNode>", Some(&bang));
    assert_eq!(door.events, vec![Event { name: "ThumpFrame".into(), time_pc: 0.55 }]);
    assert_eq!(door.speed_scale, 0.8);
}

#[test]
fn zombie_door_bang_lasts_two_seconds_and_thumps_late_in_the_loop() {
    let Some(m) = media() else { return };
    let text = std::fs::read(m.join("anims_X/Zombie/Zombie_DoorBang.X")).unwrap();
    let len = x_duration(&String::from_utf8_lossy(&text)).unwrap();
    assert!((len - 2.0).abs() < 1e-3, "{len}");
    let xml = std::fs::read_to_string(m.join("AnimSets/zombie/thump/DoorBang.xml")).unwrap();
    let node = parse_node(&xml, None);
    let t = event_times(&node, len, "ThumpFrame");
    assert_eq!(t.len(), 1);
    assert!((t[0] - 0.85 * 2.0 / 0.8).abs() < 1e-3, "{t:?}");
    let lens = clip_lengths(&m.join("anims_X"));
    assert!(lens.len() > 1000 && lens.contains_key("Zombie_DoorBang"), "{}", lens.len());
}
