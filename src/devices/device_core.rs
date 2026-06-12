// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use crate::devices::bm_address::BMAddress;
use crate::codec::io::{DataInput, DataOutput, Result};
use crate::types::device_type::DeviceType;
use std::fmt::{Debug, Display, Formatter};
use std::os::raw::c_char;
#[cfg(not(target_arch = "wasm32"))]
use std::panic::{AssertUnwindSafe, catch_unwind};
#[cfg(not(target_arch = "wasm32"))]
use std::ptr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCore {
    pub device_id: String,
    pub device_name: String,
    pub device_type: DeviceType,
    pub address: Option<BMAddress>,
}

impl DeviceCore {
    pub fn new(id: String, name: String, kind: DeviceType) -> Self {
        Self {
            device_id: id,
            device_name: name,
            device_type: kind,
            address: None,
        }
    }

    pub fn read_from(input: &mut dyn DataInput) -> Result<Self> {
        let type_int = input.read_int()?;
        let device_type = DeviceType::for_value(type_int)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        let device_id = input.read_utf()?;
        let device_name = input.read_utf()?;
        Ok(Self {
            device_id,
            device_name,
            device_type,
            address: None,
        })
    }

    pub fn write_to(&self, out: &mut dyn DataOutput) -> Result<()> {
        out.write_int(self.device_type.code())?;
        out.write_utf(&self.device_id)?;
        out.write_utf(&self.device_name)
    }
}

impl Display for DeviceCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Device[id={}, name={}, type={}]",
            self.device_id, self.device_name, self.device_type
        )
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct DeviceCoreC {
    pub device_type_code: i32,
    pub id_ptr: *mut c_char,
    pub id_len: usize,
    pub name_ptr: *mut c_char,
    pub name_len: usize,
    pub has_address: bool,
    pub addr_ptr: *mut c_char,
    pub addr_len: usize,
    pub addr_unreliable_port: i32,
    pub addr_reliable_port: i32,
}

crate::ffi_cstring_accessors!(
    DeviceCoreC,
    id_ptr,
    id_len,
    set_inner = device_core_set_id_inner,
    set = device_core_set_id,
    get_len = device_core_get_id_len,
    get = device_core_get_id,
    free_field = device_core_free_id
);

crate::ffi_cstring_accessors!(
    DeviceCoreC,
    name_ptr,
    name_len,
    set_inner = device_core_set_name_inner,
    set = device_core_set_name,
    get_len = device_core_get_name_len,
    get = device_core_get_name,
    free_field = device_core_free_name
);

crate::ffi_cstring_accessors!(
    DeviceCoreC,
    addr_ptr,
    addr_len,
    set_inner = device_core_set_addr_inner,
    set = device_core_set_addr,
    get_len = device_core_get_addr_len,
    get = device_core_get_addr,
    free_field = device_core_free_addr
);

crate::ffi_free_struct!(
    DeviceCoreC,
    device_core_free,
    device_core_free_id,
    device_core_free_name,
    device_core_free_addr
);

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
pub extern "C" fn device_core_destroy(p: *mut DeviceCoreC) {
    if p.is_null() {
        return;
    }
    device_core_free(p);
    unsafe {
        drop(Box::from_raw(p));
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
pub extern "C" fn device_core_new() -> *mut DeviceCoreC {
    catch_unwind(AssertUnwindSafe(|| {
        Box::into_raw(Box::new(DeviceCoreC {
            device_type_code: 0,
            id_ptr: ptr::null_mut(),
            id_len: 0,
            name_ptr: ptr::null_mut(),
            name_len: 0,
            has_address: false,
            addr_ptr: ptr::null_mut(),
            addr_len: 0,
            addr_unreliable_port: 0,
            addr_reliable_port: 0,
        }))
    }))
    .unwrap_or(std::ptr::null_mut())
}

impl DeviceCoreC {
    pub fn to_rust(&self) -> Option<DeviceCore> {
        let dt = DeviceType::for_value(self.device_type_code).ok()?;

        let id = if self.id_len == 0 {
            String::new()
        } else {
            let bytes =
                unsafe { std::slice::from_raw_parts(self.id_ptr as *const u8, self.id_len) };
            std::str::from_utf8(bytes).ok()?.to_owned()
        };

        let name = if self.name_len == 0 {
            String::new()
        } else {
            let bytes =
                unsafe { std::slice::from_raw_parts(self.name_ptr as *const u8, self.name_len) };
            std::str::from_utf8(bytes).ok()?.to_owned()
        };

        let mut dc = DeviceCore::new(id, name, dt);

        if self.has_address {
            let addr_str = if self.addr_len == 0 {
                String::new()
            } else {
                let bytes = unsafe {
                    std::slice::from_raw_parts(self.addr_ptr as *const u8, self.addr_len)
                };
                std::str::from_utf8(bytes).ok()?.to_owned()
            };
            dc.address = Some(BMAddress {
                address: addr_str,
                unreliable_port: self.addr_unreliable_port,
                reliable_port: self.addr_reliable_port,
            });
        }

        Some(dc)
    }
}
