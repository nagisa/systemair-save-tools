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
                dst.extend((address - 1).to_be_bytes());
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
            let Some((data, _)) = remainder.split_at_checked(required_length.into()) else {
                return Ok(None);
            };
            let [device_id, function_code, code, response @ ..] = data else {
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
