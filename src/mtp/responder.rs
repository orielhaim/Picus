use crate::babel::{self, Babel};
use crate::mtp::{
    CONTAINER_COMMAND, CONTAINER_DATA, CONTAINER_RESPONSE, ROOT_PARENT, STORAGE_ID, Transport,
};

pub const MAX_SLOTS: usize = 2048;
pub const MAX_DEPTH: usize = babel::BUF;

const TX: usize = 512;
const RX: usize = 64;

const SERIAL: &[u8] = b"BABELUSB0001";
const DATETIME: &[u8] = b"20250808T173500.0";
const FRIENDLY: &[u8] = b"USB of Babel";
const MANUFACTURER: &[u8] = b"p2r3";
const MODEL: &[u8] = b"USB of Babel";
const VERSION: &[u8] = b"1.0";
const EXTENSIONS: &[u8] = b"microsoft.com: 1.0; ";

const OP_GET_DEVICE_INFO: u16 = 0x1001;
const OP_OPEN_SESSION: u16 = 0x1002;
const OP_CLOSE_SESSION: u16 = 0x1003;
const OP_GET_STORAGE_IDS: u16 = 0x1004;
const OP_GET_STORAGE_INFO: u16 = 0x1005;
const OP_GET_NUM_OBJECTS: u16 = 0x1006;
const OP_GET_OBJECT_HANDLES: u16 = 0x1007;
const OP_GET_OBJECT_INFO: u16 = 0x1008;
const OP_GET_OBJECT: u16 = 0x1009;
const OP_DELETE_OBJECT: u16 = 0x100B;
const OP_SEND_OBJECT_INFO: u16 = 0x100C;
const OP_SEND_OBJECT: u16 = 0x100D;
const OP_FORMAT_STORE: u16 = 0x100F;
const OP_RESET_DEVICE: u16 = 0x1010;
const OP_GET_DEVICE_PROP_DESC: u16 = 0x1014;
const OP_GET_DEVICE_PROP_VALUE: u16 = 0x1015;
const OP_SET_DEVICE_PROP_VALUE: u16 = 0x1016;

const RESP_OK: u16 = 0x2001;
const RESP_SESSION_NOT_OPEN: u16 = 0x2003;
const RESP_OPERATION_NOT_SUPPORTED: u16 = 0x2005;
const RESP_PARAMETER_NOT_SUPPORTED: u16 = 0x2006;
const RESP_INVALID_STORAGE_ID: u16 = 0x2008;
const RESP_INVALID_OBJECT_HANDLE: u16 = 0x2009;
const RESP_STORE_FULL: u16 = 0x200C;
const RESP_DEVICE_BUSY: u16 = 0x2019;
const RESP_SESSION_ALREADY_OPEN: u16 = 0x201E;
const RESP_INVALID_DATASET: u16 = 0xA806;

const OBJ_FORMAT_UNDEFINED: u16 = 0x3000;
const OBJ_FORMAT_ASSOCIATION: u16 = 0x3001;
const OBJ_FORMAT_TEXT: u16 = 0x3004;
const OBJ_FORMAT_PNG: u16 = 0x380B;

const DEV_PROP_FRIENDLY_NAME: u16 = 0xD402;

const SUPPORTED_OPS: [u16; 12] = [
    OP_GET_DEVICE_INFO,
    OP_OPEN_SESSION,
    OP_CLOSE_SESSION,
    OP_GET_STORAGE_IDS,
    OP_GET_STORAGE_INFO,
    OP_GET_NUM_OBJECTS,
    OP_GET_OBJECT_HANDLES,
    OP_GET_OBJECT_INFO,
    OP_GET_OBJECT,
    OP_RESET_DEVICE,
    OP_GET_DEVICE_PROP_DESC,
    OP_GET_DEVICE_PROP_VALUE,
];
const SUPPORTED_EVENTS: [u16; 1] = [0x4002];
const SUPPORTED_DEV_PROPS: [u16; 1] = [DEV_PROP_FRIENDLY_NAME];
const SUPPORTED_FORMATS: [u16; 4] = [
    OBJ_FORMAT_UNDEFINED,
    OBJ_FORMAT_ASSOCIATION,
    OBJ_FORMAT_TEXT,
    OBJ_FORMAT_PNG,
];

#[derive(Clone, Copy)]
struct Slot {
    parent: u16,
    local: u16,
}

fn resolve_path(
    slots: &[Slot],
    slot_count: usize,
    slot: u16,
    out: &mut [u16],
) -> Result<usize, ()> {
    let mut depth = 0usize;
    let mut s = slot as usize;
    while s != 0 {
        if s >= slot_count || depth >= out.len() {
            return Err(());
        }
        out[depth] = slots[s].local;
        depth += 1;
        s = slots[s].parent as usize;
    }
    out[..depth].reverse();
    Ok(depth)
}

