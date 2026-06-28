pub mod card;
pub mod deck;
pub mod game;
pub mod player;
pub mod strategy;

pub use card::{BonusCard, Card};
pub use deck::Deck;
pub use game::{
    ActionChoice, BustCardDetail, DealOutcome, DrawOdds, ExpectedValueDetail, GameState,
    PendingAction, PlayerScore, RoundOutcome, RoundScore, SpecialAction,
};
pub use player::{Player, PlayerId, PlayerStatus, ScoreBreakdown, round_score_for_cards};
pub use strategy::{
    AiDecision, DecisionContext, PlayerController, SimulationSettings, SimulationSummary,
    StrategyKind, StrategyRecommendation, StrategyReport, all_strategy_kinds,
    compare_head_to_head_strategies, compare_strategies, human_sweep_strategy_kinds,
    recommend_decision, strategy_label, strategy_slug,
};
