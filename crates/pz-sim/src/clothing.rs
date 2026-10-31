//! Worn clothing as the thermoregulator sees it: which body parts each garment covers (`BloodClothingType`), and the
//! per-part insulation and wind resistance (`ThermalNode.calculateInsulation`). Holes and patches are not modelled.

use crate::body::PARTS;

// Part indices, in `BodyPartType` order.
const HAND_L: usize = 0; const HAND_R: usize = 1; const FOREARM_L: usize = 2; const FOREARM_R: usize = 3;
const UPPERARM_L: usize = 4; const UPPERARM_R: usize = 5; const TORSO_U: usize = 6; const TORSO_L: usize = 7;
const HEAD: usize = 8; const NECK: usize = 9; const GROIN: usize = 10; const UPPERLEG_L: usize = 11;
const UPPERLEG_R: usize = 12; const LOWERLEG_L: usize = 13; const LOWERLEG_R: usize = 14; const FOOT_L: usize = 15; const FOOT_R: usize = 16;

/// The body parts a `BloodLocation` covers (the `Back` zone has no thermal node and is dropped).
pub fn covered_parts(blood_location: &str) -> Vec<usize> {
    let shirt_no_sleeves = [TORSO_U, TORSO_L];
    let shirt = [&shirt_no_sleeves[..], &[UPPERARM_L, UPPERARM_R]].concat();
    let long = [&shirt[..], &[FOREARM_L, FOREARM_R]].concat();
    let shorts = vec![GROIN, UPPERLEG_L, UPPERLEG_R];
    match blood_location {
        "Apron" => vec![TORSO_U, TORSO_L, UPPERLEG_L, UPPERLEG_R],
        "ShirtNoSleeves" | "JumperNoSleeves" => shirt_no_sleeves.to_vec(),
        "Shirt" => shirt,
        "ShirtLongSleeves" | "Jumper" => long,
        "Jacket" => [&long[..], &[NECK]].concat(),
        "LongJacket" => [&long[..], &[NECK, GROIN, UPPERLEG_L, UPPERLEG_R]].concat(),
        "ShortsShort" => shorts,
        "Trousers" => [&shorts[..], &[LOWERLEG_L, LOWERLEG_R]].concat(),
        "Shoes" => vec![FOOT_L, FOOT_R],
        "FullHelmet" | "Head" => vec![HEAD],
        "Hands" => vec![HAND_L, HAND_R],
        "Neck" => vec![NECK],
        "Groin" => vec![GROIN],
        "UpperBody" => vec![TORSO_U],
        "LowerBody" => vec![TORSO_L],
        "LowerLegs" => vec![LOWERLEG_L, LOWERLEG_R],
        "UpperLegs" => vec![UPPERLEG_L, UPPERLEG_R],
        "LowerArms" => vec![FOREARM_L, FOREARM_R],
        "UpperArms" => vec![UPPERARM_L, UPPERARM_R],
        "Hand_L" => vec![HAND_L], "Hand_R" => vec![HAND_R],
        "ForeArm_L" => vec![FOREARM_L], "ForeArm_R" => vec![FOREARM_R],
        "UpperArm_L" => vec![UPPERARM_L], "UpperArm_R" => vec![UPPERARM_R],
        "UpperLeg_L" => vec![UPPERLEG_L], "UpperLeg_R" => vec![UPPERLEG_R],
        "LowerLeg_L" => vec![LOWERLEG_L], "LowerLeg_R" => vec![LOWERLEG_R],
        "Foot_L" => vec![FOOT_L], "Foot_R" => vec![FOOT_R],
        _ => vec![],
    }
}

/// `Temperature.getTrueInsulationValue` / `getTrueWindresistanceValue`.
pub fn true_insulation(i: f32) -> f32 { i * 2.0 + 0.5 * i * i * i }
pub fn true_windresistance(w: f32) -> f32 { w + 0.5 * w * w }

#[derive(Debug, Clone, PartialEq)]
pub struct Garment {
    /// Parts it covers (see [`covered_parts`]).
    pub parts: Vec<usize>,
    pub insulation: f32,
    pub wind_resistance: f32,
    /// Item wetness 0..=100 and condition 0..=100.
    pub wetness: f32,
    pub condition: f32,
}

impl Garment {
    pub fn new(blood_location: &str, insulation: f32, wind_resistance: f32) -> Garment {
        Garment { parts: covered_parts(blood_location), insulation, wind_resistance, wetness: 0.0, condition: 100.0 }
    }
}

/// Per-part `(insulation, windresist)` of everything worn: each layer adds its insulation, thinned by wetness
/// (up to 75%) and wear (down to half), plus 0.05 per layer.
pub fn node_values(worn: &[Garment]) -> ([f32; PARTS], [f32; PARTS]) {
    let (mut ins, mut wind) = ([0.0; PARTS], [0.0; PARTS]);
    let mut layers = [0u32; PARTS];
    for g in worn {
        let wet = (g.wetness * 0.01).clamp(0.0, 1.0);
        let cond = 0.5 + 0.5 * (g.condition * 0.01).clamp(0.0, 1.0);
        for &k in &g.parts {
            layers[k] += 1;
            ins[k] += true_insulation(g.insulation) * (1.0 - wet * 0.75) * cond;
            wind[k] += true_windresistance(g.wind_resistance) * (1.0 - wet * 0.45) * cond;
        }
    }
    for k in 0..PARTS {
        ins[k] += layers[k] as f32 * 0.05;
        wind[k] += layers[k] as f32 * 0.05;
    }
    (ins, wind)
}
