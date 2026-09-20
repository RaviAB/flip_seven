use crate::simulation::StrategyKind;

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
