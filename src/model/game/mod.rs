mod actions;
mod card_resolution;
mod events;
mod odds;
mod scoreboard;
mod state;

pub use events::{ActionChoice, DealOutcome, PendingAction, SpecialAction};
pub use odds::{BustCardDetail, DrawOdds, ExpectedValueDetail};
pub use scoreboard::{PlayerScore, RoundOutcome, RoundScore};
pub use state::GameState;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BonusCard, Card, Deck, PlayerId, PlayerStatus};

    fn game_with_draw_order(player_count: usize, cards: Vec<Card>) -> GameState {
        GameState::with_deck(player_count, Deck::from_draw_order(cards))
    }

    #[test]
    fn first_deal_goes_to_first_player_and_advances_turn() {
        let mut game = game_with_draw_order(3, vec![Card::Number(7)]);

        let outcome = game.deal_next_card();

        assert!(matches!(
            outcome,
            DealOutcome::DealtNumber {
                player_id,
                ..
            } if player_id.index() == 0
        ));
        assert_eq!(game.players()[0].hand().len(), 1);
        assert_eq!(game.players()[1].hand().len(), 0);
        assert_eq!(game.current_player_index(), Some(1));
    }

    #[test]
    fn dealing_cycles_through_players_in_order() {
        let mut game = game_with_draw_order(
            3,
            vec![
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
            ],
        );

        game.deal_next_card();
        game.deal_next_card();
        game.deal_next_card();
        game.deal_next_card();

        assert_eq!(game.players()[0].hand().len(), 2);
        assert_eq!(game.players()[1].hand().len(), 1);
        assert_eq!(game.players()[2].hand().len(), 1);
        assert_eq!(game.current_player_index(), Some(1));
    }

    #[test]
    fn duplicate_number_busts_player_without_second_chance() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::Number(5)]);

        game.deal_next_card();
        let outcome = game.deal_next_card();

        assert!(matches!(outcome, DealOutcome::RoundEnded { .. }));
        assert_eq!(game.players()[0].status(), PlayerStatus::Busted);
        assert_eq!(game.players()[0].hand().len(), 0);
        assert_eq!(game.discard_pile_count(), 2);
        assert_eq!(game.score_board()[0].last_round_score, 0);
        assert_eq!(game.score_board()[0].round_scores[0].score, 0);
    }

    #[test]
    fn second_chance_is_removed_instead_of_busting_on_duplicate() {
        let mut game = game_with_draw_order(
            1,
            vec![Card::Number(5), Card::SecondChance, Card::Number(5)],
        );

        game.deal_next_card();
        game.deal_next_card();
        let outcome = game.deal_next_card();

        assert!(matches!(outcome, DealOutcome::UsedSecondChance { .. }));
        assert_eq!(game.players()[0].status(), PlayerStatus::Active);
        assert_eq!(game.players()[0].hand(), &[Card::Number(5)]);
    }

    #[test]
    fn stay_ends_round_when_all_players_are_done_and_scores() {
        let mut game =
            game_with_draw_order(1, vec![Card::Number(5), Card::Bonus(BonusCard::Plus(10))]);

        game.deal_next_card();
        game.deal_next_card();
        let outcome = game.stay_current_player();

        assert!(matches!(outcome, DealOutcome::RoundEnded { .. }));
        assert_eq!(game.players()[0].status(), PlayerStatus::Stayed);
        assert_eq!(game.score_board()[0].last_round_score, 15);
        assert_eq!(game.score_board()[0].total_score, 15);
    }

    #[test]
    fn x2_bonus_doubles_numbers_before_additive_bonuses() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(5),
                Card::Bonus(BonusCard::Double),
                Card::Bonus(BonusCard::Plus(10)),
            ],
        );

        game.deal_next_card();
        game.deal_next_card();
        game.deal_next_card();
        game.stay_current_player();

        assert_eq!(game.score_board()[0].last_round_score, 20);
    }

    #[test]
    fn flip_seven_ends_round_with_bonus() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(0),
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
                Card::Number(6),
            ],
        );

        for _ in 0..6 {
            game.deal_next_card();
        }
        let outcome = game.deal_next_card();

        assert!(matches!(outcome, DealOutcome::RoundEnded { .. }));
        assert!(game.round_over());
        assert_eq!(game.score_board()[0].last_round_score, 36);
    }

    #[test]
    fn next_round_preserves_score_and_discards_previous_hands() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5)]);

        game.deal_next_card();
        game.stay_current_player();
        let discard_before = game.discard_pile_count();

        assert_eq!(discard_before, 1);
        assert_eq!(game.start_next_round(), DealOutcome::NewRoundStarted);
        assert!(game.players()[0].hand().is_empty());
        assert_eq!(game.players()[0].status(), PlayerStatus::Active);
        assert_eq!(game.score_board()[0].total_score, 5);
        assert_eq!(game.discard_pile_count(), 1);
        assert_eq!(game.round_number(), 2);
    }

    #[test]
    fn next_round_rotates_starting_player() {
        let mut game = game_with_draw_order(3, vec![]);

        assert_eq!(game.current_player_index(), Some(0));
        game.stay_current_player();
        game.stay_current_player();
        game.stay_current_player();

        assert!(game.round_over());
        assert_eq!(game.start_next_round(), DealOutcome::NewRoundStarted);
        assert_eq!(game.current_player_index(), Some(1));

        game.stay_current_player();
        game.stay_current_player();
        game.stay_current_player();
        assert_eq!(game.start_next_round(), DealOutcome::NewRoundStarted);
        assert_eq!(game.current_player_index(), Some(2));
    }

    #[test]
    fn manual_reshuffle_moves_discard_into_draw_pile_and_is_undoable() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5)]);

        game.deal_next_card();
        game.stay_current_player();
        assert_eq!(game.draw_pile_count(), 0);
        assert_eq!(game.discard_pile_count(), 1);

        assert_eq!(
            game.reshuffle_discard_into_draw_pile(),
            DealOutcome::DiscardReshuffled { cards_moved: 1 }
        );
        assert_eq!(game.draw_pile_count(), 1);
        assert_eq!(game.discard_pile_count(), 0);

        assert_eq!(game.undo(), DealOutcome::UndoApplied);
        assert_eq!(game.draw_pile_count(), 0);
        assert_eq!(game.discard_pile_count(), 1);
    }

    #[test]
    fn scoreboard_tracks_score_per_round() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::Number(6)]);

        game.deal_next_card();
        game.stay_current_player();
        game.start_next_round();
        game.deal_next_card();
        game.stay_current_player();

        assert_eq!(game.score_board()[0].round_scores.len(), 2);
        assert_eq!(game.score_board()[0].round_scores[0].round_number, 1);
        assert_eq!(game.score_board()[0].round_scores[0].score, 5);
        assert_eq!(game.score_board()[0].round_scores[1].round_number, 2);
        assert_eq!(game.score_board()[0].round_scores[1].score, 6);
        assert_eq!(game.score_board()[0].total_score, 11);
        assert_eq!(game.score_board()[0].rounds_completed, 2);
        assert_eq!(game.score_board()[0].average_round_score(), Some(5.5));
        assert_eq!(game.score_board()[0].best_round_score(), Some(6));
        assert_eq!(game.score_board()[0].worst_round_score(), Some(5));
    }

    #[test]
    fn scoreboard_tracks_selected_deal_bust_risk_samples() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5)]);

        game.deal_next_card();
        game.deck = Deck::from_draw_order([Card::Number(5), Card::Number(6)]);
        game.deal_selected_card(Card::Number(6));

        let score = &game.score_board()[0];
        assert_eq!(score.total_cards_dealt, 2);
        assert_eq!(score.current_round_cards_dealt, 2);
        assert_eq!(score.bust_risk_sample_count, 2);
        assert_eq!(score.average_bust_risk_basis_points(), Some(2500));
        assert_eq!(
            score.current_round_average_bust_risk_basis_points(),
            Some(2500)
        );
        assert_eq!(score.peak_bust_risk_basis_points, 5000);
        assert_eq!(score.current_round_peak_bust_risk_basis_points, 5000);
    }

    #[test]
    fn scoreboard_tracks_second_chance_protected_risk_samples() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::SecondChance]);

        game.deal_next_card();
        game.deal_next_card();
        game.deck = Deck::from_draw_order([Card::Number(5), Card::Number(6)]);
        game.deal_selected_card(Card::Number(6));

        let score = &game.score_board()[0];
        assert_eq!(score.total_cards_dealt, 3);
        assert_eq!(score.average_bust_risk_basis_points(), Some(0));
        assert_eq!(score.second_chance_protected_samples, 1);
        assert_eq!(score.current_round_second_chance_protected_samples, 1);
    }

    #[test]
    fn scoreboard_tracks_round_outcomes_and_cards_dealt() {
        let mut game = game_with_draw_order(2, vec![Card::Freeze]);

        game.deal_next_card();
        game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));
        game.stay_current_player();

        let player_one = &game.score_board()[0];
        let player_two = &game.score_board()[1];
        assert_eq!(player_one.round_scores[0].outcome, RoundOutcome::Stayed);
        assert_eq!(player_one.stayed_count, 1);
        assert_eq!(player_one.round_scores[0].cards_dealt, 1);
        assert_eq!(player_two.round_scores[0].outcome, RoundOutcome::Frozen);
        assert_eq!(player_two.frozen_count, 1);
        assert_eq!(player_two.round_scores[0].cards_dealt, 0);
    }

    #[test]
    fn scoreboard_tracks_flip_seven_outcome() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(0),
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
                Card::Number(6),
            ],
        );

        for _ in 0..7 {
            game.deal_next_card();
        }

        let score = &game.score_board()[0];
        assert_eq!(score.round_scores[0].outcome, RoundOutcome::FlipSeven);
        assert_eq!(score.flip_seven_count, 1);
        assert_eq!(score.round_scores[0].cards_dealt, 7);
    }

    #[test]
    fn undo_restores_scoreboard_telemetry() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::Number(6)]);

        game.deal_next_card();
        assert_eq!(game.score_board()[0].total_cards_dealt, 1);

        assert_eq!(game.undo(), DealOutcome::UndoApplied);

        let score = &game.score_board()[0];
        assert_eq!(score.total_cards_dealt, 0);
        assert_eq!(score.current_round_cards_dealt, 0);
        assert_eq!(score.bust_risk_sample_count, 0);
        assert_eq!(score.average_bust_risk_basis_points(), None);
    }

    #[test]
    fn freeze_waits_for_player_selection_and_freezes_target() {
        let mut game = game_with_draw_order(2, vec![Card::Freeze]);

        let outcome = game.deal_next_card();
        assert!(matches!(
            outcome,
            DealOutcome::SpecialNeedsTarget {
                action: SpecialAction::Freeze,
                ..
            }
        ));
        assert!(game.pending_action().is_some());

        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));

        assert!(matches!(
            outcome,
            DealOutcome::SpecialResolved {
                action: SpecialAction::Freeze,
                ..
            }
        ));
        assert_eq!(game.players()[1].status(), PlayerStatus::Frozen);
        assert_eq!(game.current_player_index(), Some(0));
    }

    #[test]
    fn flip_three_waits_for_player_selection_and_deals_three_to_target() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
            ],
        );

        game.deal_next_card();
        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));

        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected {
                target_player_id,
                remaining_draws: 3,
                ..
            } if target_player_id == PlayerId::new(1)
        ));
        assert!(matches!(
            game.deal_selected_card(Card::Number(1)),
            DealOutcome::FlipThreeCardDealt {
                remaining_draws: 2,
                ..
            }
        ));
        game.deal_selected_card(Card::Number(2));
        game.deal_selected_card(Card::Number(3));
        assert_eq!(game.players()[1].hand().len(), 3);
        assert_eq!(game.current_player_index(), Some(1));
    }

    #[test]
    fn selected_number_deals_exact_card() {
        let mut game = game_with_draw_order(1, vec![Card::Number(3), Card::Number(4)]);

        let outcome = game.deal_selected_card(Card::Number(4));

        assert!(matches!(
            outcome,
            DealOutcome::DealtNumber {
                card: Card::Number(4),
                ..
            }
        ));
        assert_eq!(game.players()[0].hand(), &[Card::Number(4)]);
        assert_eq!(game.draw_pile_count(), 1);
    }

    #[test]
    fn unavailable_selected_card_does_not_mutate_game() {
        let mut game = game_with_draw_order(1, vec![Card::Number(3)]);

        let outcome = game.deal_selected_card(Card::Number(4));

        assert_eq!(outcome, DealOutcome::SelectedCardUnavailable);
        assert!(game.players()[0].hand().is_empty());
        assert_eq!(game.draw_pile_count(), 1);
        assert!(!game.can_undo());
    }

    #[test]
    fn selected_special_card_uses_pending_target_flow() {
        let mut game = game_with_draw_order(2, vec![Card::Freeze]);

        let outcome = game.deal_selected_card(Card::Freeze);

        assert!(matches!(
            outcome,
            DealOutcome::SpecialNeedsTarget {
                action: SpecialAction::Freeze,
                ..
            }
        ));
        assert!(game.pending_action().is_some());
    }

    #[test]
    fn selected_flip_three_supports_nested_flip_three_resolution() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::FlipThree,
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
            ],
        );

        assert!(matches!(
            game.deal_selected_card(Card::FlipThree),
            DealOutcome::SpecialNeedsTarget {
                action: SpecialAction::FlipThree,
                ..
            }
        ));

        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));
        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected { .. }
        ));
        assert!(matches!(
            game.deal_selected_card(Card::FlipThree),
            DealOutcome::FlipThreeCardDealt {
                remaining_draws: 2,
                ..
            }
        ));
        game.deal_selected_card(Card::Number(1));
        game.deal_selected_card(Card::Number(2));
        assert_eq!(
            game.players()[1].hand(),
            &[Card::Number(1), Card::Number(2)]
        );
        assert!(matches!(
            game.pending_action().map(PendingAction::action),
            Some(SpecialAction::FlipThree)
        ));

        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(0)));
        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected { .. }
        ));
        game.deal_selected_card(Card::Number(3));
        game.deal_selected_card(Card::Number(4));
        game.deal_selected_card(Card::Number(5));
        assert_eq!(
            game.players()[0].hand(),
            &[Card::Number(3), Card::Number(4), Card::Number(5)]
        );
        assert!(game.pending_action().is_none());
    }

    #[test]
    fn selected_deal_can_be_undone_with_deck_restored() {
        let mut game = game_with_draw_order(1, vec![Card::Number(3), Card::Number(4)]);

        game.deal_selected_card(Card::Number(4));
        assert_eq!(game.undo(), DealOutcome::UndoApplied);

        assert!(game.players()[0].hand().is_empty());
        assert_eq!(game.draw_pile_count(), 2);
        assert_eq!(
            game.deal_selected_card(Card::Number(4)),
            DealOutcome::DealtNumber {
                player_id: PlayerId::new(0),
                player_name: "Player 1".to_owned(),
                card: Card::Number(4),
            }
        );
    }

    #[test]
    fn flip_three_keeps_second_chance_and_continues_dealing() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::SecondChance,
                Card::Number(1),
                Card::Number(2),
            ],
        );

        game.deal_next_card();
        game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));
        game.deal_selected_card(Card::SecondChance);
        game.deal_selected_card(Card::Number(1));
        game.deal_selected_card(Card::Number(2));

        assert_eq!(
            game.players()[1].hand(),
            &[Card::SecondChance, Card::Number(1), Card::Number(2)]
        );
        assert!(game.pending_action().is_none());
    }

    #[test]
    fn flip_three_queues_freeze_and_finishes_current_sequence() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::Freeze,
                Card::Number(1),
                Card::Number(2),
            ],
        );

        game.deal_next_card();
        let outcome = game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));

        assert!(matches!(
            outcome,
            DealOutcome::FlipThreeTargetSelected { .. }
        ));
        game.deal_selected_card(Card::Freeze);
        game.deal_selected_card(Card::Number(1));
        game.deal_selected_card(Card::Number(2));
        assert_eq!(
            game.players()[1].hand(),
            &[Card::Number(1), Card::Number(2)]
        );
        assert!(matches!(
            game.pending_action().map(PendingAction::action),
            Some(SpecialAction::Freeze)
        ));
    }

    #[test]
    fn queued_specials_resolve_fifo_after_current_sequence() {
        let mut game = game_with_draw_order(
            2,
            vec![
                Card::FlipThree,
                Card::Freeze,
                Card::FlipThree,
                Card::Number(2),
                Card::Number(3),
            ],
        );

        game.deal_next_card();
        game.resolve_pending_action(ActionChoice::Player(PlayerId::new(1)));
        game.deal_selected_card(Card::Freeze);
        game.deal_selected_card(Card::FlipThree);
        game.deal_selected_card(Card::Number(2));

        assert!(matches!(
            game.pending_action().map(PendingAction::action),
            Some(SpecialAction::Freeze)
        ));
        assert_eq!(game.queued_action_count(), 1);

        game.resolve_pending_action(ActionChoice::Player(PlayerId::new(0)));

        assert!(matches!(
            game.pending_action().map(PendingAction::action),
            Some(SpecialAction::FlipThree)
        ));
        assert_eq!(game.queued_action_count(), 0);
    }

    #[test]
    fn undo_restores_previous_deal_state() {
        let mut game = game_with_draw_order(2, vec![Card::Number(1), Card::Number(2)]);

        game.deal_next_card();
        game.deal_next_card();

        assert_eq!(game.undo(), DealOutcome::UndoApplied);
        assert_eq!(game.players()[0].hand().len(), 1);
        assert_eq!(game.players()[1].hand().len(), 0);
        assert_eq!(game.current_player_index(), Some(1));
    }

    #[test]
    fn odds_report_bust_flip_seven_and_expected_value() {
        let mut game = game_with_draw_order(
            1,
            vec![
                Card::Number(1),
                Card::Number(2),
                Card::Number(3),
                Card::Number(4),
                Card::Number(5),
                Card::Number(6),
            ],
        );
        for _ in 0..6 {
            game.deal_next_card();
        }
        game.deck = Deck::from_draw_order([
            Card::Number(1),
            Card::Number(7),
            Card::Bonus(BonusCard::Plus(10)),
            Card::Freeze,
        ]);

        let odds = game.current_player_odds().expect("odds");

        assert_eq!(odds.next_pool_size, 4);
        assert_eq!(odds.bust_probability, 0.25);
        assert_eq!(odds.flip_seven_probability, 0.25);
        assert_eq!(odds.bust_cards.len(), 1);
        assert_eq!(odds.bust_cards[0].number, 1);
        assert_eq!(odds.bust_cards[0].count, 1);
        assert!(!odds.bust_cards[0].protected_by_second_chance);
        assert!(
            odds.expected_value_details
                .iter()
                .any(|detail| detail.card == Card::Number(1) && detail.score_after_draw == 0)
        );
        assert!(
            odds.expected_value_details
                .iter()
                .any(|detail| detail.card == Card::Number(7) && detail.score_after_draw == 43)
        );
        assert!(odds.expected_score_after_draw > odds.current_score as f64);
    }

    #[test]
    fn odds_have_no_bust_contributors_when_second_chance_is_available() {
        let mut game = game_with_draw_order(1, vec![Card::Number(5), Card::SecondChance]);

        game.deal_next_card();
        game.deal_next_card();
        game.deck = Deck::from_draw_order([Card::Number(5), Card::Number(6)]);

        let odds = game.current_player_odds().expect("odds");

        assert_eq!(odds.bust_probability, 0.0);
        assert_eq!(odds.bust_cards.len(), 1);
        assert_eq!(odds.bust_cards[0].number, 5);
        assert!(odds.bust_cards[0].protected_by_second_chance);
        assert!(
            odds.expected_value_details
                .iter()
                .any(|detail| detail.card == Card::Number(5) && detail.score_after_draw == 5)
        );
    }

    #[test]
    fn deal_draws_from_discard_when_draw_pile_is_empty() {
        let mut game = GameState::with_deck(1, Deck::with_draw_and_discard([], [Card::Number(4)]));

        let outcome = game.deal_next_card();

        assert!(matches!(outcome, DealOutcome::DealtNumber { .. }));
        assert_eq!(game.players()[0].hand(), &[Card::Number(4)]);
    }
}
