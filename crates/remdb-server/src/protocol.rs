use std::io;

const MAX_FRAME_LEN: usize = 64 * 1024 * 1024; // 64 MiB

/// Encode a protobuf payload into a length-prefixed frame.
pub fn encode_frame(payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u32;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

/// Frame decode state: accumulate bytes until one complete frame is available.
#[derive(Default)]
pub struct FrameDecoder {
    /// The 4 length-prefix bytes, accumulated across reads if split.
    pending: Vec<u8>,
    /// The declared payload length once the prefix is known.
    payload_len: Option<usize>,
    /// A partial payload that arrived before its trailing bytes: only used when
    /// a frame spans multiple reads (single-read, full-frame fast path stays
    /// zero-copy / borrowed).
    stash: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push incoming bytes. When a full frame is ready, return its payload as
    /// `Ok(Some((payload, next_slice)))`. `next_slice` is the unconsumed tail of
    /// the input chunk (call repeatedly to drain coalesced/multiple frames).
    ///
    /// The whole frame arriving in one chunk is returned **borrowed** with no
    /// copy. A frame that spans multiple reads is buffered internally and
    /// returned as `FrameRef::Owned`.
    pub fn next<'a>(
        &mut self,
        buf: &'a [u8],
    ) -> io::Result<Option<(FrameRef<'a>, &'a [u8])>> {
        if self.payload_len.is_none() {
            return self.read_prefix_and_payload(buf);
        }
        self.consume_payload(buf)
    }

    /// Determine the 4-byte prefix length, then attempt to read the payload
    /// that may already be present in `buf`.
    fn read_prefix_and_payload<'a>(
        &mut self,
        buf: &'a [u8],
    ) -> io::Result<Option<(FrameRef<'a>, &'a [u8])>> {
        // Fast path: the whole 4-byte prefix is in this chunk.
        if self.pending.is_empty() && buf.len() >= 4 {
            let l = prefix_len(buf)?;
            self.payload_len = Some(l);
            return self.consume_payload(buf.get(4..).unwrap_or(&[]));
        }

        // Slow path: prefix is split across reads (or pending carried bytes).
        let need = 4 - self.pending.len();
        let take = need.min(buf.len());
        self.pending.extend_from_slice(buf.get(..take).unwrap_or(&[]));
        if self.pending.len() < 4 {
            return Ok(None);
        }
        let l = prefix_len(&self.pending)?;
        self.pending.clear();
        self.payload_len = Some(l);
        // Payload bytes (if any) follow the prefix in this chunk.
        self.consume_payload(buf.get(take..).unwrap_or(&[]))
    }

    /// Read the payload for the currently-established frame from `payload_view`
    /// (the portion of the current chunk that holds payload bytes).
    fn consume_payload<'a>(
        &mut self,
        payload_view: &'a [u8],
    ) -> io::Result<Option<(FrameRef<'a>, &'a [u8])>> {
        let l = self
            .payload_len
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no frame in progress"))?;

        if self.stash.is_empty() {
            if payload_view.len() < l {
                // Frame spans reads: buffer what we have for the next chunk.
                self.stash.extend_from_slice(payload_view);
                return Ok(None);
            }
            // Full frame in one chunk — borrow with no copy.
            let (payload, rest) = payload_view.split_at(l);
            self.payload_len = None;
            Ok(Some((FrameRef::Borrowed(payload), rest)))
        } else {
            // Completing a previously-buffered partial payload.
            let have = self.stash.len();
            let need = l - have;
            if payload_view.len() < need {
                self.stash.extend_from_slice(payload_view);
                return Ok(None);
            }
            self.stash.extend_from_slice(payload_view.get(..need).unwrap_or(&[]));
            let payload = std::mem::take(&mut self.stash);
            self.payload_len = None;
            let rest = payload_view.get(need..).unwrap_or(&[]);
            Ok(Some((FrameRef::Owned(payload), rest)))
        }
    }

    pub fn is_idle(&self) -> bool {
        self.pending.is_empty() && self.payload_len.is_none()
    }
}

/// Parse and validate the big-endian length prefix in `bytes`.
fn prefix_len(bytes: &[u8]) -> io::Result<usize> {
    let arr: [u8; 4] = bytes
        .get(..4)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad prefix"))?;
    let l = u32::from_be_bytes(arr) as usize;
    if l > MAX_FRAME_LEN {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    Ok(l)
}

/// A frame payload that is either borrowed from the socket buffer or owned.
pub enum FrameRef<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
}

impl<'a> AsRef<[u8]> for FrameRef<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Borrowed(b) => b,
            Self::Owned(b) => b,
        }
    }
}

