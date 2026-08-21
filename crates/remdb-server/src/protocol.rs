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
    // prefix stores the 4 length bytes; once filled it fixes payload_len
    pending: Vec<u8>,
    payload_len: Option<usize>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push incoming bytes. When a full frame is ready, return its protobuf
    /// payload (borrowed slice) as `Ok(Some((payload, next_slice)))`. With a
    /// **single** input chunk this returns at most one frame; call repeatedly
    /// with remaining bytes to drain the rest.
    pub fn next<'a>(
        &mut self,
        buf: &'a [u8],
    ) -> io::Result<Option<(FrameRef<'a>, &'a [u8])>> {
        let payload_len = match self.payload_len {
            Some(l) => l,
            None => {
                if self.pending.len() < 4 {
                    let need = 4 - self.pending.len();
                    self.pending.extend_from_slice(buf.get(..need).unwrap_or(buf));
                    if self.pending.len() < 4 {
                        return Ok(None);
                    }
                }
                let bytes: [u8; 4] = self
                    .pending
                    .get(..4)
                    .and_then(|s| s.try_into().ok())
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad prefix"))?;
                let l = u32::from_be_bytes(bytes) as usize;
                if l > MAX_FRAME_LEN {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
                }
                self.pending.clear();
                self.payload_len = Some(l);
                l
            }
        };

        // We have payload_len. All payload bytes are in `buf` (this decoder is
        // used with pooled buffers that always start a fresh frame).
        if buf.len() < payload_len {
            return Ok(None);
        }
        let (payload, rest) = buf.split_at(payload_len);
        self.payload_len = None;
        self.pending.clear();
        Ok(Some((FrameRef::Borrowed(payload), rest)))
    }

    pub fn is_idle(&self) -> bool {
        self.pending.is_empty() && self.payload_len.is_none()
    }
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
}