// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

#![cfg(feature = "pyo3")]

use crate::codec::externals::bm_array::BMArray;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::externals::bm_version::BMVersion;
use crate::codec::externals::handshake::handshake_bytes;
use crate::codec::externals::registry;
use crate::codec::io::DataOutput;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::messages::bm_parameter::VecOutput;
use crate::codec::object::Object;
use crate::controls::ControlScheme;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::engine::events::{Command, Outgoing};
use crate::engine::processing::Engine;
use crate::engine::protocol::{
    deserialize_packet as protocol_deserialize_packet, serialize_packet,
};
use crate::engine::registry::DeviceRecord;
use crate::types::device_type::DeviceType;
use crate::types::packet_type::PacketType;
use prost::Message;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyModule, PyString};
use pythonize::{depythonize, pythonize};
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

static PY_LOG_CALLBACK: Mutex<Option<Py<PyAny>>> = Mutex::new(None);

pub fn try_python_log(msg: &str) -> bool {
    Python::attach(|py| {
        let guard = PY_LOG_CALLBACK.lock().unwrap();
        let Some(cb) = guard.as_ref() else {
            return false;
        };
        cb.call1(py, (msg,)).is_ok()
    })
}

#[pyfunction]
fn set_log_callback(callback: Option<Py<PyAny>>) -> PyResult<()> {
    let mut slot = PY_LOG_CALLBACK.lock().unwrap();
    *slot = callback;
    crate::flush_pending_log();
    Ok(())
}

#[pyclass]
pub struct BMEnginePy {
    inner: RwLock<Engine>,
}

#[pymethods]
impl BMEnginePy {
    #[new]
    fn new() -> Self {
        Self {
            inner: RwLock::new(Engine::new()),
        }
    }

    fn init_local_device(&self, device_id: String, device_name: String, device_type: i32) {
        let mut eng = self.inner.write().unwrap();
        let dt = DeviceType::for_value(device_type).unwrap_or(DeviceType::Server);
        let core = DeviceCore::new(device_id, device_name, dt);
        eng.init_local_device(core);
    }

    fn init_local_device_with_address(
        &self,
        device_id: String,
        device_name: String,
        device_type: i32,
        address: String,
        unreliable_port: i32,
        reliable_port: i32,
    ) {
        let mut eng = self.inner.write().unwrap();
        let dt = DeviceType::for_value(device_type).unwrap_or(DeviceType::Server);
        let mut core = DeviceCore::new(device_id, device_name, dt);
        core.address = Some(BMAddress {
            address,
            unreliable_port,
            reliable_port,
        });
        eng.init_local_device(core);
    }

