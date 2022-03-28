use super::*;
use crate::mock::*;

use frame_support::assert_ok;
use frame_support::migration::put_storage_value;
use sp_runtime::Percent;

#[test]
fn test_set_eligibility_works() {
	new_test_ext().execute_with(|| {
		let value = num::NonZeroU32::new(34);

		assert_ok!(AuthorSlotFilter::set_eligible(
			Origin::root(),
			value.clone()
		));
		assert_eq!(AuthorSlotFilter::eligible_count(), value)
	});
}

#[test]
fn test_migration_works_for_converting_existing_eligible_ratio_to_eligible_count() {
	use crate::num::NonZeroU32;
	use frame_support::traits::OnRuntimeUpgrade;

	new_test_ext().execute_with(|| {
		let input_eligible_ratio = Percent::from_percent(50);
		let total_author_count = mock::Authors::get().len();
		let eligible_author_count =
			input_eligible_ratio.clone().mul_ceil(total_author_count) as u32;
		let expected_eligible_count = NonZeroU32::new(eligible_author_count);
		let expected_weight = TestDbWeight::get().write + TestDbWeight::get().read;

		put_storage_value(
			migration::PALLET_NAME,
			migration::ELIGIBLE_RATIO_ITEM_NAME,
			&[],
			input_eligible_ratio.clone(),
		);

		let actual_weight = migration::EligibleRatioToEligiblityCount::<Test>::on_runtime_upgrade();
		assert_eq!(expected_weight, actual_weight);

		let actual_eligible_ratio_after = AuthorSlotFilter::eligible_ratio();
		let actual_eligible_count = AuthorSlotFilter::eligible_count();
		assert_eq!(expected_eligible_count, actual_eligible_count);
		assert_eq!(input_eligible_ratio, actual_eligible_ratio_after);
	});
}

#[test]
fn test_migration_skips_converting_missing_eligible_ratio_to_eligible_count_and_returns_default_value(
) {
	use frame_support::traits::OnRuntimeUpgrade;

	new_test_ext().execute_with(|| {
		let expected_default_eligible_count = DEFAULT_TOTAL_ELIGIBLE_AUTHORS;
		let expected_weight = 0;

		let actual_weight = migration::EligibleRatioToEligiblityCount::<Test>::on_runtime_upgrade();
		assert_eq!(expected_weight, actual_weight);

		let actual_eligible_count = AuthorSlotFilter::eligible_count();
		assert_eq!(expected_default_eligible_count, actual_eligible_count);
	});
}
