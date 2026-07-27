// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::Result;
use crate::codec::bm_stream::BMStream;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::externals::registry;
use crate::types::device_type::DeviceType;
use crate::types::packet_type::PacketType;

pub fn serialize_packet(packet: &BMPacket) -> Result<Vec<u8>> {
    let msg_len = packet.message.as_ref().map_or(0, |m| m.len());
    let mut stream = BMStream::with_capacity(128 + msg_len);
    stream.write_unsigned_int(0)?; // placeholder length prefix

    stream.write_utf("@")?; // object marker
    stream.write_short(registry::BM_CLASS_ID_PACKET as i16)?;

    stream.write_int(packet.channel)?;
    stream.write_int(packet.sequence)?;
    stream.write_double(packet.timestamp)?;
    stream.write_double(packet.rtt)?;
    stream.write_int(packet.packet_type.code())?;
    stream.write_int(packet.device_type.code())?;

    stream.write_utf(&packet.device_id)?;
    stream.write_utf(&packet.device_name)?;

    match &packet.message {
        Some(msg) => {
            stream.write_boolean(true)?;
            stream.write_bytes(msg)?;
        }
        None => stream.write_boolean(false)?,
    }

    let mut bytes = stream.into_inner();
    let body_len = (bytes.len() - 4) as u32;
    bytes[0..4].copy_from_slice(&body_len.to_le_bytes());
    Ok(bytes)
}

pub fn deserialize_packet(data: &[u8], pkt: &mut BMPacket) -> Result<()> {
    if data.len() < 4 {
        return Err("Buffer too small for length prefix".into());
    }
    let mut framed = BMStream::view(data);
    let size = framed.read_unsigned_int()? as usize;

    if size > 5 * 1024 * 1024 {
        // cap at 5MB sanity check
        return Err(format!("Packet size too large: {}", size).into());
    }
    let end_offset = 4usize.checked_add(size).ok_or("Packet size overflow")?;

    if end_offset > data.len() {
        return Err(format!(
            "Framed size mismatch: need {} but have {}",
            end_offset,
            data.len()
        )
        .into());
    }

    let payload = &data[4..end_offset];
    let mut body = BMStream::view(payload);

    let marker = body.read_utf()?;
    let class_id = body.read_short()? as u32;

    if marker != "@" || class_id != registry::BM_CLASS_ID_PACKET {
        return Err("Invalid packet envelope".into());
    }

    pkt.channel = body.read_int()?;
    pkt.sequence = body.read_int()?;
    pkt.timestamp = body.read_double()?;
    pkt.rtt = body.read_double()?;
    let pkt_type_code = body.read_int()?;
    let dev_type_code = body.read_int()?;

    pkt.packet_type = PacketType::from_i32(pkt_type_code).ok_or("Invalid packet type")?;
    pkt.device_type =
        DeviceType::for_value(dev_type_code).map_err(|e| format!("Invalid DeviceType: {}", e))?;

    pkt.device_id = body
        .read_utf()
        .map_err(|e| format!("Failed to read device_id: {}", e))?;
    pkt.device_name = body
        .read_utf()
        .map_err(|e| format!("Failed to read device_name: {}", e))?;

    let has_message = body.read_boolean()?;
    pkt.message = if has_message {
        let pos = body.position();
        if pos > payload.len() {
            return Err("Message position out of bounds".into());
        }
        Some(payload[pos..].to_vec())
    } else {
        None
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(message: Option<Vec<u8>>) -> BMPacket {
        let mut pkt = BMPacket::default();
        pkt.channel = 3;
        pkt.sequence = 99;
        pkt.timestamp = 1234.5;
        pkt.rtt = 12.0;
        pkt.packet_type = PacketType::Data;
        pkt.device_type = DeviceType::Flash;
        pkt.device_id = "dev-id".to_string();
        pkt.device_name = "Device Name".to_string();
        pkt.message = message;
        pkt
    }

    fn assert_round_trip(original: &BMPacket) {
        let bytes = serialize_packet(original).unwrap();
        let mut decoded = BMPacket::default();
        deserialize_packet(&bytes, &mut decoded).unwrap();

        assert_eq!(decoded.channel, original.channel);
        assert_eq!(decoded.sequence, original.sequence);
        assert_eq!(decoded.timestamp, original.timestamp);
        assert_eq!(decoded.rtt, original.rtt);
        assert_eq!(decoded.packet_type, original.packet_type);
        assert_eq!(decoded.device_type, original.device_type);
        assert_eq!(decoded.device_id, original.device_id);
        assert_eq!(decoded.device_name, original.device_name);
        assert_eq!(decoded.message, original.message);
    }

    #[test]
    fn packet_round_trips_with_message() {
        assert_round_trip(&sample(Some(vec![1, 2, 3, 4, 5])));
    }

    #[test]
    fn packet_round_trips_without_message() {
        assert_round_trip(&sample(None));
    }

    #[test]
    fn framing_prefixes_body_length() {
        let bytes = serialize_packet(&sample(None)).unwrap();
        let declared = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        assert_eq!(declared, bytes.len() - 4);
    }

    #[test]
    fn truncated_buffer_errors() {
        let mut decoded = BMPacket::default();
        assert!(deserialize_packet(&[0x00, 0x00], &mut decoded).is_err());
    }

    // Golden bytes computed by hand from the wire format. Locks the exact
    // packet envelope + framing so the codec cannot silently drift.
    #[test]
    fn packet_golden_bytes() {
        let mut pkt = BMPacket::default();
        pkt.channel = 2;
        pkt.sequence = 7;
        pkt.timestamp = 0.0;
        pkt.rtt = 0.0;
        pkt.packet_type = PacketType::Data;
        pkt.device_type = DeviceType::Flash;
        pkt.device_id = "a".to_string();
        pkt.device_name = "b".to_string();
        pkt.message = None;

        let bytes = serialize_packet(&pkt).unwrap();
        let expected: &[u8] = &[
            0x2C, 0x00, 0x00, 0x00, // framed body length = 44
            0x01, 0x00, 0x40, // "@" object marker
            0x00, 0x00, // class id 0 (BMPacket)
            0x02, 0x00, 0x00, 0x00, // channel = 2
            0x07, 0x00, 0x00, 0x00, // sequence = 7
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // timestamp 0.0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // rtt 0.0
            0x00, 0x00, 0x00, 0x00, // packet_type Data (0)
            0x03, 0x00, 0x00, 0x00, // device_type Flash (3)
            0x01, 0x00, 0x61, // device_id "a"
            0x01, 0x00, 0x62, // device_name "b"
            0x00, // has_message = false
        ];
        assert_eq!(bytes, expected);
    }
}
