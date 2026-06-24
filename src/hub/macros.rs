/// Wire a manual `*ReplicatedState` struct into the hub hierarchy.
#[macro_export]
macro_rules! impl_hub_fragment {
    (
        $ty:ty,
        hub = $hub:ident,
        marshal = $marshal:ident,
        unmarshal = $unmarshal:ident $(,)?
    ) => {
        $crate::impl_hub_fragment!(@impl $ty, $hub, $marshal, $unmarshal);
    };
    (
        $ty:ty,
        hub = $hub:ident,
        marshal = $marshal:ident,
        unmarshal = $unmarshal:ident,
        category = $category:expr $(,)?
    ) => {
        $crate::impl_hub_fragment!(@impl $ty, $hub, $marshal, $unmarshal, category = $category);
    };
    (
        $ty:ty,
        hub = $hub:ident,
        marshal = $marshal:ident,
        unmarshal = $unmarshal:ident,
        world_position = $pos:ident $(,)?
    ) => {
        $crate::impl_hub_fragment!(@impl $ty, $hub, $marshal, $unmarshal, world_position = $pos);
    };
    (
        $ty:ty,
        hub = $hub:ident,
        marshal = $marshal:ident,
        unmarshal = $unmarshal:ident,
        category = $category:expr,
        world_position = $pos:ident $(,)?
    ) => {
        $crate::impl_hub_fragment!(
            @impl
            $ty,
            $hub,
            $marshal,
            $unmarshal,
            category = $category,
            world_position = $pos
        );
    };
    (
        $ty:ty,
        hub = $hub:ident,
        marshal = $marshal:ident,
        unmarshal = $unmarshal:ident,
        filter_groups = $groups:expr $(,)?
    ) => {
        $crate::impl_hub_fragment!(@impl $ty, $hub, $marshal, $unmarshal, filter_groups = $groups);
    };
    (
        @impl
        $ty:ty,
        $hub:ident,
        $marshal:ident,
        $unmarshal:ident
        $(, category = $category:expr)?
        $(, world_position = $pos:ident)?
        $(, filter_groups = $groups:expr)?
    ) => {
        impl $crate::hub::DynFragment for $ty {
            fn base(&self) -> &$crate::hub::FragmentBase {
                self.$hub.base()
            }

            fn base_mut(&mut self) -> &mut $crate::hub::FragmentBase {
                self.$hub.base_mut()
            }

            fn marshal_contents(
                &self,
                wb: &mut $crate::serialize::buffer::WriteBuffer,
            ) -> bool {
                self.$marshal(wb);
                true
            }

            fn unmarshal_contents(
                &mut self,
                rb: &mut $crate::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<bool, $crate::serialize::error::MarshalerError> {
                self.$unmarshal(rb)?;
                Ok(true)
            }
        }

        impl $crate::hub::Fragment for $ty {
            $(
                fn category(&self) -> $crate::hub::FragmentCategory {
                    $category
                }
            )?

            $(
                fn has_world_position(&self) -> bool {
                    true
                }

                fn world_position(&self) -> ::core::option::Option<::glam::Vec3> {
                    self.$pos.value().copied().map(|(x, y, height)| {
                        ::glam::Vec3::new(x, height, y)
                    })
                }
            )?

            $(
                fn num_filter_groups(&self) -> usize {
                    $groups
                }
            )?
        }
    };
}
