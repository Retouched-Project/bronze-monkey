// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCode {
    pub kind: &'static str,
    pub given: i64,
    pub expected: &'static [(&'static str, i32)],
}

impl std::fmt::Display for UnknownCode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "unknown {} code {}, expected one of:",
            self.kind, self.given
        )?;
        for (i, (name, code)) in self.expected.iter().enumerate() {
            let sep = if i == 0 { " " } else { ", " };
            write!(f, "{sep}{code} ({name})")?;
        }
        Ok(())
    }
}

impl std::error::Error for UnknownCode {}

macro_rules! crosses_as_its_code {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$vmeta:meta])* $variant:ident = $code:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr(i32)]
        #[cfg_attr(target_arch = "wasm32", ::wasm_bindgen::prelude::wasm_bindgen)]
        $vis enum $name {
            $($(#[$vmeta])* $variant = $code),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const TABLE: &'static [(&'static str, i32)] =
                &[$((stringify!($variant), $code)),+];

            pub const fn code(self) -> i32 {
                self as i32
            }

            pub fn from_code(code: i32) -> Result<Self, crate::types::coded::UnknownCode> {
                Self::from_wide(code as i64)
            }

            fn from_wide(code: i64) -> Result<Self, crate::types::coded::UnknownCode> {
                match code {
                    $($code => Ok(Self::$variant),)+
                    _ => Err(crate::types::coded::UnknownCode {
                        kind: stringify!($name),
                        given: code,
                        expected: Self::TABLE,
                    }),
                }
            }

            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant),)+
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_i32(self.code())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct Coded;

                impl serde::de::Visitor<'_> for Coded {
                    type Value = $name;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(f, "the code of a {} variant", stringify!($name))
                    }

                    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<$name, E> {
                        $name::from_wide(v).map_err(E::custom)
                    }

                    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<$name, E> {
                        let v = i64::try_from(v).unwrap_or(i64::MAX);
                        $name::from_wide(v).map_err(E::custom)
                    }

                    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<$name, E> {
                        if v.fract() != 0.0 {
                            return Err(E::invalid_type(serde::de::Unexpected::Float(v), &self));
                        }
                        $name::from_wide(v as i64).map_err(E::custom)
                    }
                }

                d.deserialize_i32(Coded)
            }
        }
    };
}

pub(crate) use crosses_as_its_code;

#[cfg(test)]
mod tests {
    use crate::codec::externals::bm_reliability::BMReliability;
    use crate::engine::events::{Sensor, TouchPhase};
    use crate::link::negotiation::{LinkRole, VersionCheck};
    use crate::logging::LogLevel;
    use crate::policy::EndpointMode;
    use crate::types::channel_type::ChannelType;
    use crate::types::control_mode::ControlMode;
    use crate::types::device_type::DeviceType;
    use crate::types::packet_type::PacketType;
    use crate::types::touch_state::TouchState;

    fn round_trips<T>(all: &[T])
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        for variant in all {
            let bytes = rmp_serde::to_vec(variant).unwrap();
            let code: i32 = rmp_serde::from_slice(&bytes).expect("a code crosses as a number");
            let back: T = rmp_serde::from_slice(&bytes).unwrap();
            assert_eq!(
                &back, variant,
                "{variant:?} came back as {back:?} from {code}"
            );
        }
    }

    fn refuses<T>(kind: &str, codes: impl IntoIterator<Item = i64>)
    where
        T: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        for code in codes {
            let bytes = rmp_serde::to_vec(&code).unwrap();
            let read = rmp_serde::from_slice::<T>(&bytes);
            assert!(
                read.is_err(),
                "{kind} took the unknown code {code}: {read:?}"
            );
        }
    }

    fn refuses_its_names<T>(kind: &str, table: &[(&str, i32)])
    where
        T: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        for (name, _) in table {
            let bytes = rmp_serde::to_vec(name).unwrap();
            let read = rmp_serde::from_slice::<T>(&bytes);
            assert!(read.is_err(), "{kind} took the name {name}: {read:?}");
        }
    }

    macro_rules! check {
        ($($ty:ty),+) => {$(
            round_trips(<$ty>::ALL);
            refuses_its_names::<$ty>(stringify!($ty), <$ty>::TABLE);
            let known: Vec<i64> = <$ty>::TABLE.iter().map(|(_, c)| *c as i64).collect();
            refuses::<$ty>(
                stringify!($ty),
                (-2..=12).chain([i32::MAX as i64, u32::MAX as i64]).filter(|c| !known.contains(c)),
            );
        )+};
    }

    #[test]
    fn every_code_round_trips_and_nothing_else_is_taken() {
        check!(
            DeviceType,
            PacketType,
            EndpointMode,
            TouchState,
            ControlMode,
            ChannelType,
            BMReliability,
            LinkRole,
            LogLevel,
            Sensor,
            TouchPhase,
            VersionCheck
        );
    }

    #[test]
    fn a_code_is_read_as_the_code_and_never_as_an_index() {
        let bytes = rmp_serde::to_vec(&2).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<EndpointMode>(&bytes).unwrap(),
            EndpointMode::Controller
        );
        let bytes = rmp_serde::to_vec(&1).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<TouchState>(&bytes).unwrap(),
            TouchState::Began
        );
    }

    #[test]
    fn a_whole_float_is_a_code_and_a_fraction_is_not() {
        let bytes = rmp_serde::to_vec(&2.0f64).unwrap();
        assert_eq!(
            rmp_serde::from_slice::<ChannelType>(&bytes).unwrap(),
            ChannelType::Touch
        );
        let bytes = rmp_serde::to_vec(&2.5f64).unwrap();
        assert!(rmp_serde::from_slice::<ChannelType>(&bytes).is_err());
    }

    #[test]
    fn an_unknown_code_says_what_would_have_been_taken() {
        assert_eq!(
            LinkRole::from_code(7).unwrap_err().to_string(),
            "unknown LinkRole code 7, expected one of: 0 (Initiator), 1 (Responder)"
        );
    }
}
