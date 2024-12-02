use tokio_util::bytes::Buf;
use tokio_util::codec::{Decoder, Encoder};
use tracing::trace;

#[derive(Debug, Clone, Copy)]
pub struct Request {
    pub device_id: u8,
    pub transaction_id: u16,
    pub operation: Operation,
}

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    GetHoldings { address: u16, count: u16 },
}

#[derive(Debug)]
pub struct Response {
    pub device_id: u8,
    pub transaction_id: u16,
    pub kind: ResponseKind,
}

#[derive(Debug)]
pub enum ResponseKind {
    ErrorCode(u8),
    GetHoldings { values: Vec<u8> },
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
                dst.extend(address.to_be_bytes());
                dst.extend(count.to_be_bytes());
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
            if required_length > u8::MAX.into() {
                src.advance(1);
                continue;
            }
            if remainder.len() < required_length.into() {
                return Ok(None);
            }
            let Some((data, _)) = remainder.split_at_checked(required_length.into()) else {
                return Ok(None);
            };
            let [device_id, function_code, byte_count, response @ ..] = data else {
                src.advance(1);
                continue;
            };
            let (device_id, function_code, byte_count) = (*device_id, *function_code, *byte_count);
            if function_code > 0x80 {
                src.advance(6 + 3);
                return Ok(Some(Response {
                    transaction_id,
                    device_id,
                    kind: ResponseKind::ErrorCode(byte_count),
                }));
            } else {
                if usize::from(byte_count) != response.len() {
                    src.advance(1);
                    continue;
                }
                let values = response.to_vec();
                src.advance(usize::from(required_length) + 6);
                return Ok(Some(Response {
                    transaction_id,
                    device_id,
                    kind: match function_code {
                        3 => ResponseKind::GetHoldings { values },
                        _ => continue,
                    },
                }));
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
        req: &Request,
        dst: &mut tokio_util::bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        todo!()
    }
}
impl Decoder for ModbusRTUCodec {
    type Item = Response;
    type Error = std::io::Error;
    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        todo!()
    }
}
impl Codec for ModbusRTUCodec {}
