// Request-scoped Go HPACK policy; included in the upstream encoder module.
// The original encoder/table remain authoritative for unmarked requests.
impl Encoder {
    pub(crate) fn encode_go<I>(&mut self, headers: I, dst: &mut BytesMut)
    where
        I: IntoIterator<Item = Header<Option<HeaderName>>>,
    {
        // Go emits the peer's table-size setting even when it repeats the current size.
        if let Some(size) = self.go_peer_size.take() {
            if self.size_update.is_none() {
                self.size_update = Some(SizeUpdate::One(size));
            }
        }
        self.encode_size_updates(dst);
        let mut previous: Option<HeaderName> = None;
        for header in headers {
            let header = match header.reify() {
                Ok(header) => header,
                Err(value) => Header::Field {
                    name: previous.clone().expect("HeaderMap value follows its name"),
                    value,
                },
            };
            if let Header::Field { name, .. } = &header {
                previous = Some(name.clone());
            }
            // Repeated field names still need lookup/insertion for each distinct value.
            let index = self.table.index_go(header);
            let header = self.table.resolve(&index);
            match index {
                Index::Indexed(value, _) => encode_int(value, 7, 0x80, dst),
                Index::Inserted(_) => {
                    dst.put_u8(0x40);
                    go_string(header.name().as_slice(), dst);
                    go_string(header.value_slice(), dst);
                }
                Index::InsertedValue(name, _) => {
                    encode_int(name, 6, 0x40, dst);
                    go_string(header.value_slice(), dst);
                }
                Index::Name(name, _) => {
                    encode_int(name, 4, 0, dst);
                    go_string(header.value_slice(), dst);
                }
                Index::NotIndexed(_) => {
                    dst.put_u8(0);
                    go_string(header.name().as_slice(), dst);
                    go_string(header.value_slice(), dst);
                }
            }
        }
    }
}

fn go_string(value: &[u8], dst: &mut BytesMut) {
    let mut encoded = BytesMut::with_capacity(value.len());
    huffman::encode(value, &mut encoded);
    if encoded.len() < value.len() {
        encode_int(encoded.len(), 7, 0x80, dst);
        dst.extend_from_slice(&encoded);
    } else {
        encode_int(value.len(), 7, 0, dst);
        dst.extend_from_slice(value);
    }
}
