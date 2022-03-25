use core::marker::PhantomData;
use frame_support::storage::migration;
use frame_support::traits::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use sp_runtime::Percent;

use super::num::NonZeroU32;
use super::pallet::Config;

pub struct EligibleRatioToEligiblityCount<T>(PhantomData<T>);

const PALLET_NAME: &[u8] = b"AuthorSlotFilter";
const OLD_ITEM_NAME: &[u8] = b"EligibleRatio";
const NEW_ITEM_NAME: &[u8] = b"EligibleCount";

impl<T> OnRuntimeUpgrade for EligibleRatioToEligiblityCount<T>
where
	T: Config,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!(target: "EligibleRatioToEligiblityCount", "starting migration");

		if let Some(old_value) =
			migration::get_storage_value::<Percent>(PALLET_NAME, OLD_ITEM_NAME, &[])
		{
			let total_authors = <T as Config>::PotentialAuthors::get().len();
			let new_value: u32 = old_value.mul_ceil(total_authors as u32);
			migration::put_storage_value::<Option<NonZeroU32>>(
				PALLET_NAME,
				NEW_ITEM_NAME,
				&[],
				NonZeroU32::new(new_value),
			);
		}

		let db_weights = T::DbWeight::get();
		db_weights.write + db_weights.read
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<(), &'static str> {
		Ok(())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		Ok(())
	}
}
