// SPDX-License-Identifier: MIT
// Copyright (C) 2026 ddavef/KinteLiX bronze-monkey

#![cfg(feature = "pyo3")]

use crate::codec::bm_stream::BMStream;
use crate::codec::externals::bm_array::BMArray;
use crate::codec::externals::bm_packet::BMPacket;
use crate::codec::externals::bm_registry_info::BMRegistryInfo;
use crate::codec::externals::bm_version::BMVersion;
use crate::codec::externals::handshake::handshake_bytes;
use crate::codec::externals::registry;
use crate::codec::messages::bm_encoding::Value;
use crate::codec::messages::bm_invoke::BMInvoke;
use crate::codec::object::Object;
use crate::config::EngineConfig;
use crate::controls::assembler::SchemeAssembler;
use crate::controls::parser::BMApplicationSchemeParser;
use crate::devices::bm_address::BMAddress;
use crate::devices::device_core::DeviceCore;
use crate::engine::device_registry::DeviceRecord;
use crate::engine::events::{Arrival, Command, Outgoing};
use crate::engine::processing::Engine;
use crate::engine::protocol::{
    deserialize_packet as protocol_deserialize_packet, serialize_packet,
};
use crate::policy::EndpointMode;
use crate::types::device_type::DeviceType;
use crate::types::packet_type::PacketType;
use prost::Message;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyModule, PyString};
use pythonize::{depythonize, pythonize};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[pyfunction]
fn configure_logging(level: u8, capacity: usize) -> bool {
    crate::logging::install(crate::logging::LogConfig {
        level: crate::logging::level_filter_from_u8(level),
        capacity,
    })
}

#[pyfunction]
fn set_log_level(level: u8) {
    crate::logging::set_level(crate::logging::level_filter_from_u8(level));
}

#[pyfunction]
fn take_logs<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    Ok(pythonize(py, &crate::logging::take_logs())?)
}

#[pyfunction]
fn generate_device_id() -> String {
    crate::identity::generate_device_id()
}

#[pyfunction]
fn generate_app_id() -> String {
    crate::identity::generate_app_id()
}

fn device_type_from_code(code: i32) -> PyResult<DeviceType> {
    DeviceType::for_value(code)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
}

