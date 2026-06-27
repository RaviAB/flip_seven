use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::{
    ActionChoice, Card, DealOutcome, DrawOdds, GameState, PendingAction, PlayerId, SpecialAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyKind {
    MaxRoundEv,
    MaxWinProbability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerController {
    Human,
    Ai(StrategyKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiDecision {
    Stay,
    Draw,
    ChooseTarget(PlayerId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionContext {
    pub player_id: PlayerId,
    pub pending_action: Option<PendingAction>,
    pub legal_targets: Vec<PlayerId>,
    pub draw_odds: Option<DrawOdds>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrategyRecommendation {
    pub strategy: StrategyKind,
    pub decision: Option<AiDecision>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationSettings {
    pub rollout_count: usize,
    pub decision_rollouts: usize,
    pub rng_seed: u64,
    pub target_score: u32,
    pub max_rounds: u32,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            rollout_count: 100,
            decision_rollouts: 24,
            rng_seed: 7,
            target_score: 200,
            max_rounds: 40,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationSummary {
    pub strategy: StrategyKind,
    pub matches: usize,
    pub wins: usize,
    pub win_rate: f64,
    pub average_final_score: f64,
    pub average_rounds_to_finish: f64,
    pub bust_rate: f64,
    pub average_pre_draw_bust_risk: f64,
    pub flip_seven_rate: f64,
    pub average_points_per_round: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrategyReport {
    pub settings: SimulationSettings,
    pub summaries: Vec<SimulationSummary>,
}

#[derive(Debug, Clone, Copy)]
struct StrategyStats {
    strategy: StrategyKind,
    matches: usize,
    wins: usize,
    final_score_sum: u64,
    rounds_sum: u64,
    busted_sum: u64,
    completed_rounds_sum: u64,
    bust_risk_basis_points_sum: u64,
    bust_risk_samples: u64,
    flip_seven_sum: u64,
}

impl StrategyStats {
    fn new(strategy: StrategyKind) -> Self {
        Self {
            strategy,
            matches: 0,
            wins: 0,
            final_score_sum: 0,
            rounds_sum: 0,
            busted_sum: 0,
            completed_rounds_sum: 0,
            bust_risk_basis_points_sum: 0,
            bust_risk_samples: 0,
            flip_seven_sum: 0,
        }
    }

    fn summary(self) -> SimulationSummary {
        let matches = self.matches.max(1) as f64;
        let completed_rounds = self.completed_rounds_sum.max(1) as f64;
        let bust_risk_samples = self.bust_risk_samples.max(1) as f64;

        SimulationSummary {
            strategy: self.strategy,
            matches: self.matches,
            wins: self.wins,
            win_rate: self.wins as f64 / matches,
            average_final_score: self.final_score_sum as f64 / matches,
            average_rounds_to_finish: self.rounds_sum as f64 / matches,
            bust_rate: self.busted_sum as f64 / completed_rounds,
            average_pre_draw_bust_risk: self.bust_risk_basis_points_sum as f64
                / bust_risk_samples
                / 10_000.0,
            flip_seven_rate: self.flip_seven_sum as f64 / completed_rounds,
            average_points_per_round: self.final_score_sum as f64 / completed_rounds,
        }
    }
}

pub fn recommend_decision(
    game: &GameState,
    strategy: StrategyKind,
    settings: SimulationSettings,
) -> StrategyRecommendation {
    let Some(context) = game.decision_context() else {
        return StrategyRecommendation {
            strategy,
            decision: None,
            rationale: "No legal AI decision is pending.".to_owned(),
        };
    };

    let decision = match strategy {
        StrategyKind::MaxRoundEv => max_round_ev_decision(game, &context, settings),
        StrategyKind::MaxWinProbability => max_win_probability_decision(game, &context, settings),
    };

    StrategyRecommendation {
        strategy,
        decision: decision.decision,
        rationale: decision.rationale,
    }
}

pub fn compare_strategies(settings: SimulationSettings) -> StrategyReport {
    let settings = SimulationSettings {
        rollout_count: settings.rollout_count.max(1),
        decision_rollouts: settings.decision_rollouts.max(1),
        target_score: settings.target_score.max(1),
        max_rounds: settings.max_rounds.max(1),
        ..settings
    };
    let mut rng = StdRng::seed_from_u64(settings.rng_seed);
    let mut ev_stats = StrategyStats::new(StrategyKind::MaxRoundEv);
    let mut win_stats = StrategyStats::new(StrategyKind::MaxWinProbability);

    for _ in 0..settings.rollout_count {
        let seed = rng.random::<u64>();
        let mut match_rng = StdRng::seed_from_u64(seed);
        let result = simulate_match(
            2,
            &[StrategyKind::MaxRoundEv, StrategyKind::MaxWinProbability],
            settings,
            &mut match_rng,
        );

        record_match(&mut ev_stats, &result, PlayerId::new(0));
        record_match(&mut win_stats, &result, PlayerId::new(1));
    }

    StrategyReport {
        settings,
        summaries: vec![ev_stats.summary(), win_stats.summary()],
    }
}

fn record_match(stats: &mut StrategyStats, result: &SimulatedMatch, player_id: PlayerId) {
    let Some(score) = result.game.player_score(player_id) else {
        return;
    };

    stats.matches += 1;
    stats.wins += usize::from(result.winner == Some(player_id));
    stats.final_score_sum += score.total_score as u64;
    stats.rounds_sum += result.rounds_played as u64;
    stats.busted_sum += score.busted_count as u64;
    stats.completed_rounds_sum += score.rounds_completed as u64;
    stats.bust_risk_basis_points_sum += score.bust_risk_sum_basis_points;
    stats.bust_risk_samples += score.bust_risk_sample_count as u64;
    stats.flip_seven_sum += score.flip_seven_count as u64;
}

#[derive(Debug, Clone)]
struct DecisionWithRationale {
    decision: Option<AiDecision>,
    rationale: String,
}

fn max_round_ev_decision(
    game: &GameState,
    context: &DecisionContext,
    settings: SimulationSettings,
) -> DecisionWithRationale {
    if let Some(pending_action) = context.pending_action.as_ref()
        && pending_action.needs_target()
    {
        return target_by_rollout_value(game, context, StrategyKind::MaxRoundEv, settings);
    }

    let Some(odds) = context.draw_odds.as_ref() else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No draw odds are available.".to_owned(),
        };
    };

    if odds.expected_score_after_draw > odds.current_score as f64 {
        DecisionWithRationale {
            decision: Some(AiDecision::Draw),
            rationale: format!(
                "Draw: next-card EV {:.2} beats staying at {} by {:.2}.",
                odds.expected_score_after_draw, odds.current_score, odds.expected_score_delta
            ),
        }
    } else {
        DecisionWithRationale {
            decision: Some(AiDecision::Stay),
            rationale: format!(
                "Stay: bank {} instead of next-card EV {:.2}.",
                odds.current_score, odds.expected_score_after_draw
            ),
        }
    }
}

fn max_win_probability_decision(
    game: &GameState,
    context: &DecisionContext,
    settings: SimulationSettings,
) -> DecisionWithRationale {
    if let Some(pending_action) = context.pending_action.as_ref()
        && pending_action.needs_target()
    {
        return target_by_rollout_value(game, context, StrategyKind::MaxWinProbability, settings);
    }

    let legal = game.legal_ai_decisions();
    if legal.is_empty() {
        return DecisionWithRationale {
            decision: None,
            rationale: "No legal decision is available.".to_owned(),
        };
    }

    let mut best = None;
    for decision in legal {
        let value = estimate_decision_win_probability(game, context.player_id, decision, settings);
        if best.is_none_or(|(_, best_value)| value > best_value) {
            best = Some((decision, value));
        }
    }

    let Some((decision, value)) = best else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No legal decision is available.".to_owned(),
        };
    };

    DecisionWithRationale {
        decision: Some(decision),
        rationale: format!(
            "{}: estimated match win chance {:.1}% to {} points.",
            decision_label(decision),
            value * 100.0,
            settings.target_score
        ),
    }
}

fn target_by_rollout_value(
    game: &GameState,
    context: &DecisionContext,
    strategy: StrategyKind,
    settings: SimulationSettings,
) -> DecisionWithRationale {
    let Some(pending_action) = context.pending_action.as_ref() else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No target decision is pending.".to_owned(),
        };
    };

    let mut best = None;
    for target_id in &context.legal_targets {
        let decision = AiDecision::ChooseTarget(*target_id);
        let value = match strategy {
            StrategyKind::MaxRoundEv => {
                estimate_target_round_value(game, context.player_id, decision, settings)
            }
            StrategyKind::MaxWinProbability => {
                estimate_decision_win_probability(game, context.player_id, decision, settings)
            }
        };
        if best.is_none_or(|(_, best_value)| value > best_value) {
            best = Some((decision, value));
        }
    }

    let Some((decision, value)) = best else {
        return DecisionWithRationale {
            decision: None,
            rationale: "No active legal targets are available.".to_owned(),
        };
    };

    let target_name = match decision {
        AiDecision::ChooseTarget(player_id) => player_name(game, player_id),
        AiDecision::Stay | AiDecision::Draw => "-".to_owned(),
    };
    let basis = match (strategy, pending_action.action()) {
        (StrategyKind::MaxRoundEv, SpecialAction::Freeze) => {
            format!("projected score value {value:.1}")
        }
        (StrategyKind::MaxRoundEv, SpecialAction::FlipThree) => {
            format!("projected round value {value:.1}")
        }
        (StrategyKind::MaxWinProbability, _) => {
            format!("estimated win chance {:.1}%", value * 100.0)
        }
    };

    DecisionWithRationale {
        decision: Some(decision),
        rationale: format!(
            "Choose {target_name} for {}: {basis}.",
            special_action_label(pending_action.action())
        ),
    }
}

fn estimate_decision_win_probability(
    game: &GameState,
    player_id: PlayerId,
    decision: AiDecision,
    settings: SimulationSettings,
) -> f64 {
    let mut wins = 0usize;

    for index in 0..settings.decision_rollouts.max(1) {
        let mut rng = StdRng::seed_from_u64(
            settings.rng_seed
                ^ (index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ ((player_id.index() as u64) << 32),
        );
        let mut candidate = game.clone_for_simulation();
        apply_simulated_decision(&mut candidate, decision, &mut rng);
        let result = finish_match_from_state(candidate, settings, &mut rng);
        if result.winner == Some(player_id) {
            wins += 1;
        }
    }

    wins as f64 / settings.decision_rollouts.max(1) as f64
}

fn estimate_target_round_value(
    game: &GameState,
    player_id: PlayerId,
    decision: AiDecision,
    settings: SimulationSettings,
) -> f64 {
    let mut value = 0.0;

    for index in 0..settings.decision_rollouts.max(1) {
        let mut rng = StdRng::seed_from_u64(
            settings.rng_seed
                ^ (index as u64 + 11).wrapping_mul(0xA24B_AED4_963E_E407)
                ^ ((player_id.index() as u64) << 28),
        );
        let mut candidate = game.clone_for_simulation();
        apply_simulated_decision(&mut candidate, decision, &mut rng);
        finish_round_with_policy(&mut candidate, &mut rng);
        if let Some(score) = candidate.player_score(player_id) {
            value += score.total_score as f64 + score.current_round_score.unwrap_or(0) as f64;
        }
    }

    value / settings.decision_rollouts.max(1) as f64
}

#[derive(Debug, Clone)]
struct SimulatedMatch {
    game: GameState,
    winner: Option<PlayerId>,
    rounds_played: u32,
}

fn simulate_match(
    player_count: usize,
    strategies: &[StrategyKind],
    settings: SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    let game = GameState::with_deck(player_count, super::Deck::new_full_ordered());
    finish_match_with_strategies(game, strategies, settings, rng)
}

fn finish_match_from_state(
    game: GameState,
    settings: SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    finish_match_with_basic_policy(game, settings, rng)
}

fn finish_match_with_strategies(
    mut game: GameState,
    strategies: &[StrategyKind],
    settings: SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    let mut rounds_played = game
        .score_board()
        .iter()
        .map(|score| score.rounds_completed)
        .max()
        .unwrap_or(0) as u32;

    while winner(&game, settings.target_score).is_none() && rounds_played < settings.max_rounds {
        play_round_with_strategies(&mut game, strategies, settings, rng);
        if game.round_over() {
            rounds_played = game
                .score_board()
                .iter()
                .map(|score| score.rounds_completed)
                .max()
                .unwrap_or(rounds_played as usize) as u32;
        }

        if winner(&game, settings.target_score).is_some() || rounds_played >= settings.max_rounds {
            break;
        }

        let _ = game.start_next_round();
    }

    let winner = winner(&game, settings.target_score).or_else(|| highest_score_player(&game));
    SimulatedMatch {
        game,
        winner,
        rounds_played,
    }
}

fn play_round_with_strategies(
    game: &mut GameState,
    strategies: &[StrategyKind],
    settings: SimulationSettings,
    rng: &mut StdRng,
) {
    let mut guard = 0usize;
    while !game.round_over() && guard < 1_000 {
        guard += 1;

        if game
            .pending_action()
            .is_some_and(PendingAction::awaiting_selected_draws)
        {
            deal_random_available_card(game, rng);
            continue;
        }

        let Some(player_id) = game.current_ai_player_id() else {
            break;
        };
        let strategy = strategies
            .get(player_id.index())
            .copied()
            .unwrap_or(StrategyKind::MaxRoundEv);
        let recommendation = recommend_decision(game, strategy, settings);

        match recommendation.decision {
            Some(AiDecision::Stay) => {
                let _ = game.stay_current_player();
            }
            Some(AiDecision::Draw) => deal_random_available_card(game, rng),
            Some(AiDecision::ChooseTarget(target_id)) => {
                let _ = game.resolve_pending_action(ActionChoice::Player(target_id));
            }
            None => break,
        }
    }
}

fn finish_round_with_policy(game: &mut GameState, rng: &mut StdRng) {
    play_round_with_basic_policy(game, rng);
}

fn finish_match_with_basic_policy(
    mut game: GameState,
    settings: SimulationSettings,
    rng: &mut StdRng,
) -> SimulatedMatch {
    let mut rounds_played = game
        .score_board()
        .iter()
        .map(|score| score.rounds_completed)
        .max()
        .unwrap_or(0) as u32;

    while winner(&game, settings.target_score).is_none() && rounds_played < settings.max_rounds {
        play_round_with_basic_policy(&mut game, rng);
        if game.round_over() {
            rounds_played = game
                .score_board()
                .iter()
                .map(|score| score.rounds_completed)
                .max()
                .unwrap_or(rounds_played as usize) as u32;
        }

        if winner(&game, settings.target_score).is_some() || rounds_played >= settings.max_rounds {
            break;
        }

        let _ = game.start_next_round();
    }

    let winner = winner(&game, settings.target_score).or_else(|| highest_score_player(&game));
    SimulatedMatch {
        game,
        winner,
        rounds_played,
    }
}

fn play_round_with_basic_policy(game: &mut GameState, rng: &mut StdRng) {
    let mut guard = 0usize;
    while !game.round_over() && guard < 1_000 {
        guard += 1;

        if game
            .pending_action()
            .is_some_and(PendingAction::awaiting_selected_draws)
        {
            deal_random_available_card(game, rng);
            continue;
        }

        if let Some(pending_action) = game.pending_action()
            && pending_action.needs_target()
        {
            if let Some(target_id) = game.legal_active_targets().first().copied() {
                let _ = game.resolve_pending_action(ActionChoice::Player(target_id));
                continue;
            }
            break;
        }

        let Some(context) = game.decision_context() else {
            break;
        };
        let Some(odds) = context.draw_odds else {
            break;
        };
        if odds.expected_score_after_draw > odds.current_score as f64 {
            deal_random_available_card(game, rng);
        } else {
            let _ = game.stay_current_player();
        }
    }
}

fn apply_simulated_decision(game: &mut GameState, decision: AiDecision, rng: &mut StdRng) {
    match decision {
        AiDecision::Stay => {
            let _ = game.stay_current_player();
        }
        AiDecision::Draw => deal_random_available_card(game, rng),
        AiDecision::ChooseTarget(player_id) => {
            let _ = game.resolve_pending_action(ActionChoice::Player(player_id));
        }
    }
}

fn deal_random_available_card(game: &mut GameState, rng: &mut StdRng) {
    let Some(card) = choose_random_card(game, rng) else {
        return;
    };
    let outcome = game.deal_selected_card(card);
    if matches!(
        outcome,
        DealOutcome::DeckEmpty | DealOutcome::SelectedCardUnavailable
    ) {
        let _ = game.stay_current_player();
    }
}

fn choose_random_card(game: &GameState, rng: &mut StdRng) -> Option<Card> {
    let counts = game.next_draw_pool_counts();
    let total = counts.iter().map(|(_, count)| *count).sum::<usize>();
    if total == 0 {
        return None;
    }

    let mut choice = rng.random_range(0..total);
    for (card, count) in counts {
        if choice < count {
            return Some(card);
        }
        choice -= count;
    }

    None
}

fn winner(game: &GameState, target_score: u32) -> Option<PlayerId> {
    game.score_board()
        .iter()
        .filter(|score| score.total_score >= target_score)
        .max_by_key(|score| score.total_score)
        .map(|score| score.player_id)
}

fn highest_score_player(game: &GameState) -> Option<PlayerId> {
    game.score_board()
        .iter()
        .max_by_key(|score| score.total_score)
        .map(|score| score.player_id)
}

fn player_name(game: &GameState, player_id: PlayerId) -> String {
    game.players()
        .iter()
        .find(|player| player.id() == player_id)
        .map(|player| player.name().to_owned())
        .unwrap_or_else(|| format!("Player {}", player_id.display_number()))
}

fn decision_label(decision: AiDecision) -> &'static str {
    match decision {
        AiDecision::Stay => "Stay",
        AiDecision::Draw => "Draw",
        AiDecision::ChooseTarget(_) => "Choose target",
    }
}

fn special_action_label(action: SpecialAction) -> &'static str {
    match action {
        SpecialAction::FlipThree => "Flip Three",
        SpecialAction::Freeze => "Freeze",
    }
}

pub fn strategy_label(strategy: StrategyKind) -> &'static str {
    match strategy {
        StrategyKind::MaxRoundEv => "EV AI",
        StrategyKind::MaxWinProbability => "Win% AI",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BonusCard, Deck, PlayerStatus};

    fn game_with_draw_order(player_count: usize, cards: Vec<Card>) -> GameState {
        GameState::with_deck(player_count, Deck::from_draw_order(cards))
    }

    #[test]
    fn ev_ai_stays_when_current_score_beats_draw_ev() {
        let mut game = game_with_draw_order(1, vec![Card::Number(12)]);
        game.deal_selected_card(Card::Number(12));
        game.replace_deck_for_test(Deck::from_draw_order([Card::Number(12), Card::Number(1)]));

        let recommendation = recommend_decision(
            &game,
            StrategyKind::MaxRoundEv,
            SimulationSettings::default(),
        );

        assert_eq!(recommendation.decision, Some(AiDecision::Stay));
    }

    #[test]
    fn ev_ai_draws_when_draw_ev_beats_current_score() {
        let mut game = game_with_draw_order(1, vec![Card::Number(1)]);
        game.deal_selected_card(Card::Number(1));
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(10),
            Card::Bonus(BonusCard::Plus(10)),
            Card::SecondChance,
        ]));

        let recommendation = recommend_decision(
            &game,
            StrategyKind::MaxRoundEv,
            SimulationSettings::default(),
        );

        assert_eq!(recommendation.decision, Some(AiDecision::Draw));
    }

    #[test]
    fn target_selection_uses_only_active_legal_targets() {
        let mut game = game_with_draw_order(3, vec![Card::Freeze]);
        game.deal_selected_card(Card::Freeze);
        game.force_bust_for_test(PlayerId::new(2));

        let recommendation = recommend_decision(
            &game,
            StrategyKind::MaxRoundEv,
            SimulationSettings::default(),
        );

        let Some(AiDecision::ChooseTarget(target_id)) = recommendation.decision else {
            panic!("expected target decision");
        };
        assert_ne!(target_id, PlayerId::new(2));
        assert!(game.legal_active_targets().contains(&target_id));
    }

    #[test]
    fn flip_three_ai_target_selection_keeps_card_selection_manual_in_live_mode() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
            ],
        );
        game.deal_selected_card(Card::FlipThree);

        let recommendation = recommend_decision(
            &game,
            StrategyKind::MaxRoundEv,
            SimulationSettings::default(),
        );
        let Some(decision @ AiDecision::ChooseTarget(_)) = recommendation.decision else {
            panic!("expected target decision");
        };
        let outcome = game.apply_ai_decision(decision);

        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected { .. }
        ));
        assert!(
            game.pending_action()
                .is_some_and(PendingAction::awaiting_selected_draws)
        );
        assert_eq!(game.players()[1].hand().len(), 0);
    }

    #[test]
    fn simulations_are_reproducible_with_fixed_seed() {
        let settings = SimulationSettings {
            rollout_count: 8,
            decision_rollouts: 2,
            rng_seed: 42,
            target_score: 50,
            max_rounds: 10,
        };

        let first = compare_strategies(settings);
        let second = compare_strategies(settings);

        assert_eq!(first, second);
    }

    #[test]
    fn simulation_summary_counts_matches_and_labels() {
        let settings = SimulationSettings {
            rollout_count: 4,
            decision_rollouts: 1,
            rng_seed: 11,
            target_score: 40,
            max_rounds: 8,
        };

        let report = compare_strategies(settings);

        assert_eq!(report.summaries.len(), 2);
        assert_eq!(report.summaries[0].strategy, StrategyKind::MaxRoundEv);
        assert_eq!(
            report.summaries[1].strategy,
            StrategyKind::MaxWinProbability
        );
        assert_eq!(report.summaries[0].matches, 4);
        assert_eq!(report.summaries[1].matches, 4);
        assert!(report.summaries[0].average_rounds_to_finish > 0.0);
    }

    #[test]
    fn win_probability_ai_can_choose_different_action_than_ev_ai() {
        let mut game = game_with_draw_order(1, vec![Card::Number(1)]);
        game.deal_selected_card(Card::Number(1));
        game.set_total_score_for_test(PlayerId::new(0), 199);
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(10),
            Card::Number(11),
            Card::Number(12),
        ]));
        let settings = SimulationSettings {
            rollout_count: 4,
            decision_rollouts: 4,
            rng_seed: 3,
            target_score: 200,
            max_rounds: 6,
        };

        let ev = recommend_decision(&game, StrategyKind::MaxRoundEv, settings);
        let win = recommend_decision(&game, StrategyKind::MaxWinProbability, settings);

        assert_eq!(ev.decision, Some(AiDecision::Draw));
        assert_eq!(win.decision, Some(AiDecision::Stay));
        assert_eq!(game.players()[0].status(), PlayerStatus::Active);
    }
}