    fn drop_device<'py>(&self, py: Python<'py>, device_id: String) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().drop_device(&device_id);
        outgoings_to_py(py, actions)
    }

    fn get_registry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let eng = self.inner.read().unwrap();
        let registry = eng.registry();
        let mut py_records = Vec::new();
        for record in registry.snapshot() {
            py_records.push(device_record_to_py(py, &record)?);
        }
        Ok(PyList::new(py, py_records)?)
    }

    fn process_incoming<'py>(&self, py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
        let out = self.inner.write().unwrap().process_incoming(data);
        Ok(pythonize(py, &out)?)
    }

    fn process_incoming_udp<'py>(
        &self,
        py: Python<'py>,
        data: &[u8],
    ) -> PyResult<Bound<'py, PyAny>> {
        let out = self.inner.write().unwrap().process_incoming_udp(data);
        Ok(pythonize(py, &out)?)
    }

    fn emit<'py>(
        &self,
        py: Python<'py>,
        command: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let cmd: Command = depythonize(&command)?;
        let outgoings = self.inner.write().unwrap().emit(cmd);
        outgoings_to_py(py, outgoings)
    }

    fn register_button_handlers(&self, handlers: Vec<String>) {
        self.inner
            .write()
            .unwrap()
            .register_button_handlers(handlers);
    }

    fn clear_button_handlers(&self) {
        self.inner.write().unwrap().clear_button_handlers();
    }

    fn make_on_host_connected<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        client_info: Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyList>> {
        let info = dict_to_registry_info(&client_info)?;
        let actions = self.inner.write().unwrap().make_message_invoke(
            &target_device_id,
            crate::engine::methods::ON_HOST_CONNECTED,
            None,
            vec![Value::Object(Object::BMRegistryInfo(info))],
        );
        outgoings_to_py(py, actions)
    }

    fn make_message_invoke<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        method: String,
        return_method: Option<String>,
        params: Bound<'py, PyList>,
    ) -> PyResult<Bound<'py, PyList>> {
        let mut rust_params = Vec::new();
        for p in params.iter() {
            rust_params.push(py_to_value(&p)?);
        }

        let actions = self.inner.write().unwrap().make_message_invoke(
            &target_device_id,
            &method,
            return_method.as_deref(),
            rust_params,
        );
        outgoings_to_py(py, actions)
    }

    fn register_device(
        &self,
        device_id: String,
        device_name: String,
        device_type: i32,
        address: String,
        unreliable_port: i32,
        reliable_port: i32,
    ) {
        let mut eng = self.inner.write().unwrap();
        let dt = DeviceType::for_value(device_type).unwrap_or(DeviceType::Server);
        let mut core = DeviceCore::new(device_id, device_name, dt);
        core.address = Some(BMAddress {
            address,
            unreliable_port,
            reliable_port,
        });
        let record = DeviceRecord::new(core, None, None);
        eng.registry_mut().upsert(record);
    }

    fn set_auto_approve_registration(&self, value: bool) {
        let mut eng = self.inner.write().unwrap();
        eng.server_policy.auto_approve_registration = value;
    }

    fn approve_registration<'py>(
        &self,
        py: Python<'py>,
        device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().approve_registration(&device_id);
        outgoings_to_py(py, actions)
    }

    fn deny_registration<'py>(
        &self,
        py: Python<'py>,
        device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().deny_registration(&device_id);
        outgoings_to_py(py, actions)
    }

    fn make_vibrate<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().make_vibrate(&target_device_id);
        outgoings_to_py(py, actions)
    }

    fn make_update_wallet<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_update_wallet(&target_device_id);
        outgoings_to_py(py, actions)
    }

    fn make_get_cookie<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        name: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_get_cookie(&target_device_id, &name);
        outgoings_to_py(py, actions)
    }

    fn make_set_cookie<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        name: String,
        value: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_set_cookie(&target_device_id, &name, &value);
        outgoings_to_py(py, actions)
    }

    fn make_prompt_trial_upsell<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_prompt_trial_upsell(&target_device_id);
        outgoings_to_py(py, actions)
    }

    fn make_wait_for_new_host<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        host_device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_wait_for_new_host(&target_device_id, &host_device_id);
        outgoings_to_py(py, actions)
    }

    fn make_set_control_mode<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        mode: i32,
        text_content: Option<String>,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().make_set_control_mode(
            &target_device_id,
            mode,
            text_content.as_deref(),
        );
        outgoings_to_py(py, actions)
    }

    fn make_enable_accelerometer<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        enabled: bool,
        interval_seconds: Option<f64>,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().make_enable_accelerometer(
            &target_device_id,
            enabled,
            interval_seconds,
        );
        outgoings_to_py(py, actions)
    }

    fn make_enable_touch<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        enabled: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_enable_touch(&target_device_id, enabled);
        outgoings_to_py(py, actions)
    }

    fn make_set_touch_interval<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        interval_seconds: f64,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_set_touch_interval(&target_device_id, interval_seconds);
        outgoings_to_py(py, actions)
    }

    fn make_enable_gyro<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        enabled: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_enable_gyro(&target_device_id, enabled);
        outgoings_to_py(py, actions)
    }

    fn make_set_gyro_interval<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        interval_seconds: f64,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_set_gyro_interval(&target_device_id, interval_seconds);
        outgoings_to_py(py, actions)
    }

    fn make_enable_orientation<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        enabled: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_enable_orientation(&target_device_id, enabled);
        outgoings_to_py(py, actions)
    }

    fn make_set_orientation_interval<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        interval_seconds: f64,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_set_orientation_interval(&target_device_id, interval_seconds);
        outgoings_to_py(py, actions)
    }

    fn make_set_reliability_for_touch<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        touch_reliability: i32,
        control_reliability: i32,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().make_set_reliability_for_touch(
            &target_device_id,
            touch_reliability,
            control_reliability,
        );
        outgoings_to_py(py, actions)
    }

    fn make_registry_register<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        info: Bound<'py, PyDict>,
        domain: Option<String>,
    ) -> PyResult<Bound<'py, PyList>> {
        let reg_info = dict_to_registry_info(&info)?;
        let actions = self.inner.write().unwrap().make_registry_register(
            &target_device_id,
            reg_info,
            domain,
            None,
        );
        outgoings_to_py(py, actions)
    }

    fn make_registry_list<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_registry_list(&target_device_id, None);
        outgoings_to_py(py, actions)
    }

    fn make_registry_relay<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        dest_info: Bound<'py, PyDict>,
        inner_method: String,
        inner_return_method: Option<String>,
        inner_params: Bound<'py, PyList>,
    ) -> PyResult<Bound<'py, PyList>> {
        let dest = dict_to_registry_info(&dest_info)?;
        let mut rust_params = Vec::new();
        for p in inner_params.iter() {
            rust_params.push(py_to_value(&p)?);
        }
        let inner = BMInvoke {
            id: 0,
            method: inner_method,
            return_method: inner_return_method,
            params: rust_params,
        };
        let actions =
            self.inner
                .write()
                .unwrap()
                .make_registry_relay(&target_device_id, dest, inner);
        outgoings_to_py(py, actions)
    }

    fn make_device_connect_requested<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        game_info: Bound<'py, PyDict>,
        controller_info: Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyList>> {
        let game = dict_to_registry_info(&game_info)?;
        let controller = dict_to_registry_info(&controller_info)?;
        let actions = self.inner.write().unwrap().make_device_connect_requested(
            &target_device_id,
            game,
            controller,
        );
        outgoings_to_py(py, actions)
    }

    fn make_button_invoke<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        handler: String,
        pressed: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions =
            self.inner
                .write()
                .unwrap()
                .make_button_invoke(&target_device_id, &handler, pressed);
        outgoings_to_py(py, actions)
    }

    fn make_dpad_update<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        x: i16,
        y: i16,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_dpad_update(&target_device_id, x, y);
        outgoings_to_py(py, actions)
    }

    fn make_touch_set<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        touches: Bound<'py, PyList>,
        reliability: i32,
    ) -> PyResult<Bound<'py, PyList>> {
        use crate::codec::messages::touch::Touch;
        use crate::types::touch_state::TouchState;
        let mut rust_touches = Vec::new();
        for t in touches.iter() {
            let td = t.cast::<PyDict>()?;
            let id: i32 = td
                .get_item("id")?
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing touch id"))?
                .extract()?;
            let x: f64 = td
                .get_item("x")?
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing touch x"))?
                .extract()?;
            let y: f64 = td
                .get_item("y")?
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing touch y"))?
                .extract()?;
            let sw: i16 = td
                .get_item("screen_width")?
                .map(|v| v.extract())
                .transpose()?
                .unwrap_or(0);
            let sh: i16 = td
                .get_item("screen_height")?
                .map(|v| v.extract())
                .transpose()?
                .unwrap_or(0);
            let state_val: i32 = td
                .get_item("state")?
                .map(|v| v.extract())
                .transpose()?
                .unwrap_or(0);
            let state = TouchState::from_value(state_val).ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid touch state")
            })?;
            rust_touches.push(Touch {
                id,
                x,
                y,
                screen_width: sw,
                screen_height: sh,
                state,
            });
        }
        let actions = self.inner.write().unwrap().make_touch_set(
            &target_device_id,
            rust_touches,
            reliability,
        );
        outgoings_to_py(py, actions)
    }

    fn make_accel<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        x: f64,
        y: f64,
        z: f64,
        reliability: i32,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions =
            self.inner
                .write()
                .unwrap()
                .make_accel(&target_device_id, x, y, z, reliability);
        outgoings_to_py(py, actions)
    }

    fn make_gyro<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        x: f32,
        y: f32,
        z: f32,
        reliability: i32,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions =
            self.inner
                .write()
                .unwrap()
                .make_gyro(&target_device_id, x, y, z, reliability);
        outgoings_to_py(py, actions)
    }

    fn make_orientation<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        reliability: i32,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().make_orientation(
            &target_device_id,
            x,
            y,
            z,
            w,
            reliability,
        );
        outgoings_to_py(py, actions)
    }

    fn make_request_xml<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        width: i32,
        height: i32,
        device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().make_request_xml(
            &target_device_id,
            width,
            height,
            &device_id,
        );
        outgoings_to_py(py, actions)
    }

    fn make_on_control_scheme_parsed<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        device_id: String,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_on_control_scheme_parsed(&target_device_id, &device_id);
        outgoings_to_py(py, actions)
    }

    fn make_set_capabilities<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        capabilities: u64,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self
            .inner
            .write()
            .unwrap()
            .make_set_capabilities(&target_device_id, capabilities);
        outgoings_to_py(py, actions)
    }

    fn make_simple_invoke<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        method: String,
        return_method: Option<String>,
        param: Option<String>,
    ) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().make_simple_invoke_string(
            &target_device_id,
            &method,
            return_method.as_deref(),
            param.as_deref(),
        );
        outgoings_to_py(py, actions)
    }

    fn make_packet<'py>(
        &self,
        py: Python<'py>,
        target_device_id: String,
        channel: i32,
        reliability: i32,
        packet_type_code: i32,
        message: Option<Vec<u8>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let rel = if reliability < 0 {
            None
        } else {
            Some(reliability)
        };
        let pt = PacketType::from_i32(packet_type_code).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>("invalid packet_type_code")
        })?;
        let actions =
            self.inner
                .write()
                .unwrap()
                .make_packet(&target_device_id, channel, rel, pt, message);
        outgoings_to_py(py, actions)
    }
}