fn decode_handle(handle: u32) -> Option<(u32, u32)> {
    let slot = handle >> 13;
    let local = handle & 0x1FFF;
    if local == 0 || local > babel::FILE_LOCAL || slot >= MAX_SLOTS as u32 {
        return None;
    }
    Some((slot, local))
}

fn push_u16(buf: &mut [u8], w: &mut usize, v: u16) {
    buf[*w..*w + 2].copy_from_slice(&v.to_le_bytes());
    *w += 2;
}

fn push_u32(buf: &mut [u8], w: &mut usize, v: u32) {
    buf[*w..*w + 4].copy_from_slice(&v.to_le_bytes());
    *w += 4;
}

fn push_u64(buf: &mut [u8], w: &mut usize, v: u64) {
    buf[*w..*w + 8].copy_from_slice(&v.to_le_bytes());
    *w += 8;
}

fn push_auint16(buf: &mut [u8], w: &mut usize, arr: &[u16]) {
    push_u32(buf, w, arr.len() as u32);
    for &v in arr {
        push_u16(buf, w, v);
    }
}

fn push_cstring(buf: &mut [u8], w: &mut usize, s: &[u8]) {
    if s.is_empty() {
        buf[*w] = 0;
        *w += 1;
        return;
    }
    buf[*w] = (s.len() + 1) as u8;
    *w += 1;
    for &c in s {
        buf[*w] = c;
        buf[*w + 1] = 0;
        *w += 2;
    }
    buf[*w] = 0;
    buf[*w + 1] = 0;
    *w += 2;
}

fn push_ustring(buf: &mut [u8], w: &mut usize, s: &[u8]) {
    buf[*w] = s.len() as u8;
    *w += 1;
    for &c in s {
        buf[*w] = c;
        buf[*w + 1] = 0;
        *w += 2;
    }
}

async fn write_bytes<T: Transport>(t: &mut T, mps: usize, data: &[u8]) -> Result<(), T::Error> {
    let mut i = 0usize;
    while i < data.len() {
        let end = (i + mps).min(data.len());
        t.write_packet(&data[i..end]).await?;
        i = end;
    }
    Ok(())
}

async fn send_data<T: Transport>(
    t: &mut T,
    mps: usize,
    code: u16,
    txn: u32,
    payload: &[u8],
) -> Result<(), T::Error> {
    let total = (12 + payload.len()) as u32;
    let mut buf = [0u8; 64];
    buf[0..4].copy_from_slice(&total.to_le_bytes());
    buf[4..6].copy_from_slice(&CONTAINER_DATA.to_le_bytes());
    buf[6..8].copy_from_slice(&code.to_le_bytes());
    buf[8..12].copy_from_slice(&txn.to_le_bytes());
    let room = mps.saturating_sub(12).min(buf.len().saturating_sub(12));
    let first = room.min(payload.len());
    buf[12..12 + first].copy_from_slice(&payload[..first]);
    t.write_packet(&buf[..12 + first]).await?;
    write_bytes(t, mps, &payload[first..]).await
}

async fn send_response<T: Transport>(
    t: &mut T,
    mps: usize,
    _code: u16,
    txn: u32,
    resp: u16,
    params: &[u32],
) -> Result<(), T::Error> {
    let total = (12 + params.len() * 4) as u32;
    let mut buf = [0u8; 12 + 20];
    buf[0..4].copy_from_slice(&total.to_le_bytes());
    buf[4..6].copy_from_slice(&CONTAINER_RESPONSE.to_le_bytes());
    buf[6..8].copy_from_slice(&resp.to_le_bytes());
    buf[8..12].copy_from_slice(&txn.to_le_bytes());
    for (i, p) in params.iter().enumerate() {
        buf[12 + 4 * i..16 + 4 * i].copy_from_slice(&p.to_le_bytes());
    }
    write_bytes(t, mps, &buf[..12 + params.len() * 4]).await
}

