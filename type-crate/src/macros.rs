#[macro_export]
macro_rules! assert_struct_size {
    ($struct: ty, $size: expr) => {
        static_assertions::const_assert_eq!(std::mem::size_of::<$struct>(), $size);
    };
}

#[macro_export]
macro_rules! assert_struct_align {
    ($struct: ty, $align: expr) => {
        static_assertions::const_assert_eq!(std::mem::align_of::<$struct>(), $align);
    };
}

#[macro_export]
macro_rules! bank_seed {
    ($vault_type: expr, $bank_pk: expr) => {
        &[$vault_type.get_seed(), &$bank_pk.to_bytes()] as &[&[u8]]
    };
}

#[macro_export]
macro_rules! bank_authority_seed {
    ($vault_type: expr, $bank_pk: expr) => {
        &[$vault_type.get_authority_seed(), &$bank_pk.to_bytes()] as &[&[u8]]
    };
}
