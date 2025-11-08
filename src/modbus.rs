use tokio_util::bytes::Buf;
use tokio_util::codec::{Decoder, Encoder};
use tracing::trace;

use crate::registers::{RegisterIndex, Value};

pub const MAX_SAFE_READ_COUNT: u16 = 123;

#[derive(Debug, Clone, Copy)]
pub struct Request {
    pub device_id: u8,
    pub transaction_id: u16,
    pub operation: Operation,
}

impl Request {
    /// Estimate how many bytes of response will be necessary to respond to this request.
    ///
    /// This is used to calculate send slots on a connection in an effort to avoid predictable
    /// "Server Busy" exceptions or loss of requests.
    pub fn expected_response_length(&self) -> u16 {
        let bytes_total = match self.operation {
            Operation::GetHoldings { address: _, count } => u32::from(count) * 2,
            Operation::SetHolding { .. } => 2,
        };
        let rtu_blocks = (bytes_total + 0xFE) / 0xFF;
        // 1 byte no padding, 2 bytes crc, 2 bytes address and function.
        let rtu_bytes = rtu_blocks * 5 + bytes_total;
        u16::try_from(rtu_bytes).unwrap_or(u16::MAX)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    GetHoldings { address: u16, count: u16 },
    SetHolding { address: u16, value: u16 },
}

#[derive(Debug)]
pub struct Response {
    pub device_id: u8,
    pub transaction_id: u16,
    pub kind: ResponseKind,
}

impl Response {
    pub fn exception_code(&self) -> Option<u8> {
        match &self.kind {
            ResponseKind::ErrorCode(c) => Some(*c),
            ResponseKind::GetHoldings { values: _ } => None,
            ResponseKind::SetHolding { value: _ } => None,
        }
    }

    pub fn is_server_busy(&self) -> bool {
        self.exception_code() == Some(6)
    }
}

#[derive(Debug)]
pub enum ResponseKind {
    ErrorCode(u8),
    GetHoldings { values: Vec<u8> },
    SetHolding { value: u16 },
}

pub trait Codec:
    for<'a> Encoder<&'a Request, Error = std::io::Error>
    + Decoder<Item = Response, Error = std::io::Error>
{
}

pub struct ModbusTCPCodec {}
impl Encoder<&Request> for ModbusTCPCodec {
    type Error = std::io::Error;
    fn encode(
        &mut self,
        req: &Request,
        dst: &mut tokio_util::bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        match req.operation {
            Operation::GetHoldings { address, count } => {
                dst.extend(req.transaction_id.to_be_bytes());
                dst.extend(&[0, 0, 0, 0, req.device_id, 3]);
                dst.extend((address - 1).to_be_bytes());
                dst.extend(count.to_be_bytes());
            }
            Operation::SetHolding { address, value } => {
                dst.extend(req.transaction_id.to_be_bytes());
                dst.extend(&[0, 0, 0, 0, req.device_id, 6]);
                dst.extend((address - 1).to_be_bytes());
                dst.extend(value.to_be_bytes());
            }
        };
        trace!(message="sending encoded", buffer=?dst);
        Ok(())
    }
}
impl Decoder for ModbusTCPCodec {
    type Item = Response;
    type Error = std::io::Error;
    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            trace!(message="attempt at decoding", buffer=?src);
            if src.len() < 8 {
                return Ok(None);
            }
            let Some((tr_id_buffer, remainder)) = src.split_first_chunk::<2>() else {
                return Ok(None);
            };
            let transaction_id = u16::from_be_bytes(*tr_id_buffer);
            let Some((proto_buffer, remainder)) = remainder.split_first_chunk::<2>() else {
                return Ok(None);
            };
            let proto = u16::from_be_bytes(*proto_buffer);
            if proto != 0 {
                src.advance(1);
                continue;
            }
            let Some((length_buffer, remainder)) = remainder.split_first_chunk::<2>() else {
                return Ok(None);
            };
            let required_length = u16::from_be_bytes(*length_buffer);
            let Some((data, _)) = remainder.split_at_checked(required_length.into()) else {
                return Ok(None);
            };
            let [device_id, function_code, code, ..] = data else {
                src.advance(1);
                continue;
            };
            let (device_id, function_code, code) = (*device_id, *function_code, *code);
            if function_code > 0x80 {
                src.advance(6 + 3);
                return Ok(Some(Response {
                    transaction_id,
                    device_id,
                    kind: ResponseKind::ErrorCode(code),
                }));
            } else {
                // NOTE: The `code` variable in the case of success might store the length of the
                // payload. However, the IAM is capable of handling larger responses (such as when
                // querying large register ranges) than 254 bytes, in which case the value of this
                // byte is sorta unspecified. We already have a length to consult from the TCP
                // header, so there kinda isn't any reason to check this byte...
                //
                // This is just one of the ways in which SystemAIR Modbus implementation is special
                // such that using off-shelf parsers doesn't work well.
                let result = Ok(Some(Response {
                    transaction_id,
                    device_id,
                    kind: match function_code {
                        3 => {
                            let [_, _, _, values@..] = data else { unreachable!() };
                            ResponseKind::GetHoldings { values: values.to_vec() }
                        }
                        6 => {
                            let [_, _, _, _, a, b] = data else { unreachable!() };
                            ResponseKind::SetHolding { value: u16::from_be_bytes([*a, *b]) }
                        }
                        _ => continue,
                    },
                }));
                src.advance(usize::from(required_length) + 6);
                return result;
            }
        }
    }
}
impl Codec for ModbusTCPCodec {}

pub struct ModbusRTUCodec {}
impl Encoder<&Request> for ModbusRTUCodec {
    type Error = std::io::Error;
    fn encode(
        &mut self,
        _req: &Request,
        _dst: &mut tokio_util::bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        todo!()
    }
}
impl Decoder for ModbusRTUCodec {
    type Item = Response;
    type Error = std::io::Error;
    fn decode(
        &mut self,
        _src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        todo!()
    }
}
impl Codec for ModbusRTUCodec {}

pub fn extract_value(request_base: u16, value_address: u16, response: &[u8]) -> Option<Value> {
    let value_register = RegisterIndex::from_address(value_address).unwrap();
    let value_offset = 2 * usize::from(value_address - request_base);
    let value_data_type = value_register.data_type();
    value_data_type
        .from_bytes(&response[value_offset..][..value_data_type.bytes()])
        .next()
}
