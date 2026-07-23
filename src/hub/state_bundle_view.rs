use std::{iter::FusedIterator, ops::Range};

use super::{
    Fragment, ReplicatedStateBundle, ReplicatedStateBundleView, StateFragmentHeaderSpan,
    StateRecordHeader, read_state_fragment_header, read_state_record_header,
};
use crate::serialize::{CARRIER_ENDIAN, MarshalerError, ReadBuffer};

#[derive(Debug)]
pub struct StateFragmentView<'a> {
    pub record: StateRecordHeader,
    pub header: StateFragmentHeaderSpan,
    pub body: &'a [u8],
    pub body_range: Range<usize>,
    fragment: Box<dyn Fragment>,
}

impl StateFragmentView<'_> {
    /// Return the fragment value decoded while walking the bundle stream.
    ///
    /// This is the same pass that advances [`StateFragmentIter`]; callers do
    /// not need to decode `body` again to inspect the value.
    #[must_use]
    pub fn value(&self) -> &dyn Fragment {
        self.fragment.as_ref()
    }

    /// Return the mutable fragment value decoded while walking the bundle stream.
    pub fn value_mut(&mut self) -> &mut dyn Fragment {
        self.fragment.as_mut()
    }

    /// Downcast the decoded fragment value to its concrete generated state type.
    #[must_use]
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Fragment + 'static,
    {
        self.fragment.as_ref().downcast_ref::<T>()
    }

    /// Mutably downcast the decoded fragment value to its concrete generated state type.
    #[must_use]
    pub fn downcast_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Fragment + 'static,
    {
        self.fragment.as_mut().downcast_mut::<T>()
    }

    /// Consume the view and return the decoded fragment value.
    #[must_use]
    pub fn into_fragment(self) -> Box<dyn Fragment> {
        self.fragment
    }
}

pub struct StateFragmentIter<'a> {
    rb: ReadBuffer<'a>,
    current_record: Option<StateRecordHeader>,
    fragments_left_in_record: usize,
    exhausted: bool,
}

impl<'a> StateFragmentIter<'a> {
    #[must_use]
    pub fn new(bundle_buffer: &'a [u8]) -> Self {
        Self {
            rb: ReadBuffer::new(CARRIER_ENDIAN, bundle_buffer),
            current_record: None,
            fragments_left_in_record: 0,
            exhausted: false,
        }
    }

    fn read_next_record(&mut self) -> Option<Result<(), MarshalerError>> {
        if self.rb.is_empty() {
            self.exhausted = true;
            return None;
        }

        match read_state_record_header(&mut self.rb) {
            Ok(record) => {
                self.fragments_left_in_record = record.fragment_count;
                self.current_record = Some(record);
                Some(Ok(()))
            }
            Err(err) => {
                self.exhausted = true;
                Some(Err(err))
            }
        }
    }

    fn read_next_fragment(&mut self) -> Result<StateFragmentView<'a>, MarshalerError> {
        let record = self
            .current_record
            .expect("state fragment iterator has an active record");
        let header = read_state_fragment_header(&mut self.rb)?;
        let body_start = self.rb.position();

        let fragment = header.type_info.decode_contents(&mut self.rb)?;

        let body_end = self.rb.position();
        self.fragments_left_in_record -= 1;
        Ok(StateFragmentView {
            record,
            header,
            body: self.rb.range(body_start..body_end)?,
            body_range: body_start..body_end,
            fragment,
        })
    }
}

impl<'a> Iterator for StateFragmentIter<'a> {
    type Item = Result<StateFragmentView<'a>, MarshalerError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.exhausted {
                return None;
            }

            if self.fragments_left_in_record == 0 {
                match self.read_next_record()? {
                    Ok(()) => {}
                    Err(err) => return Some(Err(err)),
                }
                continue;
            }

            return Some(match self.read_next_fragment() {
                Ok(fragment) => Ok(fragment),
                Err(err) => {
                    self.exhausted = true;
                    Err(err)
                }
            });
        }
    }
}

impl FusedIterator for StateFragmentIter<'_> {}

impl ReplicatedStateBundle {
    #[must_use]
    pub fn fragments(&self) -> StateFragmentIter<'_> {
        StateFragmentIter::new(&self.bundle_buffer)
    }
}

impl<'a> ReplicatedStateBundleView<'a> {
    #[must_use]
    pub fn fragments(&self) -> StateFragmentIter<'a> {
        StateFragmentIter::new(self.bundle_buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Fragment, az_rtti,
        hub::{
            DynFragment, FragmentBase, FragmentKey, FragmentTypeInfo, InterestId, StateRecordWriter,
        },
        serialize::{Marshal, Unmarshal, WriteBuffer},
        type_registry,
    };

    #[derive(Debug, Default, Fragment)]
    #[az_rtti("22222222-2222-4222-8222-222222222222")]
    #[type_registry(64_991)]
    struct ByteFragment {
        base: FragmentBase,
        value: u8,
    }

    impl ByteFragment {
        fn new(value: u8) -> Self {
            Self {
                base: FragmentBase::default(),
                value,
            }
        }
    }
    impl DynFragment for ByteFragment {
        fn base(&self) -> &FragmentBase {
            &self.base
        }

        fn base_mut(&mut self) -> &mut FragmentBase {
            &mut self.base
        }

        fn marshal_contents(&self, wb: &mut WriteBuffer) -> bool {
            self.value.marshal(wb);
            true
        }

        fn unmarshal_contents(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
            self.value = u8::unmarshal(rb)?;
            Ok(true)
        }
    }

    impl Fragment for ByteFragment {}

    #[test]
    fn fragment_iterator_yields_borrowed_bodies_and_headers() {
        let mut bundle = ReplicatedStateBundle::default();
        let _ = bundle
            .write_record(7, |record: &mut StateRecordWriter<'_>| {
                record.write_fragment(3, &ByteFragment::new(0xcc))
            })
            .unwrap();

        let fragments = bundle.fragments().collect::<Result<Vec<_>, _>>().unwrap();

        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].record.interest_id, InterestId::new(7));
        assert_eq!(fragments[0].header.fragment_key, FragmentKey::new(3));
        assert_eq!(
            fragments[0].header.type_info,
            FragmentTypeInfo::registered::<ByteFragment>()
        );
        assert_eq!(fragments[0].body, &[0xcc]);
        assert_eq!(
            fragments[0].downcast_ref::<ByteFragment>().unwrap().value,
            0xcc
        );
    }
}
