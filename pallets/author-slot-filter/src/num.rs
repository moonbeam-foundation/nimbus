use parity_scale_codec::{Decode, Encode, Error, Input};
use scale_info::TypeInfo;
use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, TypeInfo, Encode, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

	/// new_unchecked creats a `NonZeroU32` where the user MUST guarantee
	/// that the value is nonzero.
	#[inline]
	pub const fn new_unchecked(n: u32) -> Self {
		Self(n)
	}

	/// Returns the the underlying number
	pub fn get(&self) -> u32 {
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
		match NonZeroU32::new(value) {
			Some(nonzero) => Ok(nonzero),
			None => Err(DeserializeError::custom("expected a non-zero value")),
		}
	}
}

impl Decode for NonZeroU32 {
	fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
		Self::new(Decode::decode(input)?)
			.ok_or_else(|| Error::from("cannot create non-zero number from 0"))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use parity_scale_codec::Encode;

	#[test]
	fn test_new_returns_none_if_zero() {
		assert_eq!(None, NonZeroU32::new(0));
	}

	#[test]
	fn test_new_returns_some_if_nonzero() {
		let n = 10;
		let expected = Some(NonZeroU32::new_unchecked(n));

		let actual = NonZeroU32::new(n);
		assert_eq!(expected, actual);
		assert_eq!(n, actual.unwrap().get());
	}

	#[test]
	fn test_decode_errors_if_zero_value() {
		let buf: Vec<u8> = 0u32.encode();
		let result = NonZeroU32::decode(&mut &buf[..]);
		assert!(result.is_err(), "expected error, got {:?}", result);
	}

	#[test]
	fn test_decode_succeeds_if_nonzero_value() {
		let buf: Vec<u8> = 1u32.encode();

		let result = NonZeroU32::decode(&mut &buf[..]);
		assert!(result.is_ok(), "unexpected error, got {:?}", result);
		assert_eq!(Ok(NonZeroU32::new_unchecked(1)), result);
	}
}
