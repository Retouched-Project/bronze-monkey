// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::types::device_type::DeviceType;
use std::os::raw::c_char;

// Packet class IDs
pub const BM_CLASS_ID_PACKET: u32 = 0;
pub const BM_CLASS_ID_ADDRESS: u32 = 1;
pub const BM_CLASS_ID_PARAMETER: u32 = 3;
pub const BM_CLASS_ID_INVOKE: u32 = 4;
pub const BM_CLASS_ID_ACCELERATION: u32 = 5;
pub const BM_CLASS_ID_TOUCH_SET: u32 = 6;
pub const BM_CLASS_ID_ACK_PACKET: u32 = 9;
pub const BM_CLASS_ID_PING: u32 = 11;
pub const BM_CLASS_ID_STRING_LITERAL: u32 = 12;
pub const BM_CLASS_ID_SHAKE: u32 = 13;
pub const BM_CLASS_ID_BYTE_CHUNK: u32 = 14;
pub const BM_CLASS_ID_REGISTRY_INFO: u32 = 19;
pub const BM_CLASS_ID_ARRAY: u32 = 21;
pub const BM_CLASS_ID_GYRO: u32 = 22;
pub const BM_CLASS_ID_ORIENTATION: u32 = 23;
pub const BM_CLASS_ID_DPAD_UPDATE: u32 = 24;

// Device class IDs
pub const BM_CLASS_ID_IPHONE_DEVICE: u32 = 7;
pub const BM_CLASS_ID_UNITY_DEVICE: u32 = 8;
pub const BM_CLASS_ID_ANDROID_DEVICE: u32 = 10;
pub const BM_CLASS_ID_NATIVE_DEVICE: u32 = 15;
pub const BM_CLASS_ID_PALM_DEVICE: u32 = 16;
pub const BM_CLASS_ID_SERVER_DEVICE: u32 = 17;
pub const BM_CLASS_ID_FLASH_DEVICE: u32 = 18;

/// The class id to write in front of a device of this type.
///
/// A device carries its type in its own body, so the class id beside it tags
/// the serialization class rather than repeating the type, and a reader takes
/// the type from the body.
///
/// No class exists for a device of no particular type, so [`DeviceType::Any`]
/// borrows the Palm one. Not every reader knows every class, and a reader that
/// meets one it does not know gives up on the whole message rather than the one
/// field, so the choice is whichever class the most readers accept. Palm is
/// among those, and no live platform claims it.
pub fn class_id_for_device_type(dt: DeviceType) -> u32 {
    match dt {
        DeviceType::Flash => BM_CLASS_ID_FLASH_DEVICE,
        DeviceType::Palm | DeviceType::Any => BM_CLASS_ID_PALM_DEVICE,
        DeviceType::Unity => BM_CLASS_ID_UNITY_DEVICE,
        DeviceType::IPhone => BM_CLASS_ID_IPHONE_DEVICE,
        DeviceType::Android => BM_CLASS_ID_ANDROID_DEVICE,
        DeviceType::Native => BM_CLASS_ID_NATIVE_DEVICE,
        DeviceType::Server => BM_CLASS_ID_SERVER_DEVICE,
    }
}

const ENTRIES: &[(u32, &str)] = &[
    (BM_CLASS_ID_PACKET, "BMPacket"),
    (BM_CLASS_ID_ADDRESS, "BMAddress"),
    (BM_CLASS_ID_PARAMETER, "BMParameter"),
    (BM_CLASS_ID_INVOKE, "BMInvoke"),
    (BM_CLASS_ID_ACCELERATION, "Acceleration"),
    (BM_CLASS_ID_TOUCH_SET, "TouchSet"),
    (BM_CLASS_ID_ACK_PACKET, "AckPacket"),
    (BM_CLASS_ID_PING, "Ping"),
    (BM_CLASS_ID_STRING_LITERAL, "StringLiteral"),
    (BM_CLASS_ID_SHAKE, "Shake"),
    (BM_CLASS_ID_BYTE_CHUNK, "BMByteChunk"),
    (BM_CLASS_ID_REGISTRY_INFO, "BMRegistryInfo"),
    (BM_CLASS_ID_ARRAY, "BMArray"),
    (BM_CLASS_ID_GYRO, "BMGyro"),
    (BM_CLASS_ID_ORIENTATION, "Orientation"),
    (BM_CLASS_ID_DPAD_UPDATE, "DPadUpdate"),
    (BM_CLASS_ID_NATIVE_DEVICE, "NativeDevice"),
    (BM_CLASS_ID_SERVER_DEVICE, "ServerDevice"),
    (BM_CLASS_ID_FLASH_DEVICE, "FlashDevice"),
    (BM_CLASS_ID_PALM_DEVICE, "PalmDevice"),
    (BM_CLASS_ID_IPHONE_DEVICE, "IPhoneDevice"),
    (BM_CLASS_ID_ANDROID_DEVICE, "AndroidDevice"),
    (BM_CLASS_ID_UNITY_DEVICE, "UnityDevice"),
];

#[inline]
fn find(id: u32) -> Option<&'static (u32, &'static str)> {
    ENTRIES.iter().find(|(k, _)| *k == id)
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_registry_count() -> usize {
    ENTRIES.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_registry_has(id: u32) -> bool {
    find(id).is_some()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_registry_ids(out: *mut u32, out_len: usize) -> usize {
    if out.is_null() || out_len == 0 {
        return 0;
    }
    let n = ENTRIES.len().min(out_len);
    for i in 0..n {
        unsafe {
            *out.add(i) = ENTRIES[i].0;
        }
    }
    n
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_registry_name_len(id: u32) -> usize {
    find(id).map(|(_, name)| name.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bm_registry_name(id: u32, out: *mut c_char, out_len: usize) -> usize {
    if out.is_null() || out_len == 0 {
        return 0;
    }
    let Some((_, name)) = find(id) else {
        unsafe { *out = 0 };
        return 0;
    };
    let bytes = name.as_bytes();
    let n = bytes.len().min(out_len.saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
        *out.add(n) = 0;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_device_type_is_written_with_a_device_class() {
        let device_classes = [
            BM_CLASS_ID_IPHONE_DEVICE,
            BM_CLASS_ID_UNITY_DEVICE,
            BM_CLASS_ID_ANDROID_DEVICE,
            BM_CLASS_ID_NATIVE_DEVICE,
            BM_CLASS_ID_PALM_DEVICE,
            BM_CLASS_ID_SERVER_DEVICE,
            BM_CLASS_ID_FLASH_DEVICE,
        ];
        for kind in DeviceType::ALL {
            let id = class_id_for_device_type(kind);
            assert!(
                device_classes.contains(&id),
                "{} was written with class {id}, which is not a device",
                kind.label()
            );
        }
    }

    #[test]
    fn a_device_of_no_particular_type_borrows_the_palm_class() {
        assert_eq!(
            class_id_for_device_type(DeviceType::Any),
            BM_CLASS_ID_PALM_DEVICE
        );
    }

    /// Three device classes are missing from at least one reader in the wild,
    /// and meeting an unknown class costs that reader the whole message. A type
    /// we cannot name exactly must not be written with one of those.
    #[test]
    fn an_unnamed_device_type_is_written_with_a_widely_known_class() {
        let known_everywhere = [
            BM_CLASS_ID_IPHONE_DEVICE,
            BM_CLASS_ID_UNITY_DEVICE,
            BM_CLASS_ID_ANDROID_DEVICE,
            BM_CLASS_ID_PALM_DEVICE,
        ];
        assert!(known_everywhere.contains(&class_id_for_device_type(DeviceType::Any)));
    }
}