#[pyfunction]
fn handshake<'py>(
    py: Python<'py>,
    current: Option<(u8, u8, u16)>,
    minimum: Option<(u8, u8, u16)>,
) -> PyResult<Bound<'py, PyBytes>> {
    let current = current.map(|(maj, min, build)| BMVersion::new(maj, min, build));
    let minimum = minimum.map(|(maj, min, build)| BMVersion::new(maj, min, build));
    let bytes = handshake_bytes(current, minimum);
    Ok(PyBytes::new(py, &bytes))
}

#[pyfunction]
fn serialize_invoke_packet<'py>(
    py: Python<'py>,
    method: String,
    params: Bound<'py, PyList>,
    sequence: i32,
    return_method: Option<String>,
    device_id: String,
    device_name: String,
    channel: i32,
    reliability: i32,
    packet_type_code: i32,
    device_type_code: i32,
) -> PyResult<Bound<'py, PyBytes>> {
    let mut rust_params = Vec::new();
    for p in params.iter() {
        rust_params.push(py_to_value(&p)?);
    }

    let invoke = BMInvoke {
        id: sequence,
        method,
        return_method,
        params: rust_params,
    };

    let message_bytes = invoke
        .to_object_bytes()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    let packet = BMPacket {
        sequence,
        channel,
        timestamp: now_ms_f64(),
        rtt: 0.0,
        packet_type: PacketType::from_i32(packet_type_code).unwrap_or(PacketType::Data),
        device_type: DeviceType::for_value(device_type_code).unwrap_or(DeviceType::Server),
        reliability,
        device_name,
        device_id,
        message: Some(message_bytes),
        address_host: None,
        addr_unreliable_port: 0,
        addr_reliable_port: 0,
    };

    let packet_bytes = serialize_packet(&packet)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    Ok(PyBytes::new(py, &packet_bytes))
}