fn packet_type_from_code(code: i32) -> PyResult<PacketType> {
    PacketType::from_i32(code).ok_or_else(|| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("unknown packet type: {code}"))
    })
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

    fn init_local_device(
        &self,
        device_id: String,
        device_name: String,
        device_type: i32,
    ) -> PyResult<()> {
        let dt = device_type_from_code(device_type)?;
        let mut eng = self.inner.write().unwrap();
        eng.init_local_device(DeviceCore::new(device_id, device_name, dt));
        Ok(())
    }

    fn init_local_device_with_address(
        &self,
        device_id: String,
        device_name: String,
        device_type: i32,
        address: String,
        unreliable_port: i32,
        reliable_port: i32,
    ) -> PyResult<()> {
        let dt = device_type_from_code(device_type)?;
        let mut eng = self.inner.write().unwrap();
        let mut core = DeviceCore::new(device_id, device_name, dt);
        core.address = Some(BMAddress {
            address,
            unreliable_port,
            reliable_port,
        });
        eng.init_local_device(core);
        Ok(())
    }

    /// Everything this engine is told about itself, all at once.
    fn configure(&self, config: Bound<'_, PyAny>) -> PyResult<()> {
        let config: EngineConfig = depythonize(&config)?;
        self.inner
            .write()
            .unwrap()
            .configure(config)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }

    fn peer_gone<'py>(&self, py: Python<'py>, device_id: String) -> PyResult<Bound<'py, PyList>> {
        let actions = self.inner.write().unwrap().peer_gone(&device_id);
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

    /// `arrival` is whatever the transport knows about where the bytes came
    /// from. A relayed transport has nothing to say and passes nothing.
    #[pyo3(signature = (data, arrival = None))]
    fn process_incoming<'py>(
        &self,
        py: Python<'py>,
        data: &[u8],
        arrival: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let arrival: Arrival = match arrival {
            Some(value) => depythonize(&value)?,
            None => Arrival::default(),
        };
        let out = self.inner.write().unwrap().process_incoming(data, &arrival);
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

    fn declare_peer(
        &self,
        device_id: String,
        device_name: String,
        device_type: i32,
        address: String,
        unreliable_port: i32,
        reliable_port: i32,
    ) -> PyResult<()> {
        let dt = device_type_from_code(device_type)?;
        let mut eng = self.inner.write().unwrap();
        let mut core = DeviceCore::new(device_id, device_name, dt);
        core.address = Some(BMAddress {
            address,
            unreliable_port,
            reliable_port,
        });
        eng.registry_mut().upsert(DeviceRecord::new(core, None));
        Ok(())
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
        packet_type: packet_type_from_code(packet_type_code)?,
        device_type: device_type_from_code(device_type_code)?,
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
    let device_type = device_type_from_code(device_type_code)?;
    let class_id = registry::class_id_for_device_type(device_type);

    let core = DeviceCore::new(device_id.clone(), device_name.clone(), device_type);
    let mut out = BMStream::new();
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
        packet_type: packet_type_from_code(packet_type_code)?,
        device_type,
        device_name,
        device_id,
        message: Some(out.into_inner()),
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
    d.set_item("device_id", pkt.device_id.clone())?;
    d.set_item("device_name", pkt.device_name.clone())?;
    if let Some(msg) = pkt.message.as_ref() {
        d.set_item("message_bytes", PyBytes::new(py, msg))?;
        let mut cur = BMStream::view(msg.as_slice());
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
    for kind in DeviceType::ALL {
        d.set_item(kind.label(), kind.code())?;
    }
    Ok(d)
}

#[pyfunction]
fn get_packet_type_codes<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    for kind in PacketType::ALL {
        d.set_item(kind.label(), kind.code())?;
    }
    Ok(d)
}

#[pyfunction]
fn version_info<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    Ok(pythonize(py, &crate::version::version_info())?)
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

#[pyclass]
pub struct SchemeAssemblerPy {
    inner: RwLock<SchemeAssembler>,
}

#[pymethods]
impl SchemeAssemblerPy {
    #[new]
    fn new() -> Self {
        Self {
            inner: RwLock::new(SchemeAssembler::new()),
        }
    }

    fn offer<'py>(
        &self,
        py: Python<'py>,
        set_id: &str,
        blob: &[u8],
    ) -> PyResult<Bound<'py, PyAny>> {
        let result = self.inner.write().unwrap().offer(set_id, blob);
        Ok(pythonize(py, &result)?)
    }

    fn current<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.inner
            .read()
            .unwrap()
            .current()
            .map(|bytes| PyBytes::new(py, &bytes))
    }

    fn reset(&self) {
        self.inner.write().unwrap().reset();
    }
}

/// Reads a frame into its described form. Engine free: it allocates no sequence
/// numbers and needs no registered device, so it can run alongside a live
/// session without disturbing it.
#[pyfunction]
fn inspect_wire<'py>(py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
    let view = crate::inspect::inspect(data)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    Ok(pythonize(py, &view)?)
}

/// Serializes a described frame into wire bytes.
#[pyfunction]
fn build_wire<'py>(py: Python<'py>, view: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    let view: crate::inspect::WireView = depythonize(&view)?;
    let bytes = crate::inspect::build(view)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    Ok(PyBytes::new(py, &bytes))
}

/// Reassembles messages from a stream that arrives in arbitrary pieces.
#[pyclass]
pub struct FramerPy {
    inner: RwLock<crate::link::framing::Framer>,
}

