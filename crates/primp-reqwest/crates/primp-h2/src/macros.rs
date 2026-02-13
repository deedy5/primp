macro_rules! enum_count {
    () => {0};
    ($_head:ident $(, $tail:ident)*) => {1 + enum_count!($($tail),*)};
}

macro_rules! define_enum_with_values {
    (
        $(#[$enum_meta:meta])*
        @U16
        $vis:vis enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $value:expr,
            )*
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
        #[repr(u16)]
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant = $value,
            )*
        }

        impl $name {
            const MAX_ID: u16 = 15;
            const DEFAULT_STACK_SIZE: usize = enum_count!($($variant),*);
            const DEFAULT_IDS: [$name; Self::DEFAULT_STACK_SIZE] = [
                $(
                    $name::$variant,
                )*
            ];

            fn mask_id(self) -> u16 {
                let value = u16::from(self);
                if value == 0 || value > Self::MAX_ID {
                    return 0;
                }

                1 << (value - 1)
            }
        }

        impl From<$name> for u16 {
            fn from(src: $name) -> u16 {
                match src {
                    $(
                        $name::$variant => $value,
                    )*
                }
            }
        }

        impl From<u16> for $name {
            fn from(id: u16) -> $name {
                match id {
                    $(
                        $value => $name::$variant,
                    )*
                    _ => panic!("Invalid {} value: {}", stringify!($name), id),
                }
            }
        }
    };

    (
        $(#[$enum_meta:meta])*
        @U8
        $vis:vis enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $value:expr,
            )*
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
        #[repr(u8)]
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant = $value,
            )*
        }

        impl $name {
            const MAX_ID: u8 = 7;
            const DEFAULT_STACK_SIZE: usize = enum_count!($($variant),*);
            const DEFAULT_IDS: [$name; Self::DEFAULT_STACK_SIZE] = [
                $(
                    $name::$variant,
                )*
            ];

            fn mask_id(self) -> u8 {
                let value = u8::from(self);
                if value == 0 || value > Self::MAX_ID {
                    return 0;
                }

                1 << (value - 1)
            }
        }

        impl From<$name> for u8 {
            fn from(src: $name) -> u8 {
                match src {
                    $(
                        $name::$variant => $value,
                    )*
                    }
            }
        }
    };
}
