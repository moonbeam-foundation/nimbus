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

		if let Some(old_value) =
			migration::get_storage_value::<Percent>(PALLET_NAME, ELIGIBLE_RATIO_ITEM_NAME, &[])
		{
			let total_authors = <T as Config>::PotentialAuthors::get().len();
			let new_value: u32 = percent_of_num(old_value, total_authors as u32);
			migration::put_storage_value(
				PALLET_NAME,
				ELIGIBLE_COUNT_ITEM_NAME,
				&[],
				NonZeroU32::new(new_value).unwrap_or(crate::pallet::DEFAULT_TOTAL_ELIGIBLE_AUTHORS),
			);

			let db_weights = T::DbWeight::get();
			db_weights.write + db_weights.read
		} else {
			0
		}
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<(), &'static str> {
		if let Some(eligible_ratio) =
			migration::get_storage_value::<Percent>(PALLET_NAME, ELIGIBLE_RATIO_ITEM_NAME, &[])
		{
			let total_authors = <T as Config>::PotentialAuthors::get().len();
			let eligible_count: u32 = percent_of_num(eligible_ratio, total_authors as u32);
			let eligible_count = NonZeroU32::new_unchecked(eligible_count);
			Self::set_temp_storage(new_value, "expected_eligible_count");
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		let expected = Self::get_temp_storage::<NonZeroU32>("expected_eligible_count");
		let actual =
			migration::get_storage_value::<NonZeroU32>(PALLET_NAME, ELIGIBLE_COUNT_ITEM_NAME, &[]);

		assert_eq!(expected, actual);
	}
}

fn percent_of_num(percent: Percent, num: u32) -> u32 {
	percent.mul_ceil(num as u32)
}
