use crate::frame::*;

use smallvec::SmallVec;

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

const DEFAULT_STACK_SIZE: usize = 8;

/// A collection of HTTP/2 PRIORITY frames.
///
/// The `Priorities` struct maintains an ordered list of `Priority` frames,
/// which can be used to represent and manage the stream dependency tree
/// in HTTP/2. This is useful for pre-configuring stream priorities or
/// sending multiple PRIORITY frames at once during connection setup or
/// stream reprioritization.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Priorities {
    priorities: SmallVec<[Priority; DEFAULT_STACK_SIZE]>,
}

/// A builder for constructing a `Priorities` collection.
///
/// `PrioritiesBuilder` provides a convenient way to incrementally add
/// `Priority` frames to a collection, ensuring that invalid priorities
/// (such as those with a stream ID of zero) are ignored. Once all desired
/// priorities have been added, call `.build()` to obtain a `Priorities`
/// instance.
#[derive(Debug)]
pub struct PrioritiesBuilder {
    priorities: SmallVec<[Priority; DEFAULT_STACK_SIZE]>,
    inserted_bitmap: u32,
}

// ===== impl Priorities =====

impl Priorities {
    pub fn builder() -> PrioritiesBuilder {
        PrioritiesBuilder {
            priorities: SmallVec::new(),
            inserted_bitmap: 0,
        }
    }
}

impl IntoIterator for Priorities {
    type Item = Priority;
    type IntoIter = std::vec::IntoIter<Priority>;

    fn into_iter(self) -> Self::IntoIter {
        self.priorities.into_vec().into_iter()
    }
}

// ===== impl PrioritiesBuilder =====

impl PrioritiesBuilder {
    pub fn push(mut self, priority: Priority) -> Self {
        if priority.stream_id.is_zero() {
            tracing::warn!("ignoring priority frame with stream ID 0");
            return self;
        }

        const MAX_BITMAP_STREAMS: u32 = 32;

        let id: u32 = priority.stream_id.into();
        // Check for duplicate priorities based on stream ID.
        // For stream IDs less than MAX_BITMAP_STREAMS, we use a bitmap to track inserted priorities.
        // For stream IDs greater than or equal to MAX_BITMAP_STREAMS, duplicate checking is still performed using iterators.
        if id < MAX_BITMAP_STREAMS {
            let mask = 1u32 << id;
            if self.inserted_bitmap & mask != 0 {
                tracing::debug!(
                    "duplicate priority for stream_id={:?} ignored",
                    priority.stream_id
                );
                return self;
            }
            self.inserted_bitmap |= mask;
        } else {
            // For stream_id >= MAX_BITMAP_STREAMS, duplicate checking is still performed using iterators.
            if self
                .priorities
                .iter()
                .any(|p| p.stream_id == priority.stream_id)
            {
                tracing::debug!(
                    "duplicate priority for stream_id={:?} ignored",
                    priority.stream_id
                );
                return self;
            }
        }

        self.priorities.push(priority);
        self
    }

    pub fn extend(mut self, priorities: impl IntoIterator<Item = Priority>) -> Self {
        for priority in priorities {
            self = self.push(priority);
        }
        self
    }

    pub fn build(self) -> Priorities {
        Priorities {
            priorities: self.priorities,
        }
    }
}

impl Priority {
    pub fn load(head: Head, _payload: &[u8]) -> Result<Self, Error> {
        //let dependency = StreamDependency::load(payload)?;
        //
        //if dependency.dependency_id() == head.stream_id() {
        //    return Err(Error::InvalidDependencyId);
        //}

        // Ignore whatever is on the wire; always emit Chrome’s fixed priority.
        let dependency = StreamDependency::chrome();

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

    /// Chrome’s fixed priority triple: dependency=0, weight=255, exclusive=true
    pub fn chrome() -> Self {
        StreamDependency {
            dependency_id: StreamId::zero(),
            weight: 255,
            is_exclusive: true,
        }
    }
}
