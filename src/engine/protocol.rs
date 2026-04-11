// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::externals::bm_packet::BMPacket;
use crate::externals::registry;
use crate::io::io::Result;
use crate::types::device_type::DeviceType;
use crate::types::packet_type::PacketType;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};

pub fn serialize_packet(packet: &BMPacket) -> Result<Vec<u8>> {
    let mut body = Vec::new();

    write_object_envelope(&mut body, registry::BM_CLASS_ID_PACKET)?;

    body.extend_from_slice(&packet.channel.to_le_bytes());
    body.extend_from_slice(&packet.sequence.to_le_bytes());
    body.extend_from_slice(&packet.timestamp.to_le_bytes());
    body.extend_from_slice(&packet.rtt.to_le_bytes());
    body.extend_from_slice(&packet.packet_type.code().to_le_bytes());
    body.extend_from_slice(&packet.device_type.code().to_le_bytes());

    write_utf(&mut body, &packet.device_id)?;
    write_utf(&mut body, &packet.device_name)?;

    if let Some(msg) = &packet.message {
        body.push(1); // has_message = true
        body.extend_from_slice(msg);
    } else {
        body.push(0); // has_message = false
    }

    let mut framed = Vec::with_capacity(4 + body.len());
    framed.extend_from_slice(&(body.len() as u32).to_le_bytes());
    framed.extend_from_slice(&body);

    Ok(framed)
}

pub fn deserialize_packet(data: &[u8], pkt: &mut BMPacket) -> Result<()> {
    if data.len() < 4 {
        return Err("Buffer too small for length prefix".into());
    }
    let mut cur = Cursor::new(data);
    let size = cur.read_u32::<LittleEndian>()? as usize;

    if size > 100 * 1024 * 1024 {
        // cap at 100MB sanity check
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
    let mut cur = Cursor::new(payload);

    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&"WASM: deserialize_packet start".into());

    let header = cur.read_i16::<LittleEndian>()?;
    let at = cur.read_u8()?;
    let class_id = cur.read_i16::<LittleEndian>()? as u32;

    if header != 1 || at != b'@' || class_id != registry::BM_CLASS_ID_PACKET {
        return Err("Invalid packet envelope".into());
    }

    pkt.channel = cur.read_i32::<LittleEndian>()?;
    pkt.sequence = cur.read_i32::<LittleEndian>()?;
    pkt.timestamp = cur.read_f64::<LittleEndian>()?;
    pkt.rtt = cur.read_f64::<LittleEndian>()?;
    let pkt_type_code = cur.read_i32::<LittleEndian>()?;
    let dev_type_code = cur.read_i32::<LittleEndian>()?;

    pkt.packet_type = PacketType::from_i32(pkt_type_code).ok_or("Invalid packet type")?;
    pkt.device_type =
        DeviceType::for_value(dev_type_code).map_err(|e| format!("Invalid DeviceType: {}", e))?;

    pkt.device_id = read_utf(&mut cur).map_err(|e| format!("Failed to read device_id: {}", e))?;
    pkt.device_name =
        read_utf(&mut cur).map_err(|e| format!("Failed to read device_name: {}", e))?;

    let has_message = cur.read_u8()? != 0;
    pkt.message = if has_message {
        let pos = cur.position() as usize;
        if pos > payload.len() {
            return Err("Message position out of bounds".into());
        }
        Some(payload[pos..].to_vec())
    } else {
        None
    };

    Ok(())
}

fn write_object_envelope(out: &mut Vec<u8>, class_id: u32) -> Result<()> {
    out.extend_from_slice(&1i16.to_le_bytes());
    out.push(b'@');
    out.extend_from_slice(&(class_id as i16).to_le_bytes());
    Ok(())
}

fn write_utf(out: &mut Vec<u8>, s: &str) -> Result<()> {
    let bytes = s.as_bytes();
    if bytes.len() > i16::MAX as usize {
        return Err("UTF string too long".into());
    }
    out.extend_from_slice(&(bytes.len() as i16).to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn read_utf(cur: &mut Cursor<&[u8]>) -> Result<String> {
    let len = cur.read_i16::<LittleEndian>()?;
    if len < 0 {
        return Err("Negative UTF length".into());
    }
    let len = len as usize;
    if len == 0 {
        return Ok(String::new());
    }
    if cur.position() as usize + len > cur.get_ref().len() {
        return Err("UTF length exceeds buffer".into());
    }
    let mut buf = vec![0u8; len];
    cur.read_exact(&mut buf)?;
    Ok(String::from_utf8(buf)?)
}
