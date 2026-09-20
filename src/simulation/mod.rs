mod comparison;
mod config;
mod decision;
mod kind;
mod monte_carlo;
mod report;
mod simulator;
mod static_equity;

use comparison::compare;
use config::validate_config;

pub use config::{ComparisonMode, SimulationConfig, SimulationError};
pub use kind::StrategyKind;
pub use report::{SimulationReport, SimulationSummary};

pub fn run_simulation(config: SimulationConfig) -> Result<SimulationReport, SimulationError> {
    validate_config(&config)?;
    compare(config).map_err(|error| SimulationError::ExecutionFailed {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests;
