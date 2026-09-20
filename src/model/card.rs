use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Card {
    Number(u8),
    Bonus(BonusCard),
    SecondChance,
    FlipThree,
    Freeze,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BonusCard {
    Plus(u8),
    Double,
}

impl Card {
    pub const MAX_NUMBER: u8 = 12;

    pub fn short_label(self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Bonus(BonusCard::Plus(value)) => format!("+{value}"),
            Self::Bonus(BonusCard::Double) => "x2".to_owned(),
            Self::SecondChance => "SC".to_owned(),
            Self::FlipThree => "F3".to_owned(),
            Self::Freeze => "Frz".to_owned(),
        }
    }

    #[cfg(all(feature = "simulation", not(target_arch = "wasm32")))]
    pub fn is_number(self) -> bool {
        matches!(self, Self::Number(_))
    }

    pub fn sort_key(self) -> (u8, u8) {
        match self {
            Self::Number(value) => (0, value),
            Self::Bonus(BonusCard::Double) => (1, 0),
            Self::Bonus(BonusCard::Plus(value)) => (1, value),
            Self::SecondChance => (2, 0),
            Self::FlipThree => (2, 1),
            Self::Freeze => (2, 2),
        }
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => write!(f, "{value}"),
            Self::Bonus(BonusCard::Plus(value)) => write!(f, "+{value}"),
            Self::Bonus(BonusCard::Double) => write!(f, "x2"),
            Self::SecondChance => write!(f, "Second Chance"),
            Self::FlipThree => write!(f, "Flip Three"),
            Self::Freeze => write!(f, "Freeze"),
        }
    }
}
