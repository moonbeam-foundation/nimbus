use std::sync::Arc;
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::{
	traits::{Block as BlockT, DigestFor},
	generic::{Digest, DigestItem},
};
use sp_core::crypto::Public;
use sc_consensus::BlockImportParams;
use sc_consensus_manual_seal::{ConsensusDataProvider, Error};
use sp_api::{TransactionFor, ProvideRuntimeApi};
use sp_inherents::InherentData;
use nimbus_primitives::{NimbusApi, NimbusId, CompatibleDigestItem};

//TODO Do I need the generic B? I copied it from Babe impl in Substrate.
/// Provides nimbus-compatible pre-runtime digests for use with manual seal consensus
pub struct NimbusManualSealConsensusDataProvider</*B: BlockT,*/ C> {
	/// Shared reference to keystore
	pub keystore: SyncCryptoStorePtr,

	/// Shared reference to the client
	pub client: Arc<C>,

	// Could have a skip_prediction field here if it becomes desireable
}

impl<B, C> ConsensusDataProvider<B> for NimbusManualSealConsensusDataProvider</*B,*/ C>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + Send + Sync,
	C::Api: NimbusApi<B, NimbusId>,
	{
	type Transaction = TransactionFor<C, B>;

	fn create_digest(
		&self,
		parent: &B::Header,
		_inherents: &InherentData,
	) -> Result<DigestFor<B>, Error> {
		// Fetch first eligible key from keystore
		let maybe_key = crate::first_eligible_key::<B, C>(
			self.client.clone(),
			&*self.keystore,
			parent,
		0, //TODO Come up with some real slot number. Maybe just use our own block height
		);

		// If we aren't eligible, return an appropriate error
		match maybe_key {
			Some(key) => {
				println!("Found an eligible key: {:?}", key);
				Ok(Digest{
					logs: vec![DigestItem::nimbus_pre_digest(NimbusId::from_slice(&key.1))],
				})
			},
			None => {
				todo!("Grrr how to return this fucking error!");
				// Err(Error::Other(Box::new(String::from("no nimbus keys available to manual seal"))))
			},
		}
		
	}

	// IDK WTF this is for yet. Maybe we won't need it :)
	fn append_block_import(
		&self,
		_parent: &B::Header,
		_params: &mut BlockImportParams<B, Self::Transaction>,
		_inherents: &InherentData,
	) -> Result<(), Error> {
		todo!("inside append block import. I guess we at least need something here.")
	}
}
