// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

macro_rules! reads_as_its_name {
    ($ty:ty, $($variant:path => $name:literal $(| $also:literal)*),+ $(,)?) => {
        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D>(d: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct Named;

                impl serde::de::Visitor<'_> for Named {
                    type Value = $ty;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(f, "the name of a {} variant", stringify!($ty))
                    }

                    fn visit_str<E>(self, s: &str) -> Result<$ty, E>
                    where
                        E: serde::de::Error,
                    {
                        match s {
                            $($name $(| $also)* => return Ok($variant),)+
                            _ => {}
                        }

                        let shouted = !s.bytes().any(|b| b.is_ascii_lowercase());
                        let whispered = !s.bytes().any(|b| b.is_ascii_uppercase());
                        if shouted || whispered {
                            $(if s.eq_ignore_ascii_case($name)
                                $(|| s.eq_ignore_ascii_case($also))*
                            {
                                return Ok($variant);
                            })+
                        }

                        Err(E::unknown_variant(s, &[$($name),+]))
                    }
                }

                d.deserialize_str(Named)
            }
        }
    };
}

pub(crate) use reads_as_its_name;

#[cfg(test)]
mod tests {
    use crate::policy::EndpointMode;
    use crate::types::control_mode::ControlMode;
    use crate::types::device_type::DeviceType;
    use crate::types::packet_type::PacketType;
    use crate::types::touch_state::TouchState;

    #[test]
    fn a_code_is_not_a_name_and_is_refused() {
        for code in -1i32..=9 {
            let bytes = rmp_serde::to_vec(&code).unwrap();
            assert!(
                rmp_serde::from_slice::<EndpointMode>(&bytes).is_err(),
                "EndpointMode took the integer {code}"
            );
            assert!(
                rmp_serde::from_slice::<TouchState>(&bytes).is_err(),
                "TouchState took the integer {code}"
            );
            assert!(
                rmp_serde::from_slice::<DeviceType>(&bytes).is_err(),
                "DeviceType took the integer {code}"
            );
            assert!(
                rmp_serde::from_slice::<PacketType>(&bytes).is_err(),
                "PacketType took the integer {code}"
            );
        }
    }

    #[test]
    fn every_name_reads_back_as_the_variant_that_wrote_it() {
        fn round_trip<T>(variants: impl IntoIterator<Item = T>)
        where
            T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
        {
            for variant in variants {
                let bytes = rmp_serde::to_vec(&variant).unwrap();
                assert_eq!(rmp_serde::from_slice::<T>(&bytes).unwrap(), variant);
            }
        }

        round_trip(DeviceType::ALL);
        round_trip(PacketType::ALL);
        round_trip([EndpointMode::Game, EndpointMode::Controller]);
        round_trip([
            TouchState::Began,
            TouchState::Moved,
            TouchState::Stationary,
            TouchState::Ended,
            TouchState::Cancelled,
        ]);
        round_trip([
            ControlMode::Gamepad,
            ControlMode::Keyboard,
            ControlMode::Navigation,
            ControlMode::Wait,
        ]);
    }

    #[test]
    fn a_name_is_read_shouted_or_whispered_as_well_as_spelled() {
        for (spelling, expected) in [
            ("Controller", EndpointMode::Controller),
            ("controller", EndpointMode::Controller),
            ("CONTROLLER", EndpointMode::Controller),
            ("Game", EndpointMode::Game),
            ("game", EndpointMode::Game),
            ("GAME", EndpointMode::Game),
        ] {
            let bytes = rmp_serde::to_vec(spelling).unwrap();
            assert_eq!(
                rmp_serde::from_slice::<EndpointMode>(&bytes).unwrap(),
                expected,
                "{spelling}"
            );
        }

        for (spelling, expected) in [
            ("KeepAlive", PacketType::KeepAlive),
            ("keepalive", PacketType::KeepAlive),
            ("KEEPALIVE", PacketType::KeepAlive),
        ] {
            let bytes = rmp_serde::to_vec(spelling).unwrap();
            assert_eq!(
                rmp_serde::from_slice::<PacketType>(&bytes).unwrap(),
                expected,
                "{spelling}"
            );
        }
    }

    #[test]
    fn a_spelling_in_neither_one_case_nor_the_other_is_refused() {
        for spelling in ["dAtA", "kEePaLiVe", "Keepalive"] {
            let bytes = rmp_serde::to_vec(spelling).unwrap();
            assert!(
                rmp_serde::from_slice::<PacketType>(&bytes).is_err(),
                "{spelling} should not have been accepted"
            );
        }

        for spelling in ["IPhone", "iPhone", "iphone", "IPHONE"] {
            let bytes = rmp_serde::to_vec(spelling).unwrap();
            assert_eq!(
                rmp_serde::from_slice::<DeviceType>(&bytes).unwrap(),
                DeviceType::IPhone,
                "{spelling}"
            );
        }
    }

    #[test]
    fn a_name_nobody_has_heard_of_is_still_refused() {
        for spelling in ["Referee", "", "Contro ller", "Controller1"] {
            let bytes = rmp_serde::to_vec(spelling).unwrap();
            assert!(
                rmp_serde::from_slice::<EndpointMode>(&bytes).is_err(),
                "{spelling} should not have been accepted"
            );
        }
    }
}