#[pymethods]
impl FramerPy {
    /// Rejects messages longer than max_len, or the library ceiling when it is
    /// left out. A limit above that ceiling is clamped to it.
    #[new]
    #[pyo3(signature = (max_len=None))]
    fn new(max_len: Option<usize>) -> Self {
        let max_len = max_len.unwrap_or(crate::link::framing::MAX_MESSAGE_LEN);
        Self {
            inner: RwLock::new(crate::link::framing::Framer::with_max_len(max_len)),
        }
    }

    /// The limit this framer was created with.
    #[getter]
    fn max_len(&self) -> usize {
        self.inner.read().unwrap().max_len()
    }

    /// Adds bytes and returns every message they completed.
    fn feed<'py>(&self, py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyList>> {
        let messages = self
            .inner
            .write()
            .unwrap()
            .feed(data)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        let items: Vec<_> = messages.iter().map(|m| PyBytes::new(py, m)).collect();
        PyList::new(py, items)
    }

    /// Drops anything half read, for when a connection restarts.
    fn reset(&self) {
        self.inner.write().unwrap().reset();
    }

    #[getter]
    fn pending(&self) -> usize {
        self.inner.read().unwrap().pending()
    }
}

/// Writes a message with the length prefix a stream transport needs.
#[pyfunction]
fn frame<'py>(py: Python<'py>, message: &[u8]) -> Bound<'py, PyBytes> {
    PyBytes::new(py, &crate::link::framing::frame(message))
}

/// Whether these bytes open a cross domain policy request.
#[pyfunction]
fn is_policy_request(data: &[u8]) -> bool {
    crate::link::crossdomain::is_policy_request(data)
}

/// The policy response to send back, NUL terminator included.
#[pyfunction]
fn policy_response<'py>(py: Python<'py>) -> Bound<'py, PyBytes> {
    PyBytes::new(py, crate::link::crossdomain::RESPONSE)
}

/// Watches the head of one connection for a policy request, for transports
/// that hand over bytes rather than let them be peeked.
#[pyclass]
pub struct PolicySnifferPy {
    inner: RwLock<crate::link::crossdomain::Sniffer>,
}

#[pymethods]
impl PolicySnifferPy {
    #[new]
    fn new() -> Self {
        Self {
            inner: RwLock::new(crate::link::crossdomain::Sniffer::new()),
        }
    }

    /// Offers the next bytes off the wire.
    fn feed<'py>(&self, py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
        let sniff = self.inner.write().unwrap().feed(data);
        Ok(pythonize(py, &sniff)?)
    }

    /// Whether the answer is still open. Once it is not, bytes can go straight
    /// on and the sniffer can be skipped for the rest of the connection.
    #[getter]
    fn is_watching(&self) -> bool {
        self.inner.read().unwrap().is_watching()
    }

    /// Whether the connection that just dropped was one we hung up on after
    /// answering. Watches again either way.
    fn hung_up(&self) -> bool {
        self.inner.write().unwrap().hung_up()
    }

    /// Starts over, for a sniffer reused across connections.
    fn reset(&self) {
        self.inner.write().unwrap().reset();
    }
}

