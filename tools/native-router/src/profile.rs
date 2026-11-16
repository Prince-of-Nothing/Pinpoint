//! Car routing profiles.

use crate::graph::RoadClass;

/// Routing profile. Only the car profile is part of this spike; the enum keeps
/// the signature stable for pedestrian/bicycle additions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Car,
}

impl Profile {
    /// Parse a CLI profile name. `auto` is accepted as an alias for `car`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "car" | "auto" => Some(Profile::Car),
            _ => None,
        }
    }

    /// Highest speed this profile is willing to use, in km/h. The graph itself
    /// caps edge speeds; this is the profile's ceiling.
    pub fn max_speed_kmh(self) -> f32 {
        match self {
            Profile::Car => 130.0,
        }
    }

    /// Default speed for a road class under this profile.
    pub fn speed_kmh(self, class: RoadClass) -> f32 {
        match self {
            Profile::Car => class.default_speed_kmh(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_car_and_auto_case_insensitively() {
        assert_eq!(Profile::from_name("car"), Some(Profile::Car));
        assert_eq!(Profile::from_name("AUTO"), Some(Profile::Car));
        assert_eq!(Profile::from_name("bike"), None);
    }

    #[test]
    fn car_uses_documented_class_speeds() {
        assert_eq!(Profile::Car.speed_kmh(RoadClass::Motorway), 110.0);
        assert_eq!(Profile::Car.speed_kmh(RoadClass::Residential), 50.0);
        assert_eq!(Profile::Car.max_speed_kmh(), 130.0);
    }
}
