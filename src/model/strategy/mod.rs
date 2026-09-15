mod decision;
mod kind;
mod monte_carlo;
mod report;
mod simulator;
mod static_equity;

pub use decision::{AiDecision, DecisionContext, StrategyRecommendation, recommend_decision};
pub use kind::{
    PlayerController, StrategyKind, all_strategy_kinds, human_sweep_strategy_kinds, strategy_label,
    strategy_slug,
};
pub use report::{
    SimulationSettings, SimulationSummary, StrategyReport, compare_head_to_head_strategies,
    compare_strategies,
};

#[cfg(test)]
use decision::{legal_ai_decisions, win_probability_profile};
#[cfg(test)]
use monte_carlo::evaluate_monte_carlo_actions;
#[cfg(test)]
use simulator::simulation_clone;
#[cfg(test)]
use static_equity::{static_win_evaluation, static_win_features};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ActionChoice, BonusCard, Card, DealOutcome, Deck, GameState, PendingAction, PlayerId,
        PlayerStatus,
    };

    fn game_with_draw_order(player_count: usize, cards: Vec<Card>) -> GameState {
        GameState::with_deck(player_count, Deck::from_draw_order(cards))
    }

    fn settings() -> SimulationSettings {
        SimulationSettings {
            rollout_count: 8,
            decision_rollouts: 2,
            rng_seed: 42,
            target_score: 50,
            max_rounds: 10,
            player_count: 2,
            strategies: vec![StrategyKind::Conservative, StrategyKind::Balanced],
            mirrored_seating: true,
            parallel: false,
            min_matches: None,
            max_matches: None,
            win_ci_width: None,
        }
    }

    fn decision_context_for(game: &GameState, player_id: PlayerId) -> DecisionContext {
        DecisionContext {
            player_id,
            pending_action: game.pending_action().cloned(),
            legal_targets: game.legal_active_targets(),
            draw_odds: game.draw_odds_for_player(player_id),
        }
    }

    #[test]
    fn ev_ai_stays_when_current_score_beats_draw_ev() {
        let mut game = game_with_draw_order(1, vec![Card::Number(12)]);
        game.deal_selected_card(Card::Number(12));
        game.replace_deck_for_test(Deck::from_draw_order([Card::Number(12), Card::Number(1)]));

        let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings());

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

        let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings());

        assert_eq!(
            recommendation.decision,
            Some(AiDecision::Draw),
            "{}",
            recommendation.rationale
        );
    }

    #[test]
    fn every_strategy_produces_only_legal_decisions() {
        let mut game = game_with_draw_order(3, vec![Card::Number(5)]);
        game.deal_selected_card(Card::Number(5));

        for strategy in all_strategy_kinds() {
            let recommendation = recommend_decision(&game, strategy, &settings());
            if let Some(decision) = recommendation.decision {
                assert!(legal_ai_decisions(&game).contains(&decision));
            }
        }
    }

    #[test]
    fn conservative_stays_where_aggressive_draws() {
        let mut game = game_with_draw_order(1, vec![Card::Number(10)]);
        game.deal_selected_card(Card::Number(10));
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(10),
            Card::Number(11),
            Card::Number(12),
        ]));

        let conservative = recommend_decision(&game, StrategyKind::Conservative, &settings());
        let aggressive = recommend_decision(&game, StrategyKind::Aggressive, &settings());

        assert_eq!(conservative.decision, Some(AiDecision::Stay));
        assert_eq!(aggressive.decision, Some(AiDecision::Draw));
    }

    #[test]
    fn stay_at_number_count_uses_number_card_threshold() {
        let mut below = game_with_draw_order(1, vec![Card::Number(1), Card::Number(2)]);
        below.deal_selected_card(Card::Number(1));
        below.deal_selected_card(Card::Number(2));
        below.replace_deck_for_test(Deck::from_draw_order([Card::Number(3)]));

        let mut at_three =
            game_with_draw_order(1, vec![Card::Number(1), Card::Number(2), Card::Number(3)]);
        for card in [Card::Number(1), Card::Number(2), Card::Number(3)] {
            at_three.deal_selected_card(card);
        }
        at_three.replace_deck_for_test(Deck::from_draw_order([Card::Number(4)]));

        let mut at_four = game_with_draw_order(
            1,
            vec![
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
            ],
        );
        for card in [
            Card::Number(1),
            Card::Number(2),
            Card::Number(3),
            Card::Number(4),
        ] {
            at_four.deal_selected_card(card);
        }
        at_four.replace_deck_for_test(Deck::from_draw_order([Card::Number(5)]));

        assert_eq!(
            recommend_decision(&below, StrategyKind::StayAtNumberCount(3), &settings()).decision,
            Some(AiDecision::Draw)
        );
        assert_eq!(
            recommend_decision(&at_three, StrategyKind::StayAtNumberCount(3), &settings()).decision,
            Some(AiDecision::Stay)
        );
        assert_eq!(
            recommend_decision(&at_three, StrategyKind::StayAtNumberCount(4), &settings()).decision,
            Some(AiDecision::Draw)
        );
        assert_eq!(
            recommend_decision(&at_four, StrategyKind::StayAtNumberCount(4), &settings()).decision,
            Some(AiDecision::Stay)
        );
    }

    #[test]
    fn stay_at_score_uses_score_threshold() {
        for threshold in [15, 25, 35] {
            let below_bonus = (threshold - 13) as u8;
            let at_bonus = (threshold - 12) as u8;

            let mut below = game_with_draw_order(
                1,
                vec![Card::Number(12), Card::Bonus(BonusCard::Plus(below_bonus))],
            );
            below.deal_selected_card(Card::Number(12));
            below.deal_selected_card(Card::Bonus(BonusCard::Plus(below_bonus)));
            below.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

            let mut at_threshold = game_with_draw_order(
                1,
                vec![Card::Number(12), Card::Bonus(BonusCard::Plus(at_bonus))],
            );
            at_threshold.deal_selected_card(Card::Number(12));
            at_threshold.deal_selected_card(Card::Bonus(BonusCard::Plus(at_bonus)));
            at_threshold.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

            assert_eq!(
                recommend_decision(
                    &below,
                    StrategyKind::StayAtScore(threshold as u32),
                    &settings()
                )
                .decision,
                Some(AiDecision::Draw)
            );
            assert_eq!(
                recommend_decision(
                    &at_threshold,
                    StrategyKind::StayAtScore(threshold as u32),
                    &settings()
                )
                .decision,
                Some(AiDecision::Stay)
            );
        }
    }

    #[test]
    fn second_chance_overrides_simple_thresholds_unless_banking_wins() {
        let mut protected = game_with_draw_order(
            1,
            vec![
                Card::Number(10),
                Card::Number(11),
                Card::Number(12),
                Card::SecondChance,
            ],
        );
        for card in [
            Card::Number(10),
            Card::Number(11),
            Card::Number(12),
            Card::SecondChance,
        ] {
            protected.deal_selected_card(card);
        }
        protected.replace_deck_for_test(Deck::from_draw_order([Card::Number(1)]));

        let recommendation =
            recommend_decision(&protected, StrategyKind::StayAtNumberCount(3), &settings());
        assert_eq!(recommendation.decision, Some(AiDecision::Draw));

        let mut winning = simulation_clone(&protected);
        winning.set_total_score_for_test(PlayerId::new(0), 20);
        let target_settings = SimulationSettings {
            target_score: 50,
            ..settings()
        };
        let recommendation = recommend_decision(
            &winning,
            StrategyKind::StayAtNumberCount(3),
            &target_settings,
        );
        assert_eq!(recommendation.decision, Some(AiDecision::Stay));
    }

    #[test]
    fn strategy_parsing_accepts_human_simple_slugs() {
        assert_eq!(
            "max-win-static".parse::<StrategyKind>(),
            Ok(StrategyKind::MaxWinStatic)
        );

        assert_eq!(
            "stay-at-3-cards".parse::<StrategyKind>(),
            Ok(StrategyKind::StayAtNumberCount(3))
        );
        assert_eq!(
            "stay-at-4-cards".parse::<StrategyKind>(),
            Ok(StrategyKind::StayAtNumberCount(4))
        );

        for threshold in 15..=35 {
            let slug = format!("stay-at-{threshold}");
            assert_eq!(
                slug.parse::<StrategyKind>(),
                Ok(StrategyKind::StayAtScore(threshold))
            );
        }
    }

    #[test]
    fn win_probability_pressure_profiles_are_monotonic() {
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let behind = win_probability_profile(120, 40, &settings);
        let close = win_probability_profile(100, 100, &settings);
        let ahead = win_probability_profile(50, 120, &settings);

        assert!(behind.pressure > close.pressure);
        assert!(close.pressure > ahead.pressure);
        assert!(behind.bust_risk_cap > close.bust_risk_cap);
        assert!(close.bust_risk_cap > ahead.bust_risk_cap);
        assert!(behind.building_bust_risk_cap > close.building_bust_risk_cap);
        assert!(close.building_bust_risk_cap > ahead.building_bust_risk_cap);
        assert!(behind.flip_seven_risk_cap > close.flip_seven_risk_cap);
        assert!(close.flip_seven_risk_cap > ahead.flip_seven_risk_cap);
        assert!(ahead.required_ev_edge > close.required_ev_edge);
        assert!(close.required_ev_edge > behind.required_ev_edge);
    }

    #[test]
    fn static_features_use_all_opponents_and_player_count() {
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let mut two_player = game_with_draw_order(2, vec![Card::Number(8)]);
        two_player.deal_selected_card(Card::Number(8));
        two_player.set_total_score_for_test(PlayerId::new(0), 20);
        two_player.set_total_score_for_test(PlayerId::new(1), 10);
        two_player.replace_deck_for_test(Deck::from_draw_order([Card::Number(9)]));

        let mut four_player = game_with_draw_order(4, vec![Card::Number(8)]);
        four_player.deal_selected_card(Card::Number(8));
        four_player.set_total_score_for_test(PlayerId::new(0), 20);
        four_player.set_total_score_for_test(PlayerId::new(1), 10);
        four_player.set_total_score_for_test(PlayerId::new(2), 80);
        four_player.set_total_score_for_test(PlayerId::new(3), 90);
        four_player.replace_deck_for_test(Deck::from_draw_order([Card::Number(9)]));

        let two = static_win_features(&two_player, PlayerId::new(0), &settings).unwrap();
        let four = static_win_features(&four_player, PlayerId::new(0), &settings).unwrap();

        assert_eq!(two.player_count, 2);
        assert_eq!(four.player_count, 4);
        assert!(two.relative_rank_after_stay > four.relative_rank_after_stay);
        assert!(four.field_pressure > two.field_pressure);
        assert!(four.worst_margin_after_stay < two.worst_margin_after_stay);
    }

    #[test]
    fn static_endgame_urgency_increases_as_any_player_approaches_target() {
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let mut early = game_with_draw_order(3, vec![Card::Number(8)]);
        early.deal_selected_card(Card::Number(8));
        early.set_total_score_for_test(PlayerId::new(1), 40);
        early.set_total_score_for_test(PlayerId::new(2), 50);
        early.replace_deck_for_test(Deck::from_draw_order([Card::Number(9)]));

        let mut late = game_with_draw_order(3, vec![Card::Number(8)]);
        late.deal_selected_card(Card::Number(8));
        late.set_total_score_for_test(PlayerId::new(1), 190);
        late.set_total_score_for_test(PlayerId::new(2), 50);
        late.replace_deck_for_test(Deck::from_draw_order([Card::Number(9)]));

        let early_features = static_win_features(&early, PlayerId::new(0), &settings).unwrap();
        let late_features = static_win_features(&late, PlayerId::new(0), &settings).unwrap();

        assert!(late_features.endgame_urgency > early_features.endgame_urgency);
    }

    #[test]
    fn static_second_chance_increases_draw_equity() {
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let mut unsafe_game = game_with_draw_order(1, vec![Card::Number(5)]);
        unsafe_game.deal_selected_card(Card::Number(5));
        unsafe_game
            .replace_deck_for_test(Deck::from_draw_order([Card::Number(5), Card::Number(6)]));

        let mut protected_game = game_with_draw_order(1, vec![Card::Number(5), Card::SecondChance]);
        protected_game.deal_selected_card(Card::Number(5));
        protected_game.deal_selected_card(Card::SecondChance);
        protected_game
            .replace_deck_for_test(Deck::from_draw_order([Card::Number(5), Card::Number(6)]));

        let unsafe_eval = static_win_evaluation(
            &unsafe_game,
            &decision_context_for(&unsafe_game, PlayerId::new(0)),
            &settings,
        )
        .unwrap();
        let protected_eval = static_win_evaluation(
            &protected_game,
            &decision_context_for(&protected_game, PlayerId::new(0)),
            &settings,
        )
        .unwrap();

        assert!(!unsafe_eval.features.has_second_chance);
        assert!(protected_eval.features.has_second_chance);
        assert!(
            protected_eval.draw_equity - protected_eval.stay_equity
                > unsafe_eval.draw_equity - unsafe_eval.stay_equity
        );
    }

    #[test]
    fn static_flip_seven_upside_increases_draw_equity_when_close() {
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let dealt = [
            Card::Number(0),
            Card::Number(1),
            Card::Number(2),
            Card::Number(3),
            Card::Number(4),
            Card::Number(5),
        ];
        let mut flip_upside = game_with_draw_order(1, dealt.to_vec());
        for card in dealt {
            flip_upside.deal_selected_card(card);
        }
        flip_upside.replace_deck_for_test(Deck::from_draw_order([Card::Number(6)]));

        let mut bust_only = game_with_draw_order(1, dealt.to_vec());
        for card in dealt {
            bust_only.deal_selected_card(card);
        }
        bust_only.replace_deck_for_test(Deck::from_draw_order([Card::Number(0)]));

        let upside = static_win_evaluation(
            &flip_upside,
            &decision_context_for(&flip_upside, PlayerId::new(0)),
            &settings,
        )
        .unwrap();
        let bust = static_win_evaluation(
            &bust_only,
            &decision_context_for(&bust_only, PlayerId::new(0)),
            &settings,
        )
        .unwrap();

        assert_eq!(upside.features.flip_seven_distance, 1);
        assert!(upside.features.flip_seven_probability > bust.features.flip_seven_probability);
        assert!(upside.draw_equity > bust.draw_equity);
    }

    #[test]
    fn static_strategy_hard_rules_stay_at_target_and_draw_at_zero() {
        let mut winning = game_with_draw_order(1, vec![Card::Number(1)]);
        winning.deal_selected_card(Card::Number(1));
        winning.set_total_score_for_test(PlayerId::new(0), 49);
        winning.replace_deck_for_test(Deck::from_draw_order([Card::Number(12)]));
        let target_settings = SimulationSettings {
            target_score: 50,
            ..settings()
        };

        let recommendation =
            recommend_decision(&winning, StrategyKind::MaxWinStatic, &target_settings);
        assert_eq!(recommendation.decision, Some(AiDecision::Stay));
        assert!(recommendation.rationale.contains("reaches"));

        let zero = game_with_draw_order(1, vec![Card::Number(9)]);
        let recommendation = recommend_decision(&zero, StrategyKind::MaxWinStatic, &settings());
        assert_eq!(recommendation.decision, Some(AiDecision::Draw));
        assert!(recommendation.rationale.contains("no points"));
    }

    #[test]
    fn static_draw_stay_decision_does_not_use_decision_rollouts() {
        let mut game = game_with_draw_order(2, vec![Card::Number(8)]);
        game.deal_selected_card(Card::Number(8));
        game.set_total_score_for_test(PlayerId::new(0), 50);
        game.set_total_score_for_test(PlayerId::new(1), 120);
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(9),
            Card::Number(10),
            Card::Number(8),
        ]));
        let low_rollout_settings = SimulationSettings {
            target_score: 200,
            decision_rollouts: 1,
            ..settings()
        };
        let high_rollout_settings = SimulationSettings {
            decision_rollouts: 999,
            ..low_rollout_settings.clone()
        };

        let low = recommend_decision(&game, StrategyKind::MaxWinStatic, &low_rollout_settings);
        let high = recommend_decision(&game, StrategyKind::MaxWinStatic, &high_rollout_settings);

        assert_eq!(low.decision, high.decision);
        assert_eq!(low.rationale, high.rationale);
        assert!(!low.rationale.contains("Monte Carlo"));
    }

    fn two_player_game_with_player_zero_score_pressure(
        own_total: u32,
        opponent_total: u32,
    ) -> GameState {
        let mut game = game_with_draw_order(2, vec![Card::Number(8), Card::Number(12)]);
        game.deal_selected_card(Card::Number(8));
        game.stay_current_player();
        game.deal_selected_card(Card::Number(12));
        game.set_total_score_for_test(PlayerId::new(0), own_total);
        game.set_total_score_for_test(PlayerId::new(1), opponent_total);
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(8),
            Card::Number(10),
            Card::Number(11),
        ]));
        game
    }

    #[test]
    fn win_probability_draws_more_aggressively_when_behind() {
        let game = two_player_game_with_player_zero_score_pressure(50, 120);
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let recommendation = recommend_decision(&game, StrategyKind::MaxWinProbability, &settings);

        assert_eq!(recommendation.decision, Some(AiDecision::Draw));
        assert!(recommendation.rationale.contains("pressure"));
        assert!(recommendation.rationale.contains("leading opponent 120"));
    }

    #[test]
    fn win_probability_stays_more_conservatively_when_ahead() {
        let game = two_player_game_with_player_zero_score_pressure(120, 50);
        let settings = SimulationSettings {
            target_score: 200,
            ..settings()
        };
        let recommendation = recommend_decision(&game, StrategyKind::MaxWinProbability, &settings);

        assert_eq!(
            recommendation.decision,
            Some(AiDecision::Stay),
            "{}",
            recommendation.rationale
        );
        assert!(recommendation.rationale.contains("pressure -"));
        assert!(recommendation.rationale.contains("leading opponent 50"));
    }

    #[test]
    fn aggressive_draws_in_high_upside_flip_seven_state() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(0),
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
            ],
        );
        for card in [
            Card::Number(0),
            Card::Number(1),
            Card::Number(2),
            Card::Number(3),
            Card::Number(4),
            Card::Number(5),
        ] {
            game.deal_selected_card(card);
        }
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(6),
            Card::Number(7),
            Card::Number(8),
            Card::Number(0),
        ]));

        let recommendation = recommend_decision(&game, StrategyKind::Aggressive, &settings());

        assert_eq!(recommendation.decision, Some(AiDecision::Draw));
    }

    #[test]
    fn target_selection_uses_only_active_legal_targets() {
        let mut game = game_with_draw_order(3, vec![Card::Freeze]);
        game.deal_selected_card(Card::Freeze);
        game.force_bust_for_test(PlayerId::new(2));

        let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings());

        let Some(AiDecision::ChooseTarget(target_id)) = recommendation.decision else {
            panic!("expected target decision");
        };
        assert_ne!(target_id, PlayerId::new(2));
        assert!(game.legal_active_targets().contains(&target_id));
    }

    #[test]
    fn win_probability_target_selection_reports_monte_carlo_search() {
        let mut game = game_with_draw_order(3, vec![Card::Freeze]);
        game.deal_selected_card(Card::Freeze);

        let recommendation =
            recommend_decision(&game, StrategyKind::MaxWinProbability, &settings());

        let Some(AiDecision::ChooseTarget(target_id)) = recommendation.decision else {
            panic!("expected target decision");
        };
        assert!(game.legal_active_targets().contains(&target_id));
        assert!(recommendation.rationale.contains("Monte Carlo win"));
        assert!(recommendation.rationale.contains("samples"));
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

        let recommendation = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings());
        let Some(decision @ AiDecision::ChooseTarget(_)) = recommendation.decision else {
            panic!("expected target decision");
        };
        let outcome = match decision {
            AiDecision::ChooseTarget(player_id) => {
                game.resolve_pending_action(ActionChoice::Player(player_id))
            }
            AiDecision::Stay => game.stay_current_player(),
            AiDecision::Draw => panic!("target selection should not recommend draw"),
        };

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
        let first = compare_strategies(settings());
        let second = compare_strategies(settings());

        assert_eq!(first, second);
    }

    #[test]
    fn mirrored_seating_gives_equal_seat_exposure() {
        let report = compare_strategies(SimulationSettings {
            rollout_count: 3,
            player_count: 4,
            strategies: vec![
                StrategyKind::Conservative,
                StrategyKind::Balanced,
                StrategyKind::Aggressive,
                StrategyKind::MaxRoundEv,
            ],
            ..settings()
        });

        for summary in report.summaries {
            assert_eq!(summary.seat_exposure, vec![3, 3, 3, 3]);
        }
    }

    #[test]
    fn mixed_pool_comparison_seats_every_strategy() {
        let report = compare_strategies(SimulationSettings {
            rollout_count: 1,
            player_count: 4,
            strategies: vec![
                StrategyKind::MaxWinStatic,
                StrategyKind::MaxRoundEv,
                StrategyKind::Balanced,
                StrategyKind::Aggressive,
                StrategyKind::MaxWinProbability,
            ],
            decision_rollouts: 1,
            max_rounds: 4,
            parallel: false,
            ..settings()
        });

        assert_eq!(report.total_match_runs, 20);
        for summary in report.summaries {
            assert!(summary.matches > 0, "{:?}", summary.strategy);
        }
    }

    #[test]
    fn human_sweep_round_robin_gives_equal_mirrored_exposure() {
        let strategies = vec![
            StrategyKind::Balanced,
            StrategyKind::StayAtNumberCount(3),
            StrategyKind::StayAtScore(20),
        ];
        let report = compare_head_to_head_strategies(SimulationSettings {
            rollout_count: 3,
            player_count: 2,
            strategies,
            parallel: false,
            ..settings()
        });

        assert_eq!(report.total_match_runs, 18);
        for summary in report.summaries {
            assert_eq!(summary.matches, 12);
            assert_eq!(summary.seat_exposure, vec![6, 6]);
        }
    }

    #[test]
    fn parallel_and_single_thread_reports_match_for_same_seed() {
        let serial = compare_strategies(SimulationSettings {
            rollout_count: 12,
            parallel: false,
            ..settings()
        });
        let parallel = compare_strategies(SimulationSettings {
            rollout_count: 12,
            parallel: true,
            ..settings()
        });

        assert_eq!(serial.total_match_runs, parallel.total_match_runs);
        assert_eq!(serial.total_rounds, parallel.total_rounds);
        assert_eq!(serial.total_decisions, parallel.total_decisions);
        assert_eq!(serial.summaries, parallel.summaries);
    }

    #[test]
    fn simulation_summary_counts_matches_and_labels() {
        let report = compare_strategies(SimulationSettings {
            rollout_count: 4,
            strategies: vec![StrategyKind::MaxRoundEv, StrategyKind::MaxWinProbability],
            ..settings()
        });

        assert_eq!(report.summaries.len(), 2);
        assert_eq!(report.summaries[0].strategy, StrategyKind::MaxRoundEv);
        assert_eq!(
            report.summaries[1].strategy,
            StrategyKind::MaxWinProbability
        );
        assert_eq!(report.summaries[0].matches, 8);
        assert_eq!(report.summaries[1].matches, 8);
        assert!(report.summaries[0].average_rounds_to_finish > 0.0);
        assert!(report.total_decisions > 0);
    }

    #[test]
    fn win_probability_ai_can_choose_different_action_than_ev_ai() {
        let mut game = game_with_draw_order(1, vec![Card::Number(1)]);
        game.deal_selected_card(Card::Number(1));
        game.set_total_score_for_test(PlayerId::new(0), 49);
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(10),
            Card::Number(11),
            Card::Number(12),
        ]));
        let settings = SimulationSettings {
            target_score: 50,
            decision_rollouts: 1,
            max_rounds: 6,
            ..settings()
        };

        let ev = recommend_decision(&game, StrategyKind::MaxRoundEv, &settings);
        let win = recommend_decision(&game, StrategyKind::MaxWinProbability, &settings);

        assert_eq!(ev.decision, Some(AiDecision::Draw));
        assert_eq!(win.decision, Some(AiDecision::Stay));
        assert!(win.rationale.contains("reaches"));
        assert_eq!(game.players()[0].status(), PlayerStatus::Active);
    }

    #[test]
    fn monte_carlo_search_returns_stable_metadata() {
        let mut game = game_with_draw_order(2, vec![Card::Number(5)]);
        game.deal_selected_card(Card::Number(5));
        game.replace_deck_for_test(Deck::from_draw_order([
            Card::Number(6),
            Card::Number(7),
            Card::Number(5),
        ]));
        let settings = SimulationSettings {
            decision_rollouts: 3,
            ..settings()
        };
        let legal = legal_ai_decisions(&game);

        let first =
            evaluate_monte_carlo_actions(&game, PlayerId::new(0), &legal, &settings).unwrap();
        let second =
            evaluate_monte_carlo_actions(&game, PlayerId::new(0), &legal, &settings).unwrap();

        assert_eq!(first.decision, second.decision);
        assert_eq!(first.samples, 3);
        assert!((0.0..=1.0).contains(&first.win_rate));
        assert!(legal.contains(&first.decision));
    }
}
