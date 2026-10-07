use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_usb::control::{InResponse, OutResponse, Recipient, Request, RequestType};
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::{Builder, Handler};

use crate::mtp::Transport;
use crate::mtp::responder::Responder;

pub const IFACE_CLASS: u8 = 0x06;
pub const IFACE_SUBCLASS: u8 = 0x01;
pub const IFACE_PROTOCOL: u8 = 0x01;

const REQ_CANCEL: u8 = 0x64;
const REQ_RESET: u8 = 0x66;
const REQ_GET_DEVICE_STATUS: u8 = 0x67;

const RESP_OK: u16 = 0x2001;

pub type ResetSignal = Signal<CriticalSectionRawMutex, ()>;

pub struct Control {
    iface: u8,
    reset: &'static ResetSignal,
}

impl Control {
    pub fn new(iface: u8, reset: &'static ResetSignal) -> Self {
        Self { iface, reset }
    }

    fn matches(&self, req: &Request) -> bool {
        req.request_type == RequestType::Class
            && req.recipient == Recipient::Interface
            && req.index == self.iface as u16
    }
}

impl Handler for Control {
    fn reset(&mut self) {
        self.reset.signal(());
    }

    fn control_in<'a>(&'a mut self, req: Request, buf: &'a mut [u8]) -> Option<InResponse<'a>> {
        if !self.matches(&req) {
            return None;
        }
        match req.request {
            REQ_GET_DEVICE_STATUS => {
                buf[0] = 4;
                buf[1] = 0;
                buf[2] = (RESP_OK & 0xFF) as u8;
                buf[3] = (RESP_OK >> 8) as u8;
                Some(InResponse::Accepted(&buf[..4]))
            }
            _ => Some(InResponse::Rejected),
        }
    }

    fn control_out(&mut self, req: Request, _data: &[u8]) -> Option<OutResponse> {
        if !self.matches(&req) {
            return None;
        }
        match req.request {
            REQ_CANCEL => Some(OutResponse::Accepted),
            REQ_RESET => {
                self.reset.signal(());
                Some(OutResponse::Accepted)
            }
            _ => Some(OutResponse::Rejected),
        }
    }
}

pub struct MtpTransport<'e, EO, EI> {
    out: &'e mut EO,
    inn: &'e mut EI,
}

impl<EO: EndpointOut, EI: EndpointIn> Transport for MtpTransport<'_, EO, EI> {
    type Error = EndpointError;

    async fn read_packet(&mut self, buf: &mut [u8]) -> Result<usize, EndpointError> {
        self.out.read(buf).await
    }

    async fn write_packet(&mut self, buf: &[u8]) -> Result<(), EndpointError> {
        self.inn.write(buf).await
    }
}

pub struct MtpClass<'d, D: Driver<'d>> {
    ep_out: D::EndpointOut,
    ep_in: D::EndpointIn,
    _evt: D::EndpointIn,
    responder: &'d mut Responder,
    reset: &'static ResetSignal,
    mps: usize,
}

impl<'d, D: Driver<'d>> MtpClass<'d, D> {
    pub fn new(
        builder: &mut Builder<'d, D>,
        responder: &'d mut Responder,
        control: &'d mut Control,
        reset: &'static ResetSignal,
    ) -> Self {
        let (ep_out, ep_in, evt) = {
            let mut func = builder.function(IFACE_CLASS, IFACE_SUBCLASS, IFACE_PROTOCOL);
            let mut iface = func.interface();
            let mut alt = iface.alt_setting(IFACE_CLASS, IFACE_SUBCLASS, IFACE_PROTOCOL, None);
            let evt = alt.endpoint_interrupt_in(None, 64, 1);
            let ep_out = alt.endpoint_bulk_out(None, 64);
            let ep_in = alt.endpoint_bulk_in(None, 64);
            (ep_out, ep_in, evt)
        };
        builder.handler(control);
        Self {
            ep_out,
            ep_in,
            _evt: evt,
            responder,
            reset,
            mps: 64,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            self.ep_in.wait_enabled().await;
            self.ep_out.wait_enabled().await;
            self.reset.reset();
            self.responder.reset();
            let mut transport = MtpTransport {
                out: &mut self.ep_out,
                inn: &mut self.ep_in,
            };
            loop {
                match select(
                    self.responder.handle(&mut transport, self.mps),
                    self.reset.wait(),
                )
                .await
                {
                    Either::First(Ok(())) => {}
                    Either::First(Err(_)) => break,
                    Either::Second(()) => {
                        self.responder.reset();
                        break;
                    }
                }
            }
        }
    }
}
