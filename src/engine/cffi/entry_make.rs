// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

use std::os::raw::c_char;

use crate::codec::externals::bm_registry_info::BMRegistryInfoC;
use crate::codec::messages::bm_invoke::BMInvokeC;
use crate::codec::messages::touch::Touch;
use crate::engine::processing::Engine;
use crate::types::touch_state::TouchState;

use super::marshal_out::*;
use super::types::*;
use super::*;

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_registry_register(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    registry_info_ptr: *const BMRegistryInfoC,
    domain_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || registry_info_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let reg = match unsafe { &*registry_info_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let domain = if domain_ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(domain_ptr) };
            match c_str.to_str() {
                Ok(s) => Some(s.to_owned()),
                Err(_) => return false,
            }
        };
        let actions = engine.make_registry_register(&dev_id, reg, domain, None);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_registry_list(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_registry_list(&dev_id, None);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_device_connect_requested(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    game_info_ptr: *const BMRegistryInfoC,
    controller_info_ptr: *const BMRegistryInfoC,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null()
            || out_actions.is_null()
            || game_info_ptr.is_null()
            || controller_info_ptr.is_null()
        {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let game = match unsafe { &*game_info_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let controller = match unsafe { &*controller_info_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let actions = engine.make_device_connect_requested(&dev_id, game, controller);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_registry_relay(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    dest_info_ptr: *const BMRegistryInfoC,
    inner_invoke_ptr: *const BMInvokeC,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null()
            || out_actions.is_null()
            || dest_info_ptr.is_null()
            || inner_invoke_ptr.is_null()
        {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let dest = match unsafe { &*dest_info_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let inner = match unsafe { &*inner_invoke_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let actions = engine.make_registry_relay(&dev_id, dest, inner);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_message_invoke(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    invoke_ptr: *const BMInvokeC,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || invoke_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let inv = match unsafe { &*invoke_ptr }.to_rust() {
            Some(v) => v,
            None => return false,
        };
        let actions = engine.make_message_invoke(
            &dev_id,
            &inv.method,
            inv.return_method.as_deref(),
            inv.params,
        );
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_button_invoke(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    handler_ptr: *const c_char,
    pressed: bool,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || handler_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let handler_c = unsafe { std::ffi::CStr::from_ptr(handler_ptr) };
        let handler = match handler_c.to_str() {
            Ok(s) => s,
            Err(_) => return false,
        };
        let actions = engine.make_button_invoke(&dev_id, handler, pressed);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_dpad_update(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    x: i16,
    y: i16,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_dpad_update(&dev_id, x, y);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_touch_set(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    touches_ptr: *const TouchPointC,
    touches_len: usize,
    reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        if touches_len > 0 && touches_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let mut touches = Vec::with_capacity(touches_len);
        if touches_len > 0 {
            let items = unsafe { std::slice::from_raw_parts(touches_ptr, touches_len) };
            for t in items {
                let state = match TouchState::from_value(t.state) {
                    Some(v) => v,
                    None => return false,
                };
                touches.push(Touch {
                    id: t.id,
                    x: t.x,
                    y: t.y,
                    screen_width: t.screen_width,
                    screen_height: t.screen_height,
                    state,
                });
            }
        }
        let actions = engine.make_touch_set(&dev_id, touches, reliability);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_accel(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    x: f64,
    y: f64,
    z: f64,
    reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_accel(&dev_id, x, y, z, reliability);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_gyro(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    x: f32,
    y: f32,
    z: f32,
    reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let dev_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_gyro(&dev_id, x, y, z, reliability);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_orientation(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    x: f32,
    y: f32,
    z: f32,
    w: f32,
    reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }

        let engine = unsafe { &mut *ptr_engine };
        let target_device_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            c_str.to_string_lossy().into_owned()
        };

        let actions = engine.make_orientation(&target_device_id, x, y, z, w, reliability);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_request_xml(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    width: i32,
    height: i32,
    device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let device_id = if device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };

        let actions = engine.make_request_xml(&target_id, width, height, &device_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_on_control_scheme_parsed(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let device_id = if device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };

        let actions = engine.make_on_control_scheme_parsed(&target_id, &device_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_simple_invoke(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    method_ptr: *const c_char,
    return_method_ptr: *const c_char,
    param_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || method_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let method = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(method_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let return_method = if return_method_ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(return_method_ptr) };
            match c_str.to_str() {
                Ok(s) => Some(s.to_owned()),
                Err(_) => return false,
            }
        };

        let param_str = if param_ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(param_ptr) };
            match c_str.to_str() {
                Ok(s) => Some(s.to_owned()),
                Err(_) => return false,
            }
        };

        let actions = engine.make_simple_invoke_string(
            &target_id,
            &method,
            return_method.as_deref(),
            param_str.as_deref(),
        );
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_vibrate(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_vibrate(&target_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_update_wallet(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_update_wallet(&target_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_get_cookie(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    name_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || name_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let name = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(name_ptr) };
            match c_str.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            }
        };
        let actions = engine.make_get_cookie(&target_id, name);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_cookie(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    name_ptr: *const c_char,
    value_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null()
            || out_actions.is_null()
            || name_ptr.is_null()
            || value_ptr.is_null()
        {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let name = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(name_ptr) };
            match c_str.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            }
        };
        let value = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(value_ptr) };
            match c_str.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_cookie(&target_id, name, value);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_prompt_trial_upsell(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_prompt_trial_upsell(&target_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_wait_for_new_host(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    host_device_id_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() || host_device_id_ptr.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let host_device_id = {
            let c_str = unsafe { std::ffi::CStr::from_ptr(host_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            }
        };
        let actions = engine.make_wait_for_new_host(&target_id, host_device_id);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_control_mode(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    mode: i32,
    text_content_ptr: *const c_char,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let text_content = if text_content_ptr.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(text_content_ptr) };
            match c_str.to_str() {
                Ok(s) => Some(s),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_control_mode(&target_id, mode, text_content);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_enable_accelerometer(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    enabled: bool,
    interval_seconds: f64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let interval = if interval_seconds < 0.0 {
            None
        } else {
            Some(interval_seconds)
        };
        let actions = engine.make_enable_accelerometer(&target_id, enabled, interval);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_enable_touch(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    enabled: bool,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_enable_touch(&target_id, enabled);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_touch_interval(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    interval_seconds: f64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_touch_interval(&target_id, interval_seconds);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_enable_gyro(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    enabled: bool,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_enable_gyro(&target_id, enabled);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_gyro_interval(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    interval_seconds: f64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_gyro_interval(&target_id, interval_seconds);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_enable_orientation(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    enabled: bool,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_enable_orientation(&target_id, enabled);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_orientation_interval(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    interval_seconds: f64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_orientation_interval(&target_id, interval_seconds);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_reliability_for_touch(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    touch_reliability: i32,
    control_reliability: i32,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_reliability_for_touch(
            &target_id,
            touch_reliability,
            control_reliability,
        );
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn bm_engine_make_set_capabilities(
    ptr_engine: *mut Engine,
    target_device_id_ptr: *const c_char,
    capabilities: u64,
    out_actions: *mut OutgoingListC,
) -> bool {
    catch_bool(|| {
        if ptr_engine.is_null() || out_actions.is_null() {
            return false;
        }
        let engine = unsafe { &mut *ptr_engine };
        let target_id = if target_device_id_ptr.is_null() {
            String::new()
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(target_device_id_ptr) };
            match c_str.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => return false,
            }
        };
        let actions = engine.make_set_capabilities(&target_id, capabilities);
        let list = outgoings_to_c(actions);
        unsafe {
            *out_actions = list;
        }
        true
    })
}