#[pyfunction]
fn serialize_device_packet<'py>(
    py: Python<'py>,
    device_id: String,
    device_name: String,
    device_type_code: i32,
    sequence: i32,
    channel: i32,
    packet_type_code: i32,
) -> PyResult<Bound<'py, PyBytes>> {
    let device_type = DeviceType::for_value(device_type_code).unwrap_or(DeviceType::Server);
    let class_id = match device_type {
        DeviceType::Flash => registry::BM_CLASS_ID_FLASH_DEVICE,
        DeviceType::Unity => registry::BM_CLASS_ID_UNITY_DEVICE,
        DeviceType::IPhone => registry::BM_CLASS_ID_IPHONE_DEVICE,
        DeviceType::Android => registry::BM_CLASS_ID_ANDROID_DEVICE,
        DeviceType::Native => registry::BM_CLASS_ID_NATIVE_DEVICE,
        DeviceType::Palm => registry::BM_CLASS_ID_PALM_DEVICE,
        DeviceType::Server => registry::BM_CLASS_ID_SERVER_DEVICE,
        _ => registry::BM_CLASS_ID_FLASH_DEVICE,
    };

    let core = DeviceCore::new(device_id.clone(), device_name.clone(), device_type);
    let mut out = VecOutput::default();
    out.write_short(1)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    out.write_bytes(&[b'@'])
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    out.write_short(class_id as i16)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    core.write_to(&mut out)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

    let packet = BMPacket {
        sequence,
        channel,
        timestamp: now_ms_f64(),
        rtt: 0.0,
        packet_type: PacketType::from_i32(packet_type_code).unwrap_or(PacketType::Data),
        device_type,
        reliability: 0,
        device_name,
        device_id,
        message: Some(out.buf),
        address_host: None,
        addr_unreliable_port: 0,
        addr_reliable_port: 0,
    };

    let packet_bytes = serialize_packet(&packet)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    Ok(PyBytes::new(py, &packet_bytes))
}

