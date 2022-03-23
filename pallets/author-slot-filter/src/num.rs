use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, TypeInfo, Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonZeroU32(u32);

impl core::ops::Deref for NonZeroU32 {
	type Target = u32;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl parity_scale_codec::EncodeLike<u32> for NonZeroU32 {}

impl NonZeroU32 {
	/// Creates a new `Some(NonZeroU32)` instance if value is 0, `None` otherwise.
	#[inline]
	pub const fn new(n: u32) -> Option<Self> {
		if n != 0 {
			Some(Self(n))
		} else {
			None
		}
	}

	/// new_unchecked created a `NonZeroU32` where the user MUST guarantee
	/// that the value is nonzero.
	#[inline]
	pub const fn new_unchecked(n: u32) -> Option<Self> {
		Some(Self(n))
	}

	pub fn get(self) -> u32 {
		self.0
	}
}

impl Serialize for NonZeroU32 {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		self.clone().get().serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for NonZeroU32 {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let value = Deserialize::deserialize(deserializer)?;
		match <NonZeroU32>::new(value) {
			Some(nonzero) => Ok(nonzero),
			None => Err(Error::custom("expected a non-zero value")),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_returns_none_if_zero() {
		assert_eq!(None, NonZeroU32::new(0));
	}

	#[test]
	fn test_new_returns_some_nonzerou32_if_nonzero() {
		let n = 10;
		let expected = NonZeroU32::new_unchecked(n);
		let actual = NonZeroU32::new(n);
		assert_eq!(expected, actual);
		assert_eq!(n, actual.unwrap().get());
	}
}