async fn send_handles<T: Transport>(
    t: &mut T,
    mps: usize,
    code: u16,
    txn: u32,
    dir_slot: u16,
) -> Result<(), T::Error> {
    let count = babel::OBJECTS_PER_DIR;
    let total = 12u32 + 4 + count * 4;
    let mut buf = [0u8; 64];
    buf[0..4].copy_from_slice(&total.to_le_bytes());
    buf[4..6].copy_from_slice(&CONTAINER_DATA.to_le_bytes());
    buf[6..8].copy_from_slice(&code.to_le_bytes());
    buf[8..12].copy_from_slice(&txn.to_le_bytes());
    buf[12..16].copy_from_slice(&count.to_le_bytes());
    let first_room = mps.min(buf.len()).saturating_sub(16) / 4;
    let first = (count as usize).min(first_room);
    for k in 0..first {
        let h = ((dir_slot as u32) << 13) | (k as u32 + 1);
        buf[16 + 4 * k..20 + 4 * k].copy_from_slice(&h.to_le_bytes());
    }
    t.write_packet(&buf[..16 + 4 * first]).await?;
    let mut j = first as u32;
    let per = (mps / 4).max(1);
    while j < count {
        let n = ((count - j) as usize).min(per);
        for k in 0..n {
            let h = ((dir_slot as u32) << 13) | (j + k as u32 + 1);
            buf[4 * k..4 * k + 4].copy_from_slice(&h.to_le_bytes());
        }
        t.write_packet(&buf[..4 * n]).await?;
        j += n as u32;
    }
    Ok(())
}

pub struct Responder {
    babel: Babel,
    slots: [Slot; MAX_SLOTS],
    slot_count: usize,
    session_open: bool,
    session_id: u32,
    path: [u16; MAX_DEPTH],
    tx: [u8; TX],
    rx: [u8; RX],
}

impl Default for Responder {
    fn default() -> Self {
        Self::new()
    }
}

impl Responder {
    pub const fn new() -> Self {
        Self {
            babel: Babel::new(),
            slots: [Slot {
                parent: 0,
                local: 0,
            }; MAX_SLOTS],
            slot_count: 1,
            session_open: false,
            session_id: 0,
            path: [0; MAX_DEPTH],
            tx: [0; TX],
            rx: [0; RX],
        }
    }

    pub fn reset(&mut self) {
        self.session_open = false;
        self.session_id = 0;
        self.reset_slots();
    }

    fn reset_slots(&mut self) {
        self.slot_count = 1;
        self.slots[0] = Slot {
            parent: u16::MAX,
            local: 0,
        };
    }

    fn find_or_alloc(&mut self, parent: u16, local: u16) -> Option<u16> {
        let mut i = 1usize;
        while i < self.slot_count {
            if self.slots[i].parent == parent && self.slots[i].local == local {
                return Some(i as u16);
            }
            i += 1;
        }
        if self.slot_count >= MAX_SLOTS {
            return None;
        }
        let i = self.slot_count;
        self.slots[i] = Slot { parent, local };
        self.slot_count += 1;
        Some(i as u16)
    }

    pub async fn handle<T: Transport>(&mut self, t: &mut T, mps: usize) -> Result<(), T::Error> {
        let n = t.read_packet(&mut self.rx).await?;
        if n < 12 {
            return Ok(());
        }
        let total = u32::from_le_bytes([self.rx[0], self.rx[1], self.rx[2], self.rx[3]]) as usize;
        let ctype = u16::from_le_bytes([self.rx[4], self.rx[5]]);
        let code = u16::from_le_bytes([self.rx[6], self.rx[7]]);
        let txn = u32::from_le_bytes([self.rx[8], self.rx[9], self.rx[10], self.rx[11]]);
        if ctype != CONTAINER_COMMAND {
            return Ok(());
        }
        if total < 12 || total > self.rx.len() {
            return send_response(t, mps, code, txn, RESP_INVALID_DATASET, &[]).await;
        }
        let mut got = n;
        while got < total {
            let m = t.read_packet(&mut self.rx[got..]).await?;
            if m == 0 {
                break;
            }
            got += m;
        }
        let mut params = [0u32; 5];
        let nparams = (total - 12) / 4;
        let mut i = 0usize;
        while i < nparams && i < 5 {
            let o = 12 + 4 * i;
            params[i] =
                u32::from_le_bytes([self.rx[o], self.rx[o + 1], self.rx[o + 2], self.rx[o + 3]]);
            i += 1;
        }
        self.dispatch(t, mps, code, txn, &params).await
    }