/// Convenience: read a single complete frame from a byte slice, returning the
/// number of frames consumed (for the simple unit-test path). Consumes at most
/// one frame per call and requires the whole frame to be present.
fn read_frame<'a>(src: &mut &'a [u8], out: &mut Vec<u8>) -> io::Result<Option<usize>> {
    use std::io::Read;
    if src.len() < 4 {
        return Ok(None);
    }
    let mut prefix = [0u8; 4];
    src.read_exact(&mut prefix)?;
    let len = u32::from_be_bytes(prefix) as usize;
    if len > MAX_FRAME_LEN {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    if src.len() < len {
        return Ok(None);
    }
    out.extend_from_slice(src.get(..len).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "split error")
    })?);
    *src = &src[len..];
    Ok(Some(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_produces_4byte_prefix() {
        let payload = vec![1u8, 2, 3];
        let frame = encode_frame(&payload);
        assert_eq!(frame.len(), 7);
        assert_eq!(&frame[0..4], &[0, 0, 0, 3]);
        assert_eq!(&frame[4..], &[1, 2, 3]);
    }

    #[test]
    fn decode_single_complete_frame() {
        let mut reader: &[u8] = &[0, 0, 0, 3, 9, 9, 9];
        let mut out = Vec::new();
        let n = read_frame(&mut reader, &mut out).expect("decode ok");
        assert_eq!(n, Some(1));
        assert_eq!(out, vec![9, 9, 9]);
    }

    #[test]
    fn decode_short_read_returns_none() {
        let mut reader: &[u8] = &[0, 0, 0, 5, 1, 2]; // claims 5 bytes, has 2
        let mut out = Vec::new();
        out.push(0); // simulate a partial carry buffer, ignored by design
        out.clear();
        assert!(read_frame(&mut reader, &mut out).unwrap().is_none());
    }

    #[test]
    fn decode_rejects_oversized() {
        let mut reader: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, 0, 0];
        let mut out = Vec::new();
        let res = read_frame(&mut reader, &mut out);
        assert!(res.is_err());
    }

    #[test]
    fn single_chunk_full_frame_is_borrowed() {
        let frame = encode_frame(&[9u8, 9, 9]);
        let mut decoder = FrameDecoder::new();
        let got = decoder.next(&frame).expect("ok").expect("frame");
        match got.0 {
            FrameRef::Borrowed(b) => assert_eq!(b, &[9, 9, 9]),
            FrameRef::Owned(_) => panic!("expected borrowed payload"),
        }
        assert!(got.1.is_empty());
        assert!(decoder.is_idle());
    }

    #[test]
    fn frame_split_across_two_reads_is_buffered() {
        let frame = encode_frame(&[9u8, 9, 9]);
        let mut decoder = FrameDecoder::new();
        // First chunk: 2 prefix bytes + 1 payload byte; buffer must retain it.
        let a = decoder.next(&frame[..3]).expect("ok");
        assert!(a.is_none()); // prefix incomplete
        // Second chunk completes the prefix and part of the payload.
        let b = decoder.next(&frame[3..5]).expect("ok");
        assert!(b.is_none()); // payload still incomplete; stashed
        // Third chunk completes the frame; must be Owned (was split).
        let c = decoder.next(&frame[5..]).expect("ok").expect("frame done");
        match c.0 {
            FrameRef::Owned(p) => assert_eq!(p.as_slice(), &[9, 9, 9]),
            FrameRef::Borrowed(_) => panic!("expected owned payload for split frame"),
        }
        assert!(c.1.is_empty());
        assert!(decoder.is_idle());
    }

    #[test]
    fn prefix_split_then_payload_split() {
        let frame = encode_frame(&[1u8, 2, 3, 4, 5]);
        let mut decoder = FrameDecoder::new();
        assert!(decoder.next(&frame[..2]).expect("ok").is_none()); // half prefix
        assert!(decoder.next(&frame[2..4]).expect("ok").is_none()); // prefix done, 0 payload
        assert!(decoder.next(&frame[4..6]).expect("ok").is_none()); // 2/5 payload
        let done = decoder.next(&frame[6..]).expect("ok").expect("done");
        assert_eq!(done.0.as_ref(), &[1, 2, 3, 4, 5]);
        assert!(done.1.is_empty());
        assert!(decoder.is_idle());
    }

    #[test]
    fn multiple_frames_in_one_chunk_drained() {
        let f1 = encode_frame(&[1u8]);
        let f2 = encode_frame(&[2u8, 2]);
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&f1);
        chunk.extend_from_slice(&f2);

        let mut decoder = FrameDecoder::new();
        let (p1, rest) = decoder.next(&chunk).expect("ok").expect("f1");
        assert_eq!(p1.as_ref(), &[1]);
        let (p2, rest2) = decoder.next(rest).expect("ok").expect("f2");
        assert_eq!(p2.as_ref(), &[2, 2]);
        assert!(rest2.is_empty());
        assert!(decoder.is_idle());
    }
}