#[pyfunction]
fn deserialize_packet_dict<'py>(py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyDict>> {
    let mut pkt = BMPacket::default();
    protocol_deserialize_packet(data, &mut pkt)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    let d = PyDict::new(py);
    d.set_item("sequence", pkt.sequence)?;
    d.set_item("channel", pkt.channel)?;
    d.set_item("timestamp", pkt.timestamp)?;
    d.set_item("rtt", pkt.rtt)?;
    d.set_item("packet_type", pkt.packet_type.code())?;
    d.set_item("device_type", pkt.device_type.code())?;
    d.set_item("reliability", pkt.reliability)?;
    d.set_item("device_id", pkt.device_id.clone())?;
    d.set_item("device_name", pkt.device_name.clone())?;
    if let Some(msg) = pkt.message.as_ref() {
        d.set_item("message_bytes", PyBytes::new(py, msg))?;
        let mut cur = std::io::Cursor::new(msg.as_slice());
        match Object::decode(&mut cur) {
            Ok(obj) => {
                d.set_item("message", object_to_py(py, &obj)?)?;
            }
            Err(e) => {
                d.set_item("message_error", e.to_string())?;
            }
        }
    }
    Ok(d)
}

#[pyfunction]
fn get_device_type_codes<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("ANY", DeviceType::Any.code())?;
    d.set_item("UNITY", DeviceType::Unity.code())?;
    d.set_item("IPHONE", DeviceType::IPhone.code())?;
    d.set_item("FLASH", DeviceType::Flash.code())?;
    d.set_item("ANDROID", DeviceType::Android.code())?;
    d.set_item("NATIVE", DeviceType::Native.code())?;
    d.set_item("PALM", DeviceType::Palm.code())?;
    d.set_item("SERVER", DeviceType::Server.code())?;
    Ok(d)
}

#[pyfunction]
fn get_packet_type_codes<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("DATA", PacketType::Data.code())?;
    d.set_item("PING", PacketType::Ping.code())?;
    d.set_item("ACK", PacketType::Ack.code())?;
    d.set_item("ECHO", PacketType::Echo.code())?;
    d.set_item("ANALYSIS", PacketType::Analysis.code())?;
    d.set_item("KEEP_ALIVE", PacketType::KeepAlive.code())?;
    Ok(d)
}

#[pyfunction]
fn parse_control_scheme_xml<'py>(
    py: Python<'py>,
    xml_data: String,
) -> PyResult<Bound<'py, PyBytes>> {
    let mut parser = BMApplicationSchemeParser::new();
    let scheme = parser
        .parse(xml_data.as_bytes())
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;
    let mut buf = Vec::with_capacity(scheme.encoded_len());
    scheme
        .encode(&mut buf)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    Ok(PyBytes::new(py, &buf))
}

