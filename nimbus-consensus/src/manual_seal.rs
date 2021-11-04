use std::sync::Arc;
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::{
	traits::{Block as BlockT, DigestFor},
	generic::{Digest, DigestItem},
};
use sp_core::crypto::Public;
use sc_consensus::BlockImportParams;
use sc_consensus_manual_seal::{ConsensusDataProvider, Error};
use sp_api::{TransactionFor, ProvideRuntimeApi, HeaderT};
use sp_inherents::InherentData;
use nimbus_primitives::{NimbusApi, NimbusId, CompatibleDigestItem, NIMBUS_ENGINE_ID};
use cumulus_primitives_parachain_inherent::{ParachainInherentData, INHERENT_IDENTIFIER as PARACHAIN_INHERENT_IDENTIFIER};

/// Provides nimbus-compatible pre-runtime digests for use with manual seal consensus
pub struct NimbusManualSealConsensusDataProvider<C> {
	/// Shared reference to keystore
	pub keystore: SyncCryptoStorePtr,

	/// Shared reference to the client
	pub client: Arc<C>,

	// Could have a skip_prediction field here if it becomes desireable
}

impl<B, C> ConsensusDataProvider<B> for NimbusManualSealConsensusDataProvider<C>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + Send + Sync,
	C::Api: NimbusApi<B, NimbusId>,
	{
	type Transaction = TransactionFor<C, B>;

	fn create_digest(
		&self,
		parent: &B::Header,
		inherents: &InherentData,
	) -> Result<DigestFor<B>, Error> {
		// Retrieve the relay chain block number to use as the slot number from the parachain inherent
		let slot_number = inherents
			.get_data::<ParachainInherentData>(&PARACHAIN_INHERENT_IDENTIFIER)
			.expect("Parachain inherent should decode correctly")
			.expect("Parachain inherent should be present because we are mocking it")
			.validation_data
			.relay_parent_number;

		// Fetch first eligible key from keystore
		let maybe_key = crate::first_eligible_key::<B, C>(
			self.client.clone(),
			&*self.keystore,
			parent,
			// For now we author all blocks in slot zero, which is consistent with  how we are
			// mocking the relay chain height which the runtime uses for slot beacon.
			// This should improve. See https://github.com/PureStake/nimbus/issues/3
			slot_number,
		);

		// If we aren't eligible, return an appropriate error
		match maybe_key {
			Some(key) => {
				Ok(Digest{
					logs: vec![DigestItem::nimbus_pre_digest(NimbusId::from_slice(&key.1))],
				})
			},
			None => {
				Err(Error::StringError(String::from("no nimbus keys available to manual seal")))
			},
		}
		
	}

	// This is where we actually sign with the nimbus key and attach the seal
	fn append_block_import(
		&self,
		_parent: &B::Header,
		params: &mut BlockImportParams<B, Self::Transaction>,
		_inherents: &InherentData,
	) -> Result<(), Error> {

		// We have to reconstruct the type-public pair which is only communicated through the pre-runtime digest
		let claimed_author = params
			.header
			.digest()
			.logs
			.iter()
			.find_map(|digest| {
				match *digest {
					// We do not support the older author inherent in manual seal
					DigestItem::PreRuntime(id, ref author_id) if id == NIMBUS_ENGINE_ID => Some(author_id.clone()),
					_ => None,
				}
			})
			.expect("Expected one pre-runtime digest that contains author id bytes");
		
		let nimbus_public = NimbusId::from_slice(&claimed_author);

		let sig_digest = crate::seal_header::<B>(&params.header, &*self.keystore, &nimbus_public.into());

		params.post_digests.push(sig_digest);

		Ok(())
	}
}
