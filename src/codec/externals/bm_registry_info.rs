// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::codec::externals::registry;
use crate::codec::io::{DataInput, DataOutput, Result};
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::types::device_type::DeviceType;
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct BMRegistryInfo {
    pub slot_id: i16,
    pub app_id: String,
    pub current_players: Option<i16>,
    pub max_players: Option<i16>,
    pub device: DeviceCore,
    pub device_address: BMAddress,
}

impl BMRegistryInfo {
    pub const CLASS_ID: u32 = registry::BM_CLASS_ID_REGISTRY_INFO;

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let _ = input.read_short()?;
        let _ = input.read_bytes(1)?;
        let _ = input.read_short()?;
        let device = DeviceCore::read_from(input)?;

        let _ = input.read_short()?;
        let _ = input.read_bytes(1)?;
        let _ = input.read_short()?;
        let device_address = BMAddress::read_from(input)?;

        let app_id = input.read_utf()?;
        let slot_id = input.read_short()?;
        let (current_players, max_players) = if slot_id > 0 {
            (Some(input.read_short()?), Some(input.read_short()?))
        } else {
            (None, None)
        };
        let mut device = device;
        device.address = Some(device_address.clone());
        Ok(Self {
            slot_id,
            app_id,
            current_players,
            max_players,
            device,
            device_address,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        let dev_class_id: u32 = match self.device.device_type {
            DeviceType::Flash => registry::BM_CLASS_ID_FLASH_DEVICE,
            DeviceType::Unity => registry::BM_CLASS_ID_UNITY_DEVICE,
            DeviceType::IPhone => registry::BM_CLASS_ID_IPHONE_DEVICE,
            DeviceType::Android => registry::BM_CLASS_ID_ANDROID_DEVICE,
            DeviceType::Native => registry::BM_CLASS_ID_NATIVE_DEVICE,
            DeviceType::Palm => registry::BM_CLASS_ID_PALM_DEVICE,
            DeviceType::Server => registry::BM_CLASS_ID_SERVER_DEVICE,
            _ => registry::BM_CLASS_ID_FLASH_DEVICE,
        };
        out.write_short(1)?;
        out.write_bytes(&[b'@'])?;
        out.write_short(dev_class_id as i16)?;
        self.device.write_to(out)?;

        out.write_short(1)?;
        out.write_bytes(&[b'@'])?;
        out.write_short(registry::BM_CLASS_ID_ADDRESS as i16)?;
        self.device_address.write_to(out)?;
        out.write_utf(&self.app_id)?;
        out.write_short(self.slot_id)?;
        if self.slot_id > 0 {
            out.write_short(self.current_players.unwrap_or(0))?;
            out.write_short(self.max_players.unwrap_or(0))?;
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct BMRegistryInfoC {
    pub slot_id: i32,
    pub current_players: i32,
    pub max_players: i32,
    pub device_type_code: i32,

    pub app_id_ptr: *mut c_char,
    pub app_id_len: usize,

    pub device_id_ptr: *mut c_char,
    pub device_id_len: usize,

    pub device_name_ptr: *mut c_char,
    pub device_name_len: usize,

    pub addr_ptr: *mut c_char,
    pub addr_len: usize,
    pub addr_unreliable_port: i32,
    pub addr_reliable_port: i32,
}

impl BMRegistryInfoC {
    pub fn from_rust(src: BMRegistryInfo) -> Self {
        let mut out = Self::default();
        out.slot_id = src.slot_id as i32;
        out.current_players = src.current_players.unwrap_or(0) as i32;
        out.max_players = src.max_players.unwrap_or(0) as i32;
        out.device_type_code = src.device.device_type.code();
        bm_registry_info_set_app_id_inner(
            &mut out,
            std::ffi::CString::new(src.app_id).unwrap().as_ptr(),
        );
        bm_registry_info_set_device_id_inner(
            &mut out,
            std::ffi::CString::new(src.device.device_id)
                .unwrap()
                .as_ptr(),
        );
        bm_registry_info_set_device_name_inner(
            &mut out,
            std::ffi::CString::new(src.device.device_name)
                .unwrap()
                .as_ptr(),
        );
        let addr = src.device_address;
        bm_registry_info_set_addr_inner(
            &mut out,
            std::ffi::CString::new(addr.address).unwrap().as_ptr(),
        );
        out.addr_unreliable_port = addr.unreliable_port;
        out.addr_reliable_port = addr.reliable_port;
        out
    }

    pub fn to_rust(&self) -> Option<BMRegistryInfo> {
        let app_id = if self.app_id_len == 0 {
            String::new()
        } else {
            let bytes = unsafe {
                std::slice::from_raw_parts(self.app_id_ptr as *const u8, self.app_id_len)
            };
            String::from_utf8(bytes.to_vec()).ok()?
        };
        let device_id = if self.device_id_len == 0 {
            String::new()
        } else {
            let bytes = unsafe {
                std::slice::from_raw_parts(self.device_id_ptr as *const u8, self.device_id_len)
            };
            String::from_utf8(bytes.to_vec()).ok()?
        };
        let device_name = if self.device_name_len == 0 {
            String::new()
        } else {
            let bytes = unsafe {
                std::slice::from_raw_parts(self.device_name_ptr as *const u8, self.device_name_len)
            };
            String::from_utf8(bytes.to_vec()).ok()?
        };
        let addr_str = if self.addr_len == 0 {
            String::new()
        } else {
            let bytes =
                unsafe { std::slice::from_raw_parts(self.addr_ptr as *const u8, self.addr_len) };
            String::from_utf8(bytes.to_vec()).ok()?
        };
        let device_type = DeviceType::for_value(self.device_type_code).ok()?;
        let mut core =
            crate::devices::device_core::DeviceCore::new(device_id, device_name, device_type);
        let addr = BMAddress {
            address: addr_str,
            unreliable_port: self.addr_unreliable_port,
            reliable_port: self.addr_reliable_port,
        };
        core.address = Some(addr.clone());
        Some(BMRegistryInfo {
            slot_id: self.slot_id as i16,
            app_id,
            current_players: if self.slot_id > 0 {
                Some(self.current_players as i16)
            } else {
                None
            },
            max_players: if self.slot_id > 0 {
                Some(self.max_players as i16)
            } else {
                None
            },
            device: core,
            device_address: addr,
        })
    }
}

crate::ffi_cstring_accessors!(
    BMRegistryInfoC,
    app_id_ptr,
    app_id_len,
    set_inner = bm_registry_info_set_app_id_inner,
    set = bm_registry_info_set_app_id,
    get_len = bm_registry_info_get_app_id_len,
    get = bm_registry_info_get_app_id,
    free_field = bm_registry_info_free_app_id
);

crate::ffi_cstring_accessors!(
    BMRegistryInfoC,
    device_id_ptr,
    device_id_len,
    set_inner = bm_registry_info_set_device_id_inner,
    set = bm_registry_info_set_device_id,
    get_len = bm_registry_info_get_device_id_len,
    get = bm_registry_info_get_device_id,
    free_field = bm_registry_info_free_device_id
);

crate::ffi_cstring_accessors!(
    BMRegistryInfoC,
    device_name_ptr,
    device_name_len,
    set_inner = bm_registry_info_set_device_name_inner,
    set = bm_registry_info_set_device_name,
    get_len = bm_registry_info_get_device_name_len,
    get = bm_registry_info_get_device_name,
    free_field = bm_registry_info_free_device_name
);

crate::ffi_cstring_accessors!(
    BMRegistryInfoC,
    addr_ptr,
    addr_len,
    set_inner = bm_registry_info_set_addr_inner,
    set = bm_registry_info_set_addr,
    get_len = bm_registry_info_get_addr_len,
    get = bm_registry_info_get_addr,
    free_field = bm_registry_info_free_addr
);

crate::ffi_free_struct!(
    BMRegistryInfoC,
    bm_registry_info_free,
    bm_registry_info_free_app_id,
    bm_registry_info_free_device_id,
    bm_registry_info_free_device_name,
    bm_registry_info_free_addr
);

/// Frees inner fields AND the struct shell (allocated by `bm_registry_info_new`).
#[unsafe(no_mangle)]
pub extern "C" fn bm_registry_info_destroy(p: *mut BMRegistryInfoC) {
    if p.is_null() {
        return;
    }
    bm_registry_info_free(p);
    unsafe {
        drop(Box::from_raw(p));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_registry_info_new() -> *mut BMRegistryInfoC {
    catch_unwind(AssertUnwindSafe(|| {
        Box::into_raw(Box::new(BMRegistryInfoC::default()))
    }))
    .unwrap_or(std::ptr::null_mut())
}