#[pyfunction]
fn merge_control_scheme<'py>(
    py: Python<'py>,
    base: &[u8],
    update: &[u8],
) -> PyResult<Bound<'py, PyBytes>> {
    let mut base_scheme = ControlScheme::decode(base)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    let update_scheme = ControlScheme::decode(update)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    crate::controls::merge::apply_update(&mut base_scheme, update_scheme);
    let mut buf = Vec::with_capacity(base_scheme.encoded_len());
    base_scheme
        .encode(&mut buf)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    Ok(PyBytes::new(py, &buf))
}

#[pymodule]
#[pyo3(name = "bronze_monkey")]
fn bronze_monkey_py(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    crate::log_library_loaded("pyo3");
    m.add_class::<BMEnginePy>()?;
    m.add_function(wrap_pyfunction!(handshake, m)?)?;
    m.add_function(wrap_pyfunction!(serialize_invoke_packet, m)?)?;
    m.add_function(wrap_pyfunction!(serialize_device_packet, m)?)?;
    m.add_function(wrap_pyfunction!(deserialize_packet_dict, m)?)?;
    m.add_function(wrap_pyfunction!(set_log_callback, m)?)?;
    m.add_function(wrap_pyfunction!(get_device_type_codes, m)?)?;
    m.add_function(wrap_pyfunction!(get_packet_type_codes, m)?)?;
    m.add_function(wrap_pyfunction!(parse_control_scheme_xml, m)?)?;
    m.add_function(wrap_pyfunction!(merge_control_scheme, m)?)?;
    m.add("DEVICE_TYPE_ANY", DeviceType::Any.code())?;
    m.add("DEVICE_TYPE_UNITY", DeviceType::Unity.code())?;
    m.add("DEVICE_TYPE_IPHONE", DeviceType::IPhone.code())?;
    m.add("DEVICE_TYPE_FLASH", DeviceType::Flash.code())?;
    m.add("DEVICE_TYPE_ANDROID", DeviceType::Android.code())?;
    m.add("DEVICE_TYPE_NATIVE", DeviceType::Native.code())?;
    m.add("DEVICE_TYPE_PALM", DeviceType::Palm.code())?;
    m.add("DEVICE_TYPE_SERVER", DeviceType::Server.code())?;
    m.add("PACKET_TYPE_DATA", PacketType::Data.code())?;
    m.add("PACKET_TYPE_PING", PacketType::Ping.code())?;
    m.add("PACKET_TYPE_ACK", PacketType::Ack.code())?;
    m.add("PACKET_TYPE_ECHO", PacketType::Echo.code())?;
    m.add("PACKET_TYPE_ANALYSIS", PacketType::Analysis.code())?;
    m.add("PACKET_TYPE_KEEP_ALIVE", PacketType::KeepAlive.code())?;
    m.add("__actions__", PyList::empty(py))?;
    Ok(())
}

fn outgoings_to_py<'py>(py: Python<'py>, outgoings: Vec<Outgoing>) -> PyResult<Bound<'py, PyList>> {
    let items = outgoings
        .iter()
        .map(|o| pythonize(py, o).map_err(PyErr::from))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyList::new(py, items)?)
}

fn value_to_py(py: Python<'_>, v: &Value) -> PyResult<Py<PyAny>> {
    Ok(match v {
        Value::String(s) => s.into_pyobject(py)?.as_any().clone().unbind().into(),
        Value::Bool(b) => b.into_pyobject(py)?.as_any().clone().unbind().into(),
        Value::I16(i) => (*i as i64)
            .into_pyobject(py)?
            .as_any()
            .clone()
            .unbind()
            .into(),
        Value::U16(i) => (*i as u64)
            .into_pyobject(py)?
            .as_any()
            .clone()
            .unbind()
            .into(),
        Value::I32(i) => (*i as i64)
            .into_pyobject(py)?
            .as_any()
            .clone()
            .unbind()
            .into(),
        Value::U32(i) => (*i as u64)
            .into_pyobject(py)?
            .as_any()
            .clone()
            .unbind()
            .into(),
        Value::F32(f) => (*f as f64)
            .into_pyobject(py)?
            .as_any()
            .clone()
            .unbind()
            .into(),
        Value::F64(f) => f.into_pyobject(py)?.as_any().clone().unbind().into(),
        Value::Object(o) => object_to_py(py, o)?,
    })
}

