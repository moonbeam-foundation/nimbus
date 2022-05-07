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

//! This module contains the code necessary to use nimbus in a sovereign
//! (non-parachain) blockchain node. It implements the SlotWorker trait
//! (at least that's the plan for now).
//! 

use std::{pin::Pin, sync::Arc, time::Duration};
use futures::prelude::*;
use sp_api::NumberFor;
use nimbus_primitives::{NimbusApi, NimbusId};
use sc_consensus_slots::{self, BackoffAuthoringBlocksStrategy, SlotWorker, SimpleSlotWorker, SlotProportion, SlotInfo, SlotResult};
use sp_consensus_slots::Slot;
use sp_keystore::SyncCryptoStorePtr;
use sc_consensus::{BlockImport, BlockImportParams};
use sc_telemetry::TelemetryHandle;
use sp_keystore::SyncCryptoStore;
use sp_consensus::{Proposal, BlockOrigin, Environment, SyncOracle, Proposer};
use sp_blockchain::HeaderBackend;
use sc_client_api::BlockOf;
use sp_api::ProvideRuntimeApi;
use sp_api::{BlockT, HeaderT};
use tracing::error;
use crate::{CompatibleDigestItem, LOG_TARGET, first_eligible_key, seal_header};
use sp_application_crypto::ByteArray;


// pub function start_nimbus_standalone(...) -> Result<impl Future<Output = ()>, sp_consensus::Error> {
// 	let worker = ...;

// 	Ok(sc_consensus_slots::start_slot_worker(
// 		slot_duration,
// 		select_chain,
// 		worker,
// 		sync_oracle,
// 		create_inherent_data_providers,
// 		can_author_with,
// 	))
// }

pub struct NimbusStandaloneWorker {

}

#[async_trait::async_trait]
impl<B: BlockT, Proof> SlotWorker<B, Proof> for NimbusStandaloneWorker {
	async fn on_slot(&mut self, slot_info: SlotInfo<B>) -> Option<SlotResult<B, Proof>> {


		// Here's the rough seam between nimnus's simple u32 and Substrate's `struct Slot<u64>`
		let slot = slot_info.slot.into() as u32;
		let parent = &slot_info.chain_head;

		// Call into the runtime to predict eligibility
		//TODO maybe offer a skip prediction feature. Not tackling that yet.
		let maybe_key = first_eligible_key::<B, Client>(
			self.client.clone(),
			&*self.keystore,
			parent,
			slot,
		);

		// Here I'll prototype using the public instead of the type public pair
		// I've had a hunch that this is the correct way to do it for a little while
		let nimbus_id = match maybe_key {
			Some(p) => NimbusId::from_slice(&p.1)
				.map_err(
					|e| error!(target: LOG_TARGET, error = ?e, "Invalid Nimbus ID (wrong length)."),
				)
				.ok()?,
			None => {
				return None;
			}
		};

		// Make the predigest and inherent data

		let inherent_digests = sp_runtime::generic::Digest {
			logs: vec![CompatibleDigestItem::nimbus_pre_digest(nimbus_id)],
		};

		let inherent_data_providers = self
			.create_inherent_data_providers
			.create_inherent_data_providers(
				parent,
				(),
			)
			.await
			.map_err(|e| {
				tracing::error!(
					target: LOG_TARGET,
					error = ?e,
					"Failed to create inherent data providers.",
				)
			})
			.ok()?;

		let inherent_data = inherent_data_providers
			.create_inherent_data()
			.map_err(|e| {
				tracing::error!(
					target: LOG_TARGET,
					error = ?e,
					"Failed to create inherent data.",
				)
			})
			.ok();

		// Author the block
		let proposer_future = self.proposer_factory.lock().init(&parent);

		let proposer = proposer_future
			.await
			.map_err(|e| error!(target: LOG_TARGET, error = ?e, "Could not create proposer."))
			.ok()?;

		let Proposal {
			block,
			storage_changes,
			proof,
		} = proposer
			.propose(
				inherent_data,
				inherent_digests,
				//TODO: Fix this.
				Duration::from_millis(500),
				slot_info.block_size_limit,
			)
			.await
			.map_err(|e| error!(target: LOG_TARGET, error = ?e, "Proposing failed."))
			.ok()?;

		// Sign the block
		let (header, extrinsics) = block.clone().deconstruct();

		let sig_digest = seal_header::<B>(&header, &*self.keystore, &nimbus_id.into());

		let mut block_import_params = BlockImportParams::new(BlockOrigin::Own, header.clone());
		block_import_params.post_digests.push(sig_digest.clone());
		block_import_params.body = Some(extrinsics.clone());
		block_import_params.state_action = sc_consensus::StateAction::ApplyChanges(
			sc_consensus::StorageChanges::Changes(storage_changes),
		);

		// Import our own block
		if let Err(err) = self
			.block_import
			.lock()
			.await
			.import_block(block_import_params, Default::default())
			.await
		{
			error!(
				target: LOG_TARGET,
				at = ?parent.hash(),
				error = ?err,
				"Error importing built block.",
			);

			return None;
		}

		// Return the block WITH the seal for distribution around the network.
		let mut post_header = header.clone();
		post_header.digest_mut().logs.push(sig_digest.clone());
		let post_block = B::new(post_header, extrinsics);

		Some(SlotResult {
			block: post_block,
			proof,
		})
	}
}


