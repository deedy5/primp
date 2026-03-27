use crate::frame::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Priority {
    stream_id: StreamId,
    dependency: StreamDependency,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StreamDependency {
    /// The ID of the stream dependency target
    dependency_id: StreamId,

    /// The weight for the stream. The value exposed (and set) here is always in
    /// the range [0, 255], instead of [1, 256] (as defined in section 5.3.2.)
    /// so that the value fits into a `u8`.
    weight: u8,

    /// True if the stream dependency is exclusive.
    is_exclusive: bool,
}

impl Priority {
    pub fn load(head: Head, _payload: &[u8]) -> Result<Self, Error> {
        // Ignore whatever is on the wire; always emit Chrome's fixed priority.
        let dependency = StreamDependency {
            dependency_id: StreamId::zero(),
            weight: 255,
            is_exclusive: true,
        };

        Ok(Priority {
            stream_id: head.stream_id(),
            dependency,
        })
    }
}

impl<B> From<Priority> for Frame<B> {
    fn from(src: Priority) -> Self {
        Frame::Priority(src)
    }
}

// ===== impl StreamDependency =====

impl StreamDependency {
    pub fn new(dependency_id: StreamId, weight: u8, is_exclusive: bool) -> Self {
        StreamDependency {
            dependency_id,
            weight,
            is_exclusive,
        }
    }

    pub fn load(src: &[u8]) -> Result<Self, Error> {
        if src.len() != 5 {
            return Err(Error::InvalidPayloadLength);
        }

        // Parse the stream ID and exclusive flag
        let (dependency_id, is_exclusive) = StreamId::parse(&src[..4]);

        // Read the weight
        let weight = src[4];

        Ok(StreamDependency::new(dependency_id, weight, is_exclusive))
    }

    pub fn dependency_id(&self) -> StreamId {
        self.dependency_id
    }

    pub fn encode<B: bytes::BufMut>(&self, dst: &mut B) {
        let mut dependency_id = u32::from(self.dependency_id);

        if self.is_exclusive {
            dependency_id |= 1 << 31;
        }

        dst.put_u32(dependency_id);
        dst.put_u8(self.weight);
    }
}
