use super::*;

pub trait SafeMath<T>: Sized {
    fn safe_add(self, rhs: Self) -> Result<Self>;
    fn safe_mul(self, rhs: Self) -> Result<Self>;
    fn safe_div(self, rhs: Self) -> Result<Self>;
    fn safe_sub(self, rhs: Self) -> Result<Self>;
    fn safe_shl(self, offset: T) -> Result<Self>;
    fn safe_shr(self, offset: T) -> Result<Self>;
}

macro_rules! checked_impl {
    ($t:ty, $offset:ty) => {
        impl SafeMath<$offset> for $t {
            #[inline(always)]
            fn safe_add(self, v: $t) -> Result<$t> {
                match self.checked_add(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(StablecoinError::MathError.into())
                    }
                }
            }

            #[inline(always)]
            fn safe_sub(self, v: $t) -> Result<$t> {
                match self.checked_sub(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(StablecoinError::MathError.into())
                    }
                }
            }

            #[inline(always)]
            fn safe_mul(self, v: $t) -> Result<$t> {
                match self.checked_mul(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(StablecoinError::MathError.into())
                    }
                }
            }

            #[inline(always)]
            fn safe_div(self, v: $t) -> Result<$t> {
                match self.checked_div(v) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(StablecoinError::MathError.into())
                    }
                }
            }

            #[inline(always)]
            fn safe_shl(self, v: $offset) -> Result<$t> {
                match self.checked_shl(v.try_into().map_err(|_| StablecoinError::MathError)?) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(StablecoinError::MathError.into())
                    }
                }
            }

            #[inline(always)]
            fn safe_shr(self, v: $offset) -> Result<$t> {
                match self.checked_shr(v.try_into().map_err(|_| StablecoinError::MathError)?) {
                    Some(result) => Ok(result),
                    None => {
                        let caller = Location::caller();
                        msg!("Math error thrown at {}:{}", caller.file(), caller.line());
                        Err(StablecoinError::MathError.into())
                    }
                }
            }
        }
    };
}

checked_impl!(u16, u32);
checked_impl!(i32, u32);
checked_impl!(u32, u32);
checked_impl!(u64, u32);
checked_impl!(i64, u32);
checked_impl!(u128, u32);
checked_impl!(i128, u32);
checked_impl!(usize, u32);