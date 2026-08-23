// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::fmt;

use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::registry;
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct AckPacket {
    pub device: DeviceCore,
    pub device_address: BMAddress,
}

impl AckPacket {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_ACK_PACKET;

    pub fn new(device: DeviceCore, device_address: BMAddress) -> Self {
        Self {
            device,
            device_address,
        }
    }

    pub fn read_from<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<Self> {
        Self::skip_object_header(input)?;
        let device = DeviceCore::read_from(input)?;

        Self::skip_object_header(input)?;
        let device_address = BMAddress::read_from(input)?;

        Ok(Self {
            device,
            device_address,
        })
    }

    fn skip_object_header<B: AsRef<[u8]>>(input: &mut BMStream<B>) -> Result<()> {
        let _ = input.read_short()?;
        let _ = input.read_bytes(1)?;
        let _ = input.read_short()?;
        Ok(())
    }

    pub fn write_to(&self, out: &mut BMStream<Vec<u8>>) -> Result<()> {
        out.write_short(1)?;
        out.write_bytes(b"@")?;
        out.write_short(registry::class_id_for_device_type(self.device.device_type) as i16)?;
        self.device.write_to(out)?;

        out.write_short(1)?;
        out.write_bytes(b"@")?;
        out.write_short(registry::BM_CLASS_ID_ADDRESS as i16)?;
        self.device_address.write_to(out)
    }
}

impl fmt::Display for AckPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AckPacket [device={}, bmAddress={}]",
            self.device, self.device_address
        )
    }
}

#[cfg(test)]
mod tests {
    use super::AckPacket;
    use crate::codec::bm_stream::BMStream;
    use crate::codec::object::Object;
    use crate::devices::bm_address::BMAddress;
    use crate::devices::device_core::DeviceCore;
    use crate::types::device_type::DeviceType;

    const REAL_ACK: &[u8] = &[
        0x01, 0x00, 0x40, 0x09, 0x00, // AckPacket object, classId 9
        0x01, 0x00, 0x40, 0x12, 0x00, // device object, classId 18
        0x04, 0x00, 0x00, 0x00, // device_type = 4
        0x10, 0x00, // device_id length
        0x39, 0x62, 0x65, 0x33, 0x34, 0x61, 0x32, 0x66, 0x65, 0x35, 0x63, 0x33, 0x66, 0x62, 0x39,
        0x30, 0x09, 0x00, // device_name length
        0x4d, 0x32, 0x30, 0x30, 0x37, 0x4a, 0x33, 0x53, 0x47, 0x01, 0x00, 0x40, 0x01,
        0x00, // BMAddress object, classId 1
        0x0d, 0x00, // address length
        0x31, 0x39, 0x32, 0x2e, 0x31, 0x36, 0x38, 0x2e, 0x31, 0x2e, 0x31, 0x32, 0x36, 0x79, 0x23,
        0x00, 0x00, // unreliable_port
        0x79, 0x23, 0x00, 0x00, // reliable_port
    ];

    #[test]
    fn decodes_real_wire_ack() {
        let mut cur = BMStream::view(REAL_ACK);
        let ack = match Object::decode(&mut cur).expect("real ack should decode") {
            Object::AckPacket(a) => a,
            other => panic!("expected AckPacket, got class id {}", other.class_id()),
        };
        assert_eq!(ack.device.device_type, DeviceType::Android);
        assert_eq!(ack.device.device_id, "9be34a2fe5c3fb90");
        assert_eq!(ack.device.device_name, "M2007J3SG");
        assert_eq!(ack.device_address.address, "192.168.1.126");
        assert_eq!(ack.device_address.unreliable_port, 9081);
        assert_eq!(ack.device_address.reliable_port, 9081);
    }

    // Ack whose address carries the sending game's own endpoint (a real udp
    // port), with a different device class id than the captured case.
    const GAME_ENDPOINT_ACK: &[u8] = &[
        0x01, 0x00, 0x40, 0x09, 0x00, // AckPacket object, classId 9
        0x01, 0x00, 0x40, 0x0a, 0x00, // device object, classId 10
        0x04, 0x00, 0x00, 0x00, // device_type = 4
        0x06, 0x00, // device_id length
        0x63, 0x74, 0x72, 0x6c, 0x30, 0x31, // "ctrl01"
        0x07, 0x00, // device_name length
        0x47, 0x61, 0x6d, 0x65, 0x70, 0x61, 0x64, // "Gamepad"
        0x01, 0x00, 0x40, 0x01, 0x00, // BMAddress object, classId 1
        0x0d, 0x00, // address length
        0x31, 0x39, 0x32, 0x2e, 0x31, 0x36, 0x38, 0x2e, 0x31, 0x2e, 0x31, 0x31, 0x35, 0x59, 0x23,
        0x00, 0x00, // unreliable_port
        0x35, 0x23, 0x00, 0x00, // reliable_port
    ];

    #[test]
    fn decodes_ack_with_game_endpoint() {
        let mut cur = BMStream::view(GAME_ENDPOINT_ACK);
        let ack = match Object::decode(&mut cur).expect("ack should decode") {
            Object::AckPacket(a) => a,
            other => panic!("expected AckPacket, got class id {}", other.class_id()),
        };
        assert_eq!(ack.device.device_type, DeviceType::Android);
        assert_eq!(ack.device.device_id, "ctrl01");
        assert_eq!(ack.device.device_name, "Gamepad");
        assert_eq!(ack.device_address.address, "192.168.1.115");
        assert_eq!(ack.device_address.unreliable_port, 9049);
        assert_eq!(ack.device_address.reliable_port, 9013);
    }

    #[test]
    fn round_trips_through_object_codec() {
        let ack = AckPacket::new(
            DeviceCore::new(
                "somerandomid123456".to_string(),
                "Test Controller".to_string(),
                DeviceType::Android,
            ),
            BMAddress::new("192.168.1.50".to_string(), 9080, 9081),
        );

        let mut buf = BMStream::new();
        Object::AckPacket(ack.clone())
            .encode_with_marker(&mut buf)
            .expect("encode ack");
        let bytes = buf.into_inner();

        let mut cur = BMStream::view(&bytes);
        let decoded = match Object::decode(&mut cur).expect("decode ack") {
            Object::AckPacket(a) => a,
            other => panic!("expected AckPacket, got class id {}", other.class_id()),
        };
        assert_eq!(decoded.device.device_id, ack.device.device_id);
        assert_eq!(decoded.device.device_name, ack.device.device_name);
        assert_eq!(decoded.device.device_type, ack.device.device_type);
        assert_eq!(decoded.device_address.address, ack.device_address.address);
        assert_eq!(
            decoded.device_address.unreliable_port,
            ack.device_address.unreliable_port
        );
        assert_eq!(
            decoded.device_address.reliable_port,
            ack.device_address.reliable_port
        );
    }
}
