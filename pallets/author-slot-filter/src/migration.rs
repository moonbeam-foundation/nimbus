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

const PALLET_NAME: &[u8] = b"AuthorSlotFilter";
const ELIGIBLE_RATIO_ITEM_NAME: &[u8] = b"EligibleRatio";
const ELIGIBLE_COUNT_ITEM_NAME: &[u8] = b"EligibleCount";

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
			let new_value: u32 = old_value.mul_ceil(total_authors as u32);
			migration::put_storage_value::<Option<NonZeroU32>>(
				PALLET_NAME,
				ELIGIBLE_COUNT_ITEM_NAME,
				&[],
				NonZeroU32::new(new_value),
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
			let eligible_count: u32 = eligible_ratio.mul_ceil(total_authors as u32);
			let eligible_count = NonZeroU32::new(eligible_count);
			Self::set_temp_storage(new_value, "expected_eligible_count");
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		let expected = Self::get_temp_storage::<NonZeroU32>("expected_eligible_count");
		let actual = migration::get_storage_value::<Option<NonZeroU32>>(
			PALLET_NAME,
			ELIGIBLE_COUNT_ITEM_NAME,
			&[],
		);

		assert_eq!(expected, actual);
	}
}