fn object_to_py(py: Python<'_>, o: &Object) -> PyResult<Py<PyAny>> {
    match o {
        Object::BMRegistryInfo(info) => registry_info_to_py(py, info),
        Object::BMParameter(v) => value_to_py(py, v),
        Object::BMArray(arr) => {
            let mut py_items = Vec::new();
            for item in &arr.items {
                py_items.push(value_to_py(py, item)?);
            }
            Ok(PyList::new(py, py_items)?.into())
        }
        Object::BMAddress(addr) => address_to_py(py, addr),
        Object::BMInvoke(inv) => {
            let d = PyDict::new(py);
            d.set_item("method", inv.method.clone())?;
            d.set_item("return_method", inv.return_method.clone())?;
            let py_params: Vec<Py<PyAny>> = inv
                .params
                .iter()
                .map(|v| value_to_py(py, v))
                .collect::<PyResult<_>>()?;
            d.set_item("params", PyList::new(py, py_params)?)?;
            Ok(d.into())
        }
        _ => {
            let d = PyDict::new(py);
            d.set_item("class_id", o.class_id())?;
            Ok(d.into())
        }
    }
}

fn registry_info_to_py(py: Python<'_>, info: &BMRegistryInfo) -> PyResult<Py<PyAny>> {
    let d = PyDict::new(py);
    d.set_item("slot_id", info.slot_id)?;
    d.set_item("app_id", info.app_id.clone())?;
    d.set_item("current_players", info.current_players)?;
    d.set_item("max_players", info.max_players)?;
    d.set_item("device", device_core_to_py(py, &info.device)?)?;
    d.set_item("device_address", address_to_py(py, &info.device_address)?)?;
    Ok(d.into())
}

fn device_record_to_py(py: Python<'_>, rec: &DeviceRecord) -> PyResult<Py<PyAny>> {
    let d = PyDict::new(py);
    d.set_item("device", device_core_to_py(py, &rec.core)?)?;
    d.set_item("class_id", rec.class_id.map(|c| c as i32))?;
    Ok(d.into())
}

fn device_core_to_py(py: Python<'_>, dev: &DeviceCore) -> PyResult<Py<PyAny>> {
    let d = PyDict::new(py);
    d.set_item("id", dev.device_id.clone())?;
    d.set_item("name", dev.device_name.clone())?;
    d.set_item("device_type", dev.device_type.code())?;
    if let Some(addr) = dev.address.as_ref() {
        d.set_item("address", address_to_py(py, addr)?)?;
    }
    Ok(d.into())
}

fn address_to_py(py: Python<'_>, addr: &BMAddress) -> PyResult<Py<PyAny>> {
    let d = PyDict::new(py);
    d.set_item("address", addr.address.clone())?;
    d.set_item("unreliable_port", addr.unreliable_port)?;
    d.set_item("reliable_port", addr.reliable_port)?;
    Ok(d.into())
}

fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if let Ok(d) = obj.cast::<PyDict>() {
        if let Some(class_id) = d.get_item("class_id")? {
            let class_id: u32 = class_id.extract()?;
            return match class_id {
                registry::BM_CLASS_ID_ARRAY => {
                    let mut arr = BMArray::default();
                    if let Some(items) = d.get_item("items")? {
                        let list = items.cast::<PyList>()?;
                        for item in list.iter() {
                            arr.push(py_to_value(&item)?);
                        }
                    }
                    Ok(Value::Object(Object::BMArray(arr)))
                }
                registry::BM_CLASS_ID_REGISTRY_INFO => {
                    let info = dict_to_registry_info(d)?;
                    Ok(Value::Object(Object::BMRegistryInfo(info)))
                }
                registry::BM_CLASS_ID_ADDRESS => {
                    let addr = dict_to_address(d)?;
                    Ok(Value::Object(Object::BMAddress(addr)))
                }
                _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "unsupported BM dict class_id: {class_id}"
                ))),
            };
        }
        if d.contains("slot_id")? || d.contains("app_id")? {
            let info = dict_to_registry_info(d)?;
            return Ok(Value::Object(Object::BMRegistryInfo(info)));
        }
        if d.contains("address")?
            && d.contains("unreliable_port")?
            && d.contains("reliable_port")?
        {
            let addr = dict_to_address(d)?;
            return Ok(Value::Object(Object::BMAddress(addr)));
        }
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "unsupported dict payload; add class_id or BM-compatible shape",
        ));
    }
    if let Ok(list) = obj.cast::<PyList>() {
        let mut arr = BMArray::default();
        for item in list.iter() {
            arr.push(py_to_value(&item)?);
        }
        return Ok(Value::Object(Object::BMArray(arr)));
    }
    if let Ok(s) = obj.cast::<PyString>() {
        return Ok(Value::String(s.to_string()));
    }
    if let Ok(b) = obj.cast::<PyBool>() {
        return Ok(Value::Bool(b.is_true()));
    }
    if let Ok(i) = obj.cast::<PyInt>() {
        let val: i64 = i.extract()?;
        if val >= i32::MIN as i64 && val <= i32::MAX as i64 {
            return Ok(Value::I32(val as i32));
        }
        return Ok(Value::F64(val as f64));
    }
    if let Ok(f) = obj.cast::<PyFloat>() {
        return Ok(Value::F64(f.extract()?));
    }
    Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
        "unsupported Python value for BM encoding",
    ))
}

fn opt_i16(item: Option<Bound<'_, PyAny>>) -> PyResult<Option<i16>> {
    if let Some(v) = item {
        if v.is_none() {
            return Ok(None);
        }
        return Ok(Some(v.extract()?));
    }
    Ok(None)
}

fn opt_i32(item: Option<Bound<'_, PyAny>>) -> PyResult<Option<i32>> {
    if let Some(v) = item {
        if v.is_none() {
            return Ok(None);
        }
        return Ok(Some(v.extract()?));
    }
    Ok(None)
}

fn opt_string(item: Option<Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    if let Some(v) = item {
        if v.is_none() {
            return Ok(None);
        }
        return Ok(Some(v.extract()?));
    }
    Ok(None)
}

fn dict_to_address(d: &Bound<'_, PyDict>) -> PyResult<BMAddress> {
    let addr_host = opt_string(d.get_item("address")?)?.unwrap_or_default();
    let addr_unreliable = opt_i32(d.get_item("unreliable_port")?)?.unwrap_or(0);
    let addr_reliable = opt_i32(d.get_item("reliable_port")?)?.unwrap_or(0);
    Ok(BMAddress {
        address: addr_host,
        unreliable_port: addr_unreliable,
        reliable_port: addr_reliable,
    })
}

fn dict_to_registry_info(d: &Bound<'_, PyDict>) -> PyResult<BMRegistryInfo> {
    let slot_id = opt_i16(d.get_item("slot_id")?)?.unwrap_or(0);
    let app_id = opt_string(d.get_item("app_id")?)?.unwrap_or_default();
    let current_players = opt_i16(d.get_item("current_players")?)?;
    let max_players = opt_i16(d.get_item("max_players")?)?;

    let dev_any = d
        .get_item("device")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing device"))?;
    if dev_any.is_none() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "device is None",
        ));
    }
    let dev_dict = dev_any.cast::<PyDict>()?.clone();
    let dev_id = opt_string(dev_dict.get_item("id")?)?.unwrap_or_default();
    let dev_name = opt_string(dev_dict.get_item("name")?)?.unwrap_or_default();
    let dev_type_code = opt_i32(dev_dict.get_item("device_type")?)?.unwrap_or(0);
    let dev_type = DeviceType::for_value(dev_type_code).unwrap_or(DeviceType::Server);

    let addr_any = d
        .get_item("device_address")?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("missing device_address"))?;
    if addr_any.is_none() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            "device_address is None",
        ));
    }
    let addr_dict = addr_any.cast::<PyDict>()?.clone();
    let address = dict_to_address(&addr_dict)?;

    let mut device = DeviceCore::new(dev_id, dev_name, dev_type);
    device.address = Some(address.clone());

    Ok(BMRegistryInfo {
        slot_id,
        app_id,
        current_players,
        max_players,
        device,
        device_address: address,
    })
}

fn now_ms_f64() -> f64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    dur.as_millis() as f64
}
