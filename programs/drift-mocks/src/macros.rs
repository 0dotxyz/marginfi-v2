
#[macro_export]
macro_rules! math_error {
    () => {{
        || {
            let error_code = $crate::DriftMocksError::ScalingOverflow;
            anchor_lang::prelude::msg!(
                "Error \"{}\" thrown at {}:{}",
                error_code,
                file!(),
                line!()
            );
            error_code
        }
    }};
}
