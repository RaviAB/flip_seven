use super::settings;
use crate::simulation::{SimulationConfig, SimulationError, run_simulation};

#[test]
fn invalid_settings_are_rejected_instead_of_normalized() {
    assert_eq!(
        run_simulation(SimulationConfig {
            rollout_count: 0,
            ..settings()
        }),
        Err(SimulationError::ZeroRollouts)
    );
    assert_eq!(
        run_simulation(SimulationConfig {
            strategies: Vec::new(),
            ..settings()
        }),
        Err(SimulationError::EmptyStrategies)
    );
    assert!(matches!(
        run_simulation(SimulationConfig {
            min_matches: Some(10),
            max_matches: Some(5),
            win_ci_width: Some(0.1),
            ..settings()
        }),
        Err(SimulationError::InvalidMatchRange { .. })
    ));
}
