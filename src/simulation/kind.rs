use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StrategyKind {
    Conservative,
    Balanced,
    Aggressive,
    MaxRoundEv,
    MaxWinProbability,
    MaxWinStatic,
    StayAtNumberCount(u8),
    StayAtScore(u32),
}

impl FromStr for StrategyKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = normalize_strategy_name(value);
        match normalized.as_str() {
            "conservative" => Ok(Self::Conservative),
            "balanced" => Ok(Self::Balanced),
            "aggressive" => Ok(Self::Aggressive),
            "maxroundev" | "roundev" | "ev" | "maxev" => Ok(Self::MaxRoundEv),
            "maxwinprobability" | "winprobability" | "winprob" | "win" => {
                Ok(Self::MaxWinProbability)
            }
            "maxwinstatic" | "winstatic" | "staticwin" => Ok(Self::MaxWinStatic),
            _ => parse_simple_strategy(&normalized)
                .ok_or_else(|| format!("unknown strategy '{value}'")),
        }
    }
}

impl StrategyKind {
    pub fn label(self) -> String {
        let strategy = self;
        match strategy {
            StrategyKind::Conservative => "Conservative".to_owned(),
            StrategyKind::Balanced => "Balanced".to_owned(),
            StrategyKind::Aggressive => "Aggressive".to_owned(),
            StrategyKind::MaxRoundEv => "MaxRoundEV".to_owned(),
            StrategyKind::MaxWinProbability => "MaxWin%".to_owned(),
            StrategyKind::MaxWinStatic => "MaxWinStatic".to_owned(),
            StrategyKind::StayAtNumberCount(threshold) => format!("Stay{threshold}Cards"),
            StrategyKind::StayAtScore(threshold) => format!("Stay{threshold}"),
        }
    }

    pub fn slug(self) -> String {
        let strategy = self;
        match strategy {
            StrategyKind::Conservative => "conservative".to_owned(),
            StrategyKind::Balanced => "balanced".to_owned(),
            StrategyKind::Aggressive => "aggressive".to_owned(),
            StrategyKind::MaxRoundEv => "max-round-ev".to_owned(),
            StrategyKind::MaxWinProbability => "max-win-probability".to_owned(),
            StrategyKind::MaxWinStatic => "max-win-static".to_owned(),
            StrategyKind::StayAtNumberCount(threshold) => format!("stay-at-{threshold}-cards"),
            StrategyKind::StayAtScore(threshold) => format!("stay-at-{threshold}"),
        }
    }

    pub fn catalog() -> Vec<StrategyKind> {
        let mut strategies = vec![
            StrategyKind::Conservative,
            StrategyKind::Balanced,
            StrategyKind::Aggressive,
            StrategyKind::MaxRoundEv,
            StrategyKind::MaxWinProbability,
            StrategyKind::MaxWinStatic,
            StrategyKind::StayAtNumberCount(3),
            StrategyKind::StayAtNumberCount(4),
        ];
        strategies.extend((15..=35).map(StrategyKind::StayAtScore));
        strategies
    }

    pub fn human_sweep_catalog() -> Vec<StrategyKind> {
        Self::catalog()
    }
}

fn normalize_strategy_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn parse_simple_strategy(normalized: &str) -> Option<StrategyKind> {
    if let Some(number) = normalized
        .strip_prefix("stayat")
        .and_then(|suffix| suffix.strip_suffix("cards"))
        .and_then(|number| number.parse::<u8>().ok())
        && matches!(number, 3 | 4)
    {
        return Some(StrategyKind::StayAtNumberCount(number));
    }

    if let Some(score) = normalized
        .strip_prefix("stayat")
        .and_then(|number| number.parse::<u32>().ok())
        && (15..=35).contains(&score)
    {
        return Some(StrategyKind::StayAtScore(score));
    }

    None
}

pub(super) fn unique_strategies(strategies: &[StrategyKind]) -> Vec<StrategyKind> {
    let mut unique = Vec::new();
    for strategy in strategies {
        if !unique.contains(strategy) {
            unique.push(*strategy);
        }
    }
    unique
}

pub(super) fn strategy_sort_key(strategy: StrategyKind) -> usize {
    StrategyKind::catalog()
        .iter()
        .position(|candidate| *candidate == strategy)
        .unwrap_or(usize::MAX)
}