    async fn dispatch<T: Transport>(
        &mut self,
        t: &mut T,
        mps: usize,
        code: u16,
        txn: u32,
        params: &[u32; 5],
    ) -> Result<(), T::Error> {
        match code {
            OP_GET_DEVICE_INFO => {
                let len = self.build_device_info();
                send_data(t, mps, code, txn, &self.tx[..len]).await?;
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_OPEN_SESSION => {
                if self.session_open {
                    return send_response(t, mps, code, txn, RESP_SESSION_ALREADY_OPEN, &[]).await;
                }
                self.session_open = true;
                self.session_id = params[0];
                self.reset_slots();
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_CLOSE_SESSION => {
                if !self.session_open {
                    return send_response(t, mps, code, txn, RESP_SESSION_NOT_OPEN, &[]).await;
                }
                self.session_open = false;
                self.reset_slots();
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_RESET_DEVICE => {
                self.reset();
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_GET_STORAGE_IDS => {
                let mut payload = [0u8; 8];
                payload[0..4].copy_from_slice(&1u32.to_le_bytes());
                payload[4..8].copy_from_slice(&STORAGE_ID.to_le_bytes());
                send_data(t, mps, code, txn, &payload).await?;
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_GET_STORAGE_INFO => {
                if params[0] != STORAGE_ID {
                    return send_response(t, mps, code, txn, RESP_INVALID_STORAGE_ID, &[]).await;
                }
                let len = self.build_storage_info();
                send_data(t, mps, code, txn, &self.tx[..len]).await?;
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_GET_NUM_OBJECTS => {
                if params[0] != STORAGE_ID && params[0] != ROOT_PARENT {
                    return send_response(t, mps, code, txn, RESP_INVALID_STORAGE_ID, &[]).await;
                }
                if params[2] != ROOT_PARENT && decode_handle(params[2]).is_none() {
                    return send_response(t, mps, code, txn, RESP_INVALID_OBJECT_HANDLE, &[]).await;
                }
                send_response(t, mps, code, txn, RESP_OK, &[babel::OBJECTS_PER_DIR]).await
            }
            OP_GET_OBJECT_HANDLES => {
                let storage = params[0];
                let parent = params[2];
                if storage != STORAGE_ID && storage != ROOT_PARENT {
                    return send_response(t, mps, code, txn, RESP_INVALID_STORAGE_ID, &[]).await;
                }
                let dir_slot = if parent == ROOT_PARENT {
                    0u16
                } else {
                    let Some((s, l)) = decode_handle(parent) else {
                        return send_response(t, mps, code, txn, RESP_INVALID_OBJECT_HANDLE, &[])
                            .await;
                    };
                    if l == babel::FILE_LOCAL || s >= self.slot_count as u32 {
                        return send_response(t, mps, code, txn, RESP_INVALID_OBJECT_HANDLE, &[])
                            .await;
                    }
                    match self.find_or_alloc(s as u16, l as u16) {
                        Some(d) => d,
                        None => {
                            return send_response(t, mps, code, txn, RESP_DEVICE_BUSY, &[]).await;
                        }
                    }
                };
                send_handles(t, mps, code, txn, dir_slot).await?;
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_GET_OBJECT_INFO => {
                let Some((s, l)) = decode_handle(params[0]) else {
                    return send_response(t, mps, code, txn, RESP_INVALID_OBJECT_HANDLE, &[]).await;
                };
                if s >= self.slot_count as u32 {
                    return send_response(t, mps, code, txn, RESP_INVALID_OBJECT_HANDLE, &[]).await;
                }
                if l == babel::FILE_LOCAL {
                    let depth = match resolve_path(
                        &self.slots,
                        self.slot_count,
                        s as u16,
                        &mut self.path,
                    ) {
                        Ok(d) => d,
                        Err(_) => {
                            return send_response(
                                t,
                                mps,
                                code,
                                txn,
                                RESP_INVALID_OBJECT_HANDLE,
                                &[],
                            )
                            .await;
                        }
                    };
                    let size = match self.babel.file_size(&self.path[..depth]) {
                        Ok(v) => v,
                        Err(_) => {
                            return send_response(t, mps, code, txn, RESP_STORE_FULL, &[]).await;
                        }
                    };
                    let len = self.build_object_info(babel::FILE_NAME, false, size);
                    send_data(t, mps, code, txn, &self.tx[..len]).await?;
                } else {
                    let name = babel::dir_name(l);
                    let len = self.build_object_info(&name, true, 0);
                    send_data(t, mps, code, txn, &self.tx[..len]).await?;
                }
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_GET_OBJECT => {
                let Some((s, l)) = decode_handle(params[0]) else {
                    return send_response(t, mps, code, txn, RESP_INVALID_OBJECT_HANDLE, &[]).await;
                };
                if l != babel::FILE_LOCAL || s >= self.slot_count as u32 {
                    return send_response(t, mps, code, txn, RESP_INVALID_OBJECT_HANDLE, &[]).await;
                }
                let depth =
                    match resolve_path(&self.slots, self.slot_count, s as u16, &mut self.path) {
                        Ok(d) => d,
                        Err(_) => {
                            return send_response(
                                t,
                                mps,
                                code,
                                txn,
                                RESP_INVALID_OBJECT_HANDLE,
                                &[],
                            )
                            .await;
                        }
                    };
                let content = match self.babel.file_content(&self.path[..depth]) {
                    Ok(c) => c,
                    Err(_) => {
                        return send_response(t, mps, code, txn, RESP_STORE_FULL, &[]).await;
                    }
                };
                send_data(t, mps, code, txn, content).await?;
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_GET_DEVICE_PROP_DESC => {
                if params[0] as u16 != DEV_PROP_FRIENDLY_NAME {
                    return send_response(t, mps, code, txn, RESP_PARAMETER_NOT_SUPPORTED, &[])
                        .await;
                }
                let len = self.build_device_prop_desc();
                send_data(t, mps, code, txn, &self.tx[..len]).await?;
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_GET_DEVICE_PROP_VALUE => {
                if params[0] as u16 != DEV_PROP_FRIENDLY_NAME {
                    return send_response(t, mps, code, txn, RESP_PARAMETER_NOT_SUPPORTED, &[])
                        .await;
                }
                let len = self.build_friendly_name();
                send_data(t, mps, code, txn, &self.tx[..len]).await?;
                send_response(t, mps, code, txn, RESP_OK, &[]).await
            }
            OP_DELETE_OBJECT
            | OP_SEND_OBJECT_INFO
            | OP_SEND_OBJECT
            | OP_FORMAT_STORE
            | OP_SET_DEVICE_PROP_VALUE => {
                send_response(t, mps, code, txn, RESP_OPERATION_NOT_SUPPORTED, &[]).await
            }
            _ => send_response(t, mps, code, txn, RESP_OPERATION_NOT_SUPPORTED, &[]).await,
        }
    }

    fn build_device_info(&mut self) -> usize {
        let mut w = 0usize;
        push_u16(&mut self.tx, &mut w, 100);
        push_u32(&mut self.tx, &mut w, 6);
        push_u16(&mut self.tx, &mut w, 100);
        push_cstring(&mut self.tx, &mut w, EXTENSIONS);
        push_u16(&mut self.tx, &mut w, 0);
        push_auint16(&mut self.tx, &mut w, &SUPPORTED_OPS);
        push_auint16(&mut self.tx, &mut w, &SUPPORTED_EVENTS);
        push_auint16(&mut self.tx, &mut w, &SUPPORTED_DEV_PROPS);
        push_auint16(&mut self.tx, &mut w, &SUPPORTED_FORMATS);
        push_auint16(&mut self.tx, &mut w, &SUPPORTED_FORMATS);
        push_cstring(&mut self.tx, &mut w, MANUFACTURER);
        push_cstring(&mut self.tx, &mut w, MODEL);
        push_cstring(&mut self.tx, &mut w, VERSION);
        push_ustring(&mut self.tx, &mut w, SERIAL);
        w
    }

    fn build_storage_info(&mut self) -> usize {
        let mut w = 0usize;
        push_u16(&mut self.tx, &mut w, 1);
        push_u16(&mut self.tx, &mut w, 2);
        push_u16(&mut self.tx, &mut w, 1);
        push_u64(&mut self.tx, &mut w, u64::MAX / 2);
        push_u64(&mut self.tx, &mut w, u64::MAX / 2);
        push_u32(&mut self.tx, &mut w, u32::MAX);
        push_cstring(&mut self.tx, &mut w, b"disk");
        push_cstring(&mut self.tx, &mut w, b"vol");
        w
    }

    fn build_object_info(&mut self, name: &[u8], is_dir: bool, size: usize) -> usize {
        let mut w = 0usize;
        push_u32(&mut self.tx, &mut w, STORAGE_ID);
        push_u16(
            &mut self.tx,
            &mut w,
            if is_dir {
                OBJ_FORMAT_ASSOCIATION
            } else {
                OBJ_FORMAT_TEXT
            },
        );
        push_u16(&mut self.tx, &mut w, 1);
        push_u32(&mut self.tx, &mut w, size as u32);
        push_u16(&mut self.tx, &mut w, OBJ_FORMAT_UNDEFINED);
        push_u32(&mut self.tx, &mut w, 0);
        push_u32(&mut self.tx, &mut w, 0);
        push_u32(&mut self.tx, &mut w, 0);
        push_u32(&mut self.tx, &mut w, 0);
        push_u32(&mut self.tx, &mut w, 0);
        push_u32(&mut self.tx, &mut w, 0);
        push_u32(&mut self.tx, &mut w, 0);
        push_u16(&mut self.tx, &mut w, if is_dir { 1 } else { 0 });
        push_u32(&mut self.tx, &mut w, 0);
        push_u32(&mut self.tx, &mut w, 0);
        push_ustring(&mut self.tx, &mut w, name);
        push_cstring(&mut self.tx, &mut w, DATETIME);
        push_cstring(&mut self.tx, &mut w, DATETIME);
        push_cstring(&mut self.tx, &mut w, b"");
        w
    }

    fn build_device_prop_desc(&mut self) -> usize {
        let mut w = 0usize;
        push_u16(&mut self.tx, &mut w, DEV_PROP_FRIENDLY_NAME);
        push_u16(&mut self.tx, &mut w, 0xFFFF);
        self.tx[w] = 0;
        w += 1;
        push_cstring(&mut self.tx, &mut w, FRIENDLY);
        push_cstring(&mut self.tx, &mut w, FRIENDLY);
        self.tx[w] = 0;
        w += 1;
        w
    }

    fn build_friendly_name(&mut self) -> usize {
        let mut w = 0usize;
        push_cstring(&mut self.tx, &mut w, FRIENDLY);
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    struct Mock {
        input: Vec<Vec<u8>>,
        pos: usize,
        output: Vec<u8>,
    }

    impl Mock {
        fn new(input: Vec<Vec<u8>>) -> Self {
            Self {
                input,
                pos: 0,
                output: Vec::new(),
            }
        }
    }

    impl Transport for Mock {
        type Error = ();
        async fn read_packet(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
            if self.pos >= self.input.len() {
                return Err(());
            }
            let pkt = &self.input[self.pos];
            self.pos += 1;
            buf[..pkt.len()].copy_from_slice(pkt);
            Ok(pkt.len())
        }
        async fn write_packet(&mut self, buf: &[u8]) -> Result<(), ()> {
            self.output.extend_from_slice(buf);
            Ok(())
        }
    }

    fn command(code: u16, txn: u32, params: &[u32]) -> Vec<u8> {
        let mut v = Vec::new();
        let total = (12 + params.len() * 4) as u32;
        v.extend_from_slice(&total.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&code.to_le_bytes());
        v.extend_from_slice(&txn.to_le_bytes());
        for p in params {
            v.extend_from_slice(&p.to_le_bytes());
        }
        v
    }

    fn containers(bytes: &[u8]) -> Vec<(u16, u16, u32, Vec<u8>)> {
        let mut out = Vec::new();
        let mut i = 0usize;
        while i + 12 <= bytes.len() {
            let len =
                u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
            let ctype = u16::from_le_bytes([bytes[i + 4], bytes[i + 5]]);
            let code = u16::from_le_bytes([bytes[i + 6], bytes[i + 7]]);
            let txn =
                u32::from_le_bytes([bytes[i + 8], bytes[i + 9], bytes[i + 10], bytes[i + 11]]);
            if len < 12 || i + len > bytes.len() {
                break;
            }
            out.push((ctype, code, txn, bytes[i + 12..i + len].to_vec()));
            i += len;
        }
        out
    }

    fn run(input: Vec<Vec<u8>>) -> Vec<(u16, u16, u32, Vec<u8>)> {
        run_mps(input, 64)
    }

    fn run_mps(input: Vec<Vec<u8>>, mps: usize) -> Vec<(u16, u16, u32, Vec<u8>)> {
        let mut r = Responder::new();
        r.reset();
        let mut mock = Mock::new(input);
        embassy_futures::block_on(async {
            while mock.pos < mock.input.len() {
                r.handle(&mut mock, mps).await.unwrap();
            }
        });
        containers(&mock.output)
    }

    #[test]
    fn device_info_and_session() {
        let out = run(vec![
            command(OP_GET_DEVICE_INFO, 1, &[]),
            command(OP_OPEN_SESSION, 2, &[1]),
            command(OP_GET_STORAGE_IDS, 3, &[]),
            command(OP_GET_STORAGE_INFO, 4, &[STORAGE_ID]),
        ]);
        assert_eq!(out.len(), 7);
        assert_eq!(out[0].0, CONTAINER_DATA);
        assert_eq!(out[1].0, CONTAINER_RESPONSE);
        assert_eq!(out[1].1, RESP_OK);
        assert_eq!(out[1].2, 1);
        assert_eq!(out[2].0, CONTAINER_RESPONSE);
        assert_eq!(out[2].1, RESP_OK);
        assert_eq!(out[3].0, CONTAINER_DATA);
        assert_eq!(
            u32::from_le_bytes([out[3].3[0], out[3].3[1], out[3].3[2], out[3].3[3]]),
            1
        );
        assert_eq!(
            u32::from_le_bytes([out[3].3[4], out[3].3[5], out[3].3[6], out[3].3[7]]),
            STORAGE_ID
        );
        assert_eq!(out[4].1, RESP_OK);
        assert_eq!(out[5].0, CONTAINER_DATA);
        assert_eq!(out[6].1, RESP_OK);
    }

    #[test]
    fn root_enumeration() {
        let out = run(vec![command(
            OP_GET_OBJECT_HANDLES,
            1,
            &[STORAGE_ID, 0, ROOT_PARENT],
        )]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, CONTAINER_DATA);
        let payload = &out[0].3;
        let count = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        assert_eq!(count, 4901);
        assert_eq!(payload.len(), 4 + 4901 * 4);
        let first = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
        assert_eq!(first, 1);
        let last_off = 4 + 4900 * 4;
        let last = u32::from_le_bytes([
            payload[last_off],
            payload[last_off + 1],
            payload[last_off + 2],
            payload[last_off + 3],
        ]);
        assert_eq!(last, 4901);
    }

    #[test]
    fn navigate_and_read() {
        let out = run(vec![
            command(OP_GET_OBJECT_INFO, 1, &[1]),
            command(OP_GET_OBJECT_HANDLES, 2, &[STORAGE_ID, 0, 1]),
            command(OP_GET_OBJECT_INFO, 3, &[8192 + 4901]),
            command(OP_GET_OBJECT, 4, &[8192 + 4901]),
            command(OP_GET_OBJECT, 5, &[4901]),
        ]);
        assert_eq!(out[0].0, CONTAINER_DATA);
        let name_len = out[0].3[52] as usize;
        assert_eq!(&out[0].3[53..53 + 2 * name_len], b"A\0A\0");
        let count = u32::from_le_bytes([out[2].3[0], out[2].3[1], out[2].3[2], out[2].3[3]]);
        assert_eq!(count, 4901);
        let h = u32::from_le_bytes([out[2].3[4], out[2].3[5], out[2].3[6], out[2].3[7]]);
        assert_eq!(h, 8192 + 1);
        assert_eq!(out[6].0, CONTAINER_DATA);
        assert_eq!(out[6].3, vec![0u8]);
        assert_eq!(out[8].0, CONTAINER_DATA);
        assert_eq!(out[8].3.len(), 0);
    }

    #[test]
    fn read_only_ops_rejected() {
        let out = run(vec![
            command(OP_DELETE_OBJECT, 1, &[1, 0]),
            command(OP_SEND_OBJECT_INFO, 2, &[STORAGE_ID, 0, 0]),
            command(OP_SEND_OBJECT, 3, &[]),
            command(OP_FORMAT_STORE, 4, &[STORAGE_ID]),
            command(0x1017, 5, &[]),
        ]);
        for c in out.iter().filter(|c| c.0 == CONTAINER_RESPONSE) {
            assert_eq!(c.1, RESP_OPERATION_NOT_SUPPORTED);
        }
    }

    #[test]
    fn invalid_handles() {
        let out = run(vec![
            command(OP_GET_OBJECT_INFO, 1, &[0]),
            command(OP_GET_OBJECT, 2, &[0xFFFF_FFFF]),
            command(OP_GET_OBJECT_HANDLES, 3, &[STORAGE_ID, 0, 4901]),
        ]);
        for c in out.iter().filter(|c| c.0 == CONTAINER_RESPONSE) {
            assert_eq!(c.1, RESP_INVALID_OBJECT_HANDLE);
        }
    }

    #[test]
    fn malformed_length_does_not_crash() {
        let mut bad = command(OP_GET_OBJECT, 7, &[4901]);
        bad[0..4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let out = run(vec![bad]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, RESP_INVALID_DATASET);
    }

    #[test]
    fn short_packet_ignored() {
        let out = run(vec![vec![1, 2, 3]]);
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn session_state_machine() {
        let out = run(vec![
            command(OP_CLOSE_SESSION, 1, &[]),
            command(OP_OPEN_SESSION, 2, &[1]),
            command(OP_OPEN_SESSION, 3, &[1]),
            command(OP_CLOSE_SESSION, 4, &[]),
            command(OP_CLOSE_SESSION, 5, &[]),
        ]);
        let resps: Vec<u16> = out
            .iter()
            .filter(|c| c.0 == CONTAINER_RESPONSE)
            .map(|c| c.1)
            .collect();
        assert_eq!(
            resps,
            vec![
                RESP_SESSION_NOT_OPEN,
                RESP_OK,
                RESP_SESSION_ALREADY_OPEN,
                RESP_OK,
                RESP_SESSION_NOT_OPEN
            ]
        );
    }

    #[test]
    fn chunked_file_retrieval() {
        let locals: Vec<u16> = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut babel = Babel::new();
        let expected = babel.file_content(&locals).unwrap().to_vec();
        assert!(expected.len() > 4);
        let mut input = Vec::new();
        for (i, h) in locals.iter().enumerate() {
            let handle = ((i as u32) << 13) | (*h as u32);
            input.push(command(
                OP_GET_OBJECT_HANDLES,
                i as u32,
                &[STORAGE_ID, 0, handle],
            ));
        }
        let d = locals.len() as u32;
        let file_handle = (d << 13) | babel::FILE_LOCAL;
        input.push(command(OP_GET_OBJECT, 1000, &[file_handle]));
        let cs = run_mps(input, 16);
        let file = cs
            .iter()
            .rev()
            .find(|c| c.0 == CONTAINER_DATA && c.1 == OP_GET_OBJECT)
            .unwrap();
        assert_eq!(file.3, expected);
        assert_eq!(file.2, 1000);
    }

    #[test]
    fn chunked_handle_streaming() {
        let out = run_mps(
            vec![command(
                OP_GET_OBJECT_HANDLES,
                1,
                &[STORAGE_ID, 0, ROOT_PARENT],
            )],
            16,
        );
        let payload = &out[0].3;
        let count = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        assert_eq!(count, 4901);
        assert_eq!(payload.len(), 4 + 4901 * 4);
        let last_off = 4 + 4900 * 4;
        let last = u32::from_le_bytes([
            payload[last_off],
            payload[last_off + 1],
            payload[last_off + 2],
            payload[last_off + 3],
        ]);
        assert_eq!(last, 4901);
    }

    #[test]
    fn device_properties_and_counts() {
        let out = run(vec![
            command(OP_GET_NUM_OBJECTS, 1, &[STORAGE_ID, 0, ROOT_PARENT]),
            command(OP_GET_DEVICE_PROP_DESC, 2, &[DEV_PROP_FRIENDLY_NAME as u32]),
            command(
                OP_GET_DEVICE_PROP_VALUE,
                3,
                &[DEV_PROP_FRIENDLY_NAME as u32],
            ),
            command(OP_GET_DEVICE_PROP_DESC, 4, &[0x5001]),
            command(OP_GET_DEVICE_PROP_VALUE, 5, &[0x5001]),
            command(OP_GET_OBJECT_HANDLES, 6, &[0xDEAD, 0, ROOT_PARENT]),
            command(OP_GET_STORAGE_INFO, 7, &[0xDEAD]),
        ]);
        assert_eq!(out[0].0, CONTAINER_RESPONSE);
        assert_eq!(out[0].1, RESP_OK);
        assert_eq!(
            u32::from_le_bytes([out[0].3[0], out[0].3[1], out[0].3[2], out[0].3[3]]),
            4901
        );
        assert_eq!(out[1].0, CONTAINER_DATA);
        assert_eq!(out[2].1, RESP_OK);
        assert_eq!(out[3].0, CONTAINER_DATA);
        assert_eq!(out[4].1, RESP_OK);
        assert_eq!(out[5].1, RESP_PARAMETER_NOT_SUPPORTED);
        assert_eq!(out[6].1, RESP_PARAMETER_NOT_SUPPORTED);
        assert_eq!(out[7].1, RESP_INVALID_STORAGE_ID);
        assert_eq!(out[8].1, RESP_INVALID_STORAGE_ID);
    }

    #[test]
    fn repeated_enumeration_is_stable() {
        let mut r = Responder::new();
        r.reset();
        let mut mock = Mock::new(vec![
            command(OP_GET_OBJECT_HANDLES, 1, &[STORAGE_ID, 0, 1]),
            command(OP_GET_OBJECT_HANDLES, 2, &[STORAGE_ID, 0, 1]),
            command(OP_GET_OBJECT_HANDLES, 3, &[STORAGE_ID, 0, 2]),
            command(OP_GET_OBJECT_HANDLES, 4, &[STORAGE_ID, 0, 1]),
        ]);
        embassy_futures::block_on(async {
            while mock.pos < mock.input.len() {
                r.handle(&mut mock, 64).await.unwrap();
            }
        });
        assert_eq!(r.slot_count, 3);
        let cs = containers(&mock.output);
        let handle_of = |i: usize| {
            let p = &cs[i * 2].3;
            u32::from_le_bytes([p[4], p[5], p[6], p[7]])
        };
        assert_eq!(handle_of(0), 8192 + 1);
        assert_eq!(handle_of(1), 8192 + 1);
        assert_eq!(handle_of(2), 16384 + 1);
        assert_eq!(handle_of(3), 8192 + 1);
    }
}