#[pymodule]
#[pyo3(name = "bronze_monkey")]
fn bronze_monkey_py(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    crate::log_library_loaded();
    m.add_class::<BMEnginePy>()?;
    m.add_class::<SchemeAssemblerPy>()?;
    m.add_class::<FramerPy>()?;
    m.add_class::<HandshakerPy>()?;
    m.add_class::<PolicySnifferPy>()?;
    m.add_class::<EndpointMode>()?;
    m.add_function(wrap_pyfunction!(handshake, m)?)?;
    m.add_function(wrap_pyfunction!(serialize_invoke_packet, m)?)?;
    m.add_function(wrap_pyfunction!(serialize_device_packet, m)?)?;
    m.add_function(wrap_pyfunction!(deserialize_packet_dict, m)?)?;
    m.add_function(wrap_pyfunction!(inspect_wire, m)?)?;
    m.add_function(wrap_pyfunction!(build_wire, m)?)?;
    m.add_function(wrap_pyfunction!(frame, m)?)?;
    m.add_function(wrap_pyfunction!(is_policy_request, m)?)?;
    m.add_function(wrap_pyfunction!(policy_response, m)?)?;
    m.add_function(wrap_pyfunction!(configure_logging, m)?)?;
    m.add_function(wrap_pyfunction!(set_log_level, m)?)?;
    m.add_function(wrap_pyfunction!(take_logs, m)?)?;
    m.add_function(wrap_pyfunction!(generate_device_id, m)?)?;
    m.add_function(wrap_pyfunction!(generate_app_id, m)?)?;
    m.add_function(wrap_pyfunction!(get_device_type_codes, m)?)?;
    m.add_function(wrap_pyfunction!(get_packet_type_codes, m)?)?;
    m.add_function(wrap_pyfunction!(parse_control_scheme_xml, m)?)?;
    m.add_function(wrap_pyfunction!(version_info, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("MAX_MESSAGE_LEN", crate::link::framing::MAX_MESSAGE_LEN)?;
    m.add("DEVICE_TYPE_ANY", DeviceType::Any.code())?;
    m.add("DEVICE_TYPE_UNITY", DeviceType::Unity.code())?;
    m.add("DEVICE_TYPE_IPHONE", DeviceType::IPhone.code())?;
    m.add("DEVICE_TYPE_FLASH", DeviceType::Flash.code())?;
    m.add("DEVICE_TYPE_ANDROID", DeviceType::Android.code())?;
    m.add("DEVICE_TYPE_NATIVE", DeviceType::Native.code())?;
    m.add("DEVICE_TYPE_PALM", DeviceType::Palm.code())?;
    m.add("DEVICE_TYPE_SERVER", DeviceType::Server.code())?;
    m.add("ENDPOINT_MODE_NONE", EndpointMode::NONE_CODE)?;
    m.add("ENDPOINT_MODE_GAME", EndpointMode::Game.code())?;
    m.add("ENDPOINT_MODE_CONTROLLER", EndpointMode::Controller.code())?;
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
        Object::BMParameter(p) => value_to_py(py, &p.value),
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
    let dev_type = device_type_from_code(dev_type_code)?;

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

/// Tracks the version exchange for one connection.
#[pyclass]
pub struct HandshakerPy {
    inner: RwLock<crate::link::negotiation::Handshaker>,
}

#[pymethods]
impl HandshakerPy {
    /// role is 0 to speak first, 1 to wait and answer. The versions default to
    /// the library's own; pass a pair to stand in as a different build.
    #[new]
    #[pyo3(signature = (role, current=None, minimum=None))]
    fn new(
        role: i32,
        current: Option<(u8, u8, u16)>,
        minimum: Option<(u8, u8, u16)>,
    ) -> PyResult<Self> {
        let role = crate::link::negotiation::LinkRole::from_code(role)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("unknown link role"))?;
        let inner = match (current, minimum) {
            (Some(c), Some(m)) => crate::link::negotiation::Handshaker::with_version(
                role,
                crate::codec::externals::handshake::Handshake::new(
                    BMVersion::new(c.0, c.1, c.2),
                    BMVersion::new(m.0, m.1, m.2),
                ),
            ),
            _ => crate::link::negotiation::Handshaker::new(role),
        };
        Ok(Self {
            inner: RwLock::new(inner),
        })
    }

    /// What to send now the connection is up, empty when there is nothing.
    fn on_connect<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let bytes = self.inner.write().unwrap().on_connect().unwrap_or_default();
        PyBytes::new(py, &bytes)
    }

    fn on_message<'py>(&self, py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
        let outcome = self.inner.write().unwrap().on_message(data);
        Ok(pythonize(py, &outcome)?)
    }

    #[getter]
    fn is_complete(&self) -> bool {
        self.inner.read().unwrap().is_complete()
    }

    fn reset(&self) {
        self.inner.write().unwrap().reset();
    }
}
