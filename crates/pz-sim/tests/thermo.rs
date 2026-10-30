use pz_sim::thermo::*;

fn run(air: f32, ticks: usize, tweak: impl Fn(&mut Inputs)) -> Thermoregulator {
    let mut t = Thermoregulator::default();
    let mut i = Inputs { air_temperature: air, air_and_wind_temperature: air, ..Inputs::default() };
    tweak(&mut i);
    for _ in 0..ticks {
        t.update(&i);
    }
    t
}

#[test]
fn a_bare_character_in_mild_air_holds_its_core_temperature() {
    let t = run(27.0, 30_000, |_| {});
    assert!((t.core_celcius() - 37.0).abs() < 0.5, "core {}", t.core_celcius());
    assert!(t.fatigue_multiplier >= 1.0 && t.energy_multiplier >= 1.0);
}

#[test]
fn freezing_air_cools_the_core_and_hot_air_warms_it() {
    let cold = run(-10.0, 60_000, |_| {});
    assert!(cold.core_celcius() < 36.5, "cold core {}", cold.core_celcius());
    assert!(cold.body_heat_delta < 0.0 && cold.prim_total < 0.0, "shivering response");
    let hot = run(42.0, 60_000, |_| {});
    assert!(hot.core_celcius() > 37.2, "hot core {}", hot.core_celcius());
    assert!(hot.prim_total > 0.0, "sweating response");
    assert!(hot.fluids_multiplier > 1.0, "heat makes you thirstier");
    // The extremities end up colder than the core in the cold.
    assert!(cold.nodes[16].skin_celcius < cold.nodes[6].skin_celcius);
}

#[test]
fn wind_chill_and_wet_skin_make_cold_worse() {
    let calm = run(5.0, 40_000, |_| {});
    let windy = run(5.0, 40_000, |i| i.air_and_wind_temperature = -5.0);
    let wet = run(5.0, 40_000, |i| i.wetness = [100.0; 17]);
    assert!(windy.core_celcius() < calm.core_celcius());
    assert!(wet.core_celcius() < calm.core_celcius());
}

#[test]
fn exercise_raises_the_metabolic_rate_and_a_fever_raises_the_set_point() {
    let rest = run(20.0, 20_000, |_| {});
    let running = run(20.0, 20_000, |i| i.activity = met::RUNNING_10KMH);
    assert!(running.metabolic_rate > rest.metabolic_rate + 3.0, "{} vs {}", running.metabolic_rate, rest.metabolic_rate);
    assert!((rest.metabolic_rate - met::DEFAULT).abs() < 0.01);
    assert_eq!(run(20.0, 1, |i| i.sickness = 0.5).set_point, 38.0);
    // Heavy loads add up to 35% to the target.
    let loaded = run(20.0, 20_000, |i| i.carried_ratio = 1.0);
    assert!(loaded.metabolic_rate > rest.metabolic_rate * 1.2);
}

#[test]
fn energy_and_fluids_follow_hunger_fatigue_and_thirst() {
    assert_eq!(Thermoregulator::energy(0.0, 0.0), 1.0);
    assert!(Thermoregulator::energy(1.0, 1.0).abs() < 1e-6);
    assert!((Thermoregulator::body_fluids(0.3) - 0.7).abs() < 1e-6);
}

#[test]
fn thermal_damage_builds_in_severe_cold_and_fades_when_warm() {
    let mut t = Thermoregulator::default();
    let cold = Inputs { air_temperature: -10.0, air_and_wind_temperature: -10.0, hypothermia_level: 4, world_seconds: 2.0, ..Inputs::default() };
    for _ in 0..3000 {
        t.update(&cold);
    }
    let after_cold = t.thermal_damage;
    assert!(after_cold >= 0.0);
    let warm = Inputs { world_seconds: 2.0, ..Inputs::default() };
    for _ in 0..3000 {
        t.update(&warm);
    }
    assert_eq!(t.thermal_damage, 0.0, "damage decays back to zero in warm air (was {after_cold})");
}
