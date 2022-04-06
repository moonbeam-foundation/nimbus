// Copyright 2019-2021 PureStake Inc.
// This file is part of Nimbus.

// Nimbus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Nimbus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Nimbus.  If not, see <http://www.gnu.org/licenses/>.

use core::marker::PhantomData;
use frame_support::storage::migration;
use frame_support::traits::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use sp_runtime::Percent;

#[cfg(feature = "try-runtime")]
use frame_support::traits::OnRuntimeUpgradeHelpersExt;

use super::num::NonZeroU32;
use super::pallet::Config;
use super::pallet::EligibilityValue;

pub struct EligibleRatioToEligiblityCount<T>(PhantomData<T>);

pub const PALLET_NAME: &[u8] = b"AuthorSlotFilter";
pub const ELIGIBLE_RATIO_ITEM_NAME: &[u8] = b"EligibleRatio";
pub const ELIGIBLE_COUNT_ITEM_NAME: &[u8] = b"EligibleCount";

impl<T> OnRuntimeUpgrade for EligibleRatioToEligiblityCount<T>
where
	T: Config,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!(target: "EligibleRatioToEligiblityCount", "starting migration");

		let old_value =
			migration::get_storage_value::<Percent>(PALLET_NAME, ELIGIBLE_RATIO_ITEM_NAME, &[]);

		let new_value = old_value
			.and_then(|value| {
				let total_authors = <T as Config>::PotentialAuthors::get().len();
				let new_value = percent_of_num(value, total_authors as u32);
				NonZeroU32::new(new_value)
			})
			.unwrap_or(EligibilityValue::default());

		let db_weights = T::DbWeight::get();
		migration::put_storage_value(PALLET_NAME, ELIGIBLE_COUNT_ITEM_NAME, &[], new_value);
		db_weights.write + db_weights.read
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<(), &'static str> {
		let old_value =
			migration::get_storage_value::<Percent>(PALLET_NAME, ELIGIBLE_RATIO_ITEM_NAME, &[]);

		let expected_value = old_value
			.and_then(|value| {
				let total_authors = <T as Config>::PotentialAuthors::get().len();
				let eligible_count: u32 = percent_of_num(value, total_authors as u32);
				NonZeroU32::new(eligible_count)
			})
			.unwrap_or(EligibilityValue::default());

		Self::set_temp_storage(expected_value, "expected_eligible_count");

		Ok(())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		let expected = Self::get_temp_storage::<NonZeroU32>("expected_eligible_count");
		let actual =
			migration::get_storage_value::<NonZeroU32>(PALLET_NAME, ELIGIBLE_COUNT_ITEM_NAME, &[]);

		assert_eq!(expected, actual);

		Ok(())
	}
}

fn percent_of_num(percent: Percent, num: u32) -> u32 {
	percent.mul_ceil(num as u32)
}

#[cfg(test)]
mod tests {
	use super::percent_of_num;
	use super::*;

	#[test]
	fn test_percent_of_num_ceils_value() {
		let fifty_percent = Percent::from_float(0.5);

		let actual = percent_of_num(fifty_percent, 5);
		assert_eq!(3, actual);

		let actual = percent_of_num(fifty_percent, 20);
		assert_eq!(10, actual);
	}
}
