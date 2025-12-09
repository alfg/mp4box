use mp4box::boxes::{BoxHeader, BoxKey, FourCC};
use mp4box::registry::{BoxDecoder, BoxValue, Registry};
use std::io::Read;

struct DummyDecoder;

impl BoxDecoder for DummyDecoder {
    fn decode(
        &self,
        r: &mut dyn Read,
        _hdr: &BoxHeader,
        _version: Option<u8>,
        _flags: Option<u32>,
    ) -> anyhow::Result<BoxValue> {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf)?;
        Ok(BoxValue::Bytes(buf))
    }
}

#[test]
fn registry_invokes_decoder() {
    let reg = Registry::new().with_decoder(
        BoxKey::FourCC(FourCC(*b"test")),
        "test",
        Box::new(DummyDecoder),
    );

    let hdr = BoxHeader {
        start: 0,
        size: 12,
        header_size: 8,
        typ: FourCC(*b"test"),
        uuid: None,
    };

    let payload = &[1u8, 2, 3, 4];
    let mut cursor = std::io::Cursor::new(payload.to_vec());

    let res = reg.decode(
        &BoxKey::FourCC(FourCC(*b"test")),
        &mut cursor,
        &hdr,
        None,
        None,
    );
    assert!(res.is_some());

    match res.unwrap().unwrap() {
        BoxValue::Bytes(b) => assert_eq!(b, payload),
        _ => panic!("expected bytes"),
    }
}
