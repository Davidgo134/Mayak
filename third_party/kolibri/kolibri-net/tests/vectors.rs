use kolibri_net::protocol::{codec, compress, framing, opcodes, packet};
use kolibri_net::{decode, encode};

/// MessagePack for the map {"a": 1}: fixmap(1) + fixstr "a" + int 1 = 4 bytes.
const MSGPACK_A1: [u8; 4] = [0x81, 0xA1, 0x61, 0x01];

#[test]
fn header_layout_matches_dart_wire_format() {
    // Uncompressed small payload → flag byte 0, big-endian seq/opcode/len.
    let bytes = encode(opcodes::LOGIN, &MSGPACK_A1, 5);
    let expected: Vec<u8> = vec![
        10, // ver
        0,  // cmd (request)
        0, 5, // seq = 5
        0, 19, // opcode = LOGIN (19)
        0, 0, 0, 4, // packedLen: flag 0 | len 4
        0x81, 0xA1, 0x61, 0x01, // payload
    ];
    assert_eq!(bytes, expected);
}

#[test]
fn roundtrip_uncompressed() {
    let bytes = encode(opcodes::MSG_SEND, &MSGPACK_A1, 42);
    let pkt = decode(&bytes).unwrap();
    assert_eq!(pkt.ver, packet::PROTOCOL_VERSION);
    assert_eq!(pkt.cmd, packet::cmd::REQUEST);
    assert_eq!(pkt.seq, 42);
    assert_eq!(pkt.opcode, opcodes::MSG_SEND);
    assert_eq!(pkt.payload, MSGPACK_A1);
}

#[test]
fn empty_payload_packet() {
    let bytes = encode(opcodes::PING, &[], 1);
    // header only, len 0
    assert_eq!(bytes.len(), packet::HEADER_SIZE);
    let pkt = decode(&bytes).unwrap();
    assert!(pkt.payload.is_empty());
    assert_eq!(pkt.opcode, opcodes::PING);
}

#[test]
fn compression_kicks_in_past_threshold() {
    // A payload >= 32 bytes must be compressed (flag != 0) and round-trip.
    let big: Vec<u8> = std::iter::repeat_n(0xABu8, 200).collect();
    let bytes = encode(opcodes::MSG_SEND, &big, 7);
    // Flag byte is the high byte of packedLen at offset 6.
    assert_ne!(bytes[6], 0, "expected compression flag to be set");
    let pkt = decode(&bytes).unwrap();
    assert_eq!(pkt.payload, big);
}

#[test]
fn below_threshold_stays_uncompressed() {
    let small: Vec<u8> = std::iter::repeat_n(0x01u8, 31).collect();
    let bytes = encode(opcodes::MSG_SEND, &small, 1);
    assert_eq!(bytes[6], 0, "expected no compression flag");
    let pkt = decode(&bytes).unwrap();
    assert_eq!(pkt.payload, small);
}

#[test]
fn framing_reassembles_across_chunks() {
    let a = encode(opcodes::PING, &MSGPACK_A1, 1);
    let b = encode(opcodes::MSG_SEND, &MSGPACK_A1, 2);
    let c = encode(opcodes::CHATS_LIST, &[], 3);
    let mut stream = Vec::new();
    stream.extend_from_slice(&a);
    stream.extend_from_slice(&b);
    stream.extend_from_slice(&c);

    let mut rx = framing::PacketReceiver::new();
    let mut got = Vec::new();
    // Feed one byte at a time to stress partial-header / partial-payload paths.
    for &byte in &stream {
        for raw in rx.feed(&[byte]).unwrap() {
            got.push(decode(&raw).unwrap());
        }
    }
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].seq, 1);
    assert_eq!(got[1].seq, 2);
    assert_eq!(got[2].seq, 3);
    assert_eq!(rx.buffered_len(), 0);
}

#[test]
fn framing_handles_multiple_packets_in_one_chunk() {
    let a = encode(opcodes::PING, &MSGPACK_A1, 10);
    let b = encode(opcodes::PING, &MSGPACK_A1, 11);
    let mut joined = a.clone();
    joined.extend_from_slice(&b);

    let mut rx = framing::PacketReceiver::new();
    let packets = rx.feed(&joined).unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(decode(&packets[0]).unwrap().seq, 10);
    assert_eq!(decode(&packets[1]).unwrap().seq, 11);
}

#[test]
fn lz4_block_literals_only() {
    // token 0x50 = 5 literals, then "hello".
    let src = [0x50, b'h', b'e', b'l', b'l', b'o'];
    let out = compress::decompress_lz4_block(&src, compress::MAX_DECOMPRESSED_SIZE).unwrap();
    assert_eq!(out, b"hello");
}

#[test]
fn lz4_block_with_match_copy() {
    // 1 literal 'a', then a back-reference of length 4 at offset 1 → "aaaaa".
    let src = [0x10, b'a', 0x01, 0x00];
    let out = compress::decompress_lz4_block(&src, compress::MAX_DECOMPRESSED_SIZE).unwrap();
    assert_eq!(out, b"aaaaa");
}

#[test]
fn lz4_block_via_sniffer_default_path() {
    // No magic prefix → sniffer must fall through to block decode.
    let src = [0x50, b'w', b'o', b'r', b'l', b'd'];
    let out = compress::decompress(&src).unwrap();
    assert_eq!(out, b"world");
}

#[test]
fn lz4_frame_roundtrip_and_sniff() {
    let data: Vec<u8> = (0..500).map(|i| (i % 7) as u8).collect();
    let framed = compress::compress_lz4_frame(&data);
    // Frame magic present.
    assert_eq!(&framed[0..4], &[0x04, 0x22, 0x4D, 0x18]);
    let out = compress::decompress(&framed).unwrap();
    assert_eq!(out, data);
}

#[test]
fn zstd_sniff_and_decompress() {
    let data: Vec<u8> = (0..1000).map(|i| (i % 13) as u8).collect();
    let compressed = zstd::stream::encode_all(&data[..], 3).unwrap();
    assert_eq!(&compressed[0..4], &[0x28, 0xB5, 0x2F, 0xFD]);
    let out = compress::decompress(&compressed).unwrap();
    assert_eq!(out, data);
}

#[test]
fn decoded_payload_reads_back_as_msgpack_value() {
    let bytes = encode(opcodes::MSG_SEND, &MSGPACK_A1, 1);
    let pkt = decode(&bytes).unwrap();
    let value = pkt.value().unwrap();
    let map = value.as_map().expect("expected a map");
    assert_eq!(map.len(), 1);
    assert_eq!(map[0].0.as_str(), Some("a"));
    assert_eq!(map[0].1.as_u64(), Some(1));
}

#[test]
fn packet_total_len_reads_header() {
    let bytes = encode(opcodes::MSG_SEND, &MSGPACK_A1, 1);
    assert_eq!(codec::packet_total_len(&bytes), Some(bytes.len()));
    // Fewer than a full header → None.
    assert_eq!(codec::packet_total_len(&bytes[..4]), None);
}
