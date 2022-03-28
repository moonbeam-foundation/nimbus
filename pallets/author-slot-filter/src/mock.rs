use crate as pallet_testing;
use frame_support::parameter_types;
use frame_support::sp_io;
use frame_support::weights::RuntimeDbWeight;
use frame_support_test::TestRandomness;
use frame_system;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		AuthorSlotFilter: pallet_testing::{Pallet, Call, Storage, Event},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub Authors: Vec<u64> = vec![1, 2, 3, 4, 5];
	pub const TestDbWeight: RuntimeDbWeight = RuntimeDbWeight {
		read: 1,
		write: 10,
	};
}

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = TestDbWeight;
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

impl pallet_testing::Config for Test {
	type Event = Event;
	type RandomnessSource = TestRandomness<Self>;
	type PotentialAuthors = Authors;
	type WeightInfo = ();
}

/// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}