// Okay, so actually, I think implementing SlotWorker directly
// will be more straightforward. Let's try that first
/*
/// The SlotWorker implementation for Nimbus standalone chains.
/// This code is responsible running the authoring process on a
/// fixed interval in th)e node service. It is analogous to the
/// Aura or Babe workers.
pub struct NimbusStandaloneWorker<C, E, I, SO, L, BS> {
    // Fields are copied from Aura
    // I removed generic parameter P because our crypto is concrete sr25519
	client: Arc<C>,
	block_import: I,
	env: E,
	keystore: SyncCryptoStorePtr,
	sync_oracle: SO,
	justification_sync_link: L,
	force_authoring: bool,
	backoff_authoring_blocks: Option<BS>,
	block_proposal_slot_portion: SlotProportion,
	max_block_proposal_slot_portion: Option<SlotProportion>,
	telemetry: Option<TelemetryHandle>,
	// _key_type: PhantomData<P>,
}

#[async_trait::async_trait]
impl<B, C, E, I, Error, SO, L, BS> sc_consensus_slots::SimpleSlotWorker<B>
	for NimbusStandaloneWorker<C, E, I, SO, L, BS>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + BlockOf + HeaderBackend<B> + Sync,
	// C::Api: AuraApi<B, AuthorityId<P>>,
    C::Api: NimbusApi<B>,
	E: Environment<B, Error = Error> + Send + Sync,
	E::Proposer: Proposer<B, Error = Error, Transaction = sp_api::TransactionFor<C, B>>,
	I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync + 'static,
	SO: SyncOracle + Send + Clone + Sync,
	L: sc_consensus::JustificationSyncLink<B>,
	BS: BackoffAuthoringBlocksStrategy<NumberFor<B>> + Send + Sync + 'static,
	Error: std::error::Error + Send + From<sp_consensus::Error> + 'static,
{
	type BlockImport = I;
	type SyncOracle = SO;
	type JustificationSyncLink = L;
	type CreateProposer =
		Pin<Box<dyn Future<Output = Result<E::Proposer, sp_consensus::Error>> + Send + 'static>>;
	type Proposer = E::Proposer;
	type Claim = NimbusId;
	// type EpochData = Vec<AuthorityId<P>>;
    type EpochData = (); // I don't think we need this. At least there is no analogue to the finite authority set.

	fn logging_target(&self) -> &'static str {
		"nimbus"
	}

	fn block_import(&mut self) -> &mut Self::BlockImport {
		&mut self.block_import
	}

	fn epoch_data(
		&self,
		header: &B::Header,
		_slot: Slot,
	) -> Result<Self::EpochData, sp_consensus::Error> {
        Ok(())
		// authorities(self.client.as_ref(), &BlockId::Hash(header.hash()))
	}

	fn authorities_len(&self, epoch_data: &Self::EpochData) -> Option<usize> {
		// Some(epoch_data.len())
        None // There is no fixed authority set length in nimbus.
        // IMO, this trait's design assumes more than that the consensus engine is slot-based.
	}

	async fn claim_slot(
		&self,
		_header: &B::Header,
		slot: Slot,
		epoch_data: &Self::EpochData,
	) -> Option<Self::Claim> {
		// let expected_author = slot_author::<P>(slot, epoch_data);
		// expected_author.and_then(|p| {
		// 	if SyncCryptoStore::has_keys(
		// 		&*self.keystore,
		// 		&[(p.to_raw_vec(), sp_application_crypto::key_types::AURA)],
		// 	) {
		// 		Some(p.clone())
		// 	} else {
		// 		None
		// 	}
		// })

        todo!("This is the main function. This is where we call into the runtime to see if we can author. Copy this from the parachain code.")
	}

	fn pre_digest_data(&self, slot: Slot, _claim: &Self::Claim) -> Vec<sp_runtime::DigestItem> {
		// vec![<DigestItem as CompatibleDigestItem<P::Signature>>::aura_pre_digest(slot)]
        todo!("Seems like I'll need something here. We do have a predigest containing the signers public ID. Maybe thisis where I put that info")
	}

	async fn block_import_params(
		&self,
		header: B::Header,
		header_hash: &B::Hash,
		body: Vec<B::Extrinsic>,
		storage_changes: sc_consensus::StorageChanges<<Self::BlockImport as BlockImport<B>>::Transaction, B>,
		public: Self::Claim,
		_epoch: Self::EpochData,
	) -> Result<
		sc_consensus::BlockImportParams<B, <Self::BlockImport as BlockImport<B>>::Transaction>,
		sp_consensus::Error,
	> {
        todo!("I'll probably have to copy this part from the parachain worker too");

		// sign the pre-sealed hash of the block and then
		// add it to a digest item.
		let public_type_pair = public.to_public_crypto_pair();
		let public = public.to_raw_vec();
		let signature = SyncCryptoStore::sign_with(
			&*self.keystore,
			<AuthorityId<P> as AppKey>::ID,
			&public_type_pair,
			header_hash.as_ref(),
		)
		.map_err(|e| sp_consensus::Error::CannotSign(public.clone(), e.to_string()))?
		.ok_or_else(|| {
			sp_consensus::Error::CannotSign(
				public.clone(),
				"Could not find key in keystore.".into(),
			)
		})?;
		let signature = signature
			.clone()
			.try_into()
			.map_err(|_| sp_consensus::Error::InvalidSignature(signature, public))?;

		let signature_digest_item =
			<DigestItem as CompatibleDigestItem<P::Signature>>::aura_seal(signature);

		let mut import_block = BlockImportParams::new(BlockOrigin::Own, header);
		import_block.post_digests.push(signature_digest_item);
		import_block.body = Some(body);
		import_block.state_action =
			sc_consensus::StateAction::ApplyChanges(sc_consensus::StorageChanges::Changes(storage_changes));
		import_block.fork_choice = Some(ForkChoiceStrategy::LongestChain);

		Ok(import_block)
	}

	fn force_authoring(&self) -> bool {
		self.force_authoring
	}

	fn should_backoff(&self, slot: Slot, chain_head: &B::Header) -> bool {
		// Backoff is advanced stuff. We;re not doing that. Let's get it working first.
        false
	}

	fn sync_oracle(&mut self) -> &mut Self::SyncOracle {
		&mut self.sync_oracle
	}

	fn justification_sync_link(&mut self) -> &mut Self::JustificationSyncLink {
		&mut self.justification_sync_link
	}

	fn proposer(&mut self, block: &B::Header) -> Self::CreateProposer {
		self.env
			.init(block)
			.map_err(|e| sp_consensus::Error::ClientImport(format!("{:?}", e)).into())
			.boxed()
	}

	fn telemetry(&self) -> Option<TelemetryHandle> {
		self.telemetry.clone()
	}

	fn proposing_remaining_duration(&self, slot_info: &SlotInfo<B>) -> std::time::Duration {
        // This aura-based method basically works. Unlike Aura, we'll have to call the SlotBeacon
        // in the runtime to know what slot this block is supposed to be. That could be made to work
        // with an additional Runtime API methodthat stores the last slot number. Alternatively, we
        // could add a runtime digest that records the slot number in the block's header once it is
        // calculated from the SlotBeacon during execution. Then we could just look the slot up in the header.
        //
        // But for now, we aren't doing any of that. Let's just hardcode a number. That means the timeout
        // feature will be effectively disabled because there is effectively always 6 seconds left.

        std::time::Duration::from_secs(6)

		// let parent_slot = find_pre_digest::<B, P::Signature>(&slot_info.chain_head).ok();

		// sc_consensus_slots::proposing_remaining_duration(
		// 	parent_slot,
		// 	slot_info,
		// 	&self.block_proposal_slot_portion,
		// 	self.max_block_proposal_slot_portion.as_ref(),
		// 	sc_consensus_slots::SlotLenienceType::Exponential,
		// 	self.logging_target(),
		// )
	}
}
*/