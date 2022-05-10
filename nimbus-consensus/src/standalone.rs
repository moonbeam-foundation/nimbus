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
//! (non-parachain) blockchain node. It implements the SlotWorker trait.

use crate::{first_eligible_key, seal_header, CompatibleDigestItem, LOG_TARGET};
use futures::Future;
use nimbus_primitives::{AuthorFilterAPI, NimbusApi, NimbusId};
use sc_consensus::{BlockImport, BlockImportParams, ForkChoiceStrategy};
use sc_consensus_slots::InherentDataProviderExt;
use sc_consensus_slots::{self, SlotInfo, SlotResult, SlotWorker};
use sp_api::ProvideRuntimeApi;
use sp_api::{BlockT, HeaderT};
use sp_application_crypto::ByteArray;
use sp_consensus::CanAuthorWith;
use sp_consensus::SelectChain;
use sp_consensus::SyncOracle;
use sp_consensus::{BlockOrigin, Environment, Proposal, Proposer};
use sp_consensus_slots::SlotDuration;
use sp_inherents::CreateInherentDataProviders;
use sp_keystore::SyncCryptoStorePtr;
use std::{marker::PhantomData, sync::Arc, time::Duration};
use tracing::error;

/// Start the nimbus standalone worker. The returned future should be run in a futures executor.
pub fn start_nimbus_standalone<B, C, SC, BI, PF, CIDP, SO, CAW, Error>(
	client: Arc<C>,
	select_chain: SC,
	block_import: BI,
	proposer_factory: PF,
	keystore: SyncCryptoStorePtr,
	sync_oracle: SO,
	can_author_with: CAW,
	create_inherent_data_providers: CIDP,
) -> Result<impl Future<Output = ()>, sp_consensus::Error>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + Send + Sync,
	C::Api: NimbusApi<B>,
	C::Api: AuthorFilterAPI<B, NimbusId>, // Grrrrr. Remove this after https://github.com/PureStake/nimbus/pull/30 lands
	SC: SelectChain<B>,
	BI: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync + 'static,
	PF: Environment<B, Error = Error> + Send + Sync + 'static,
	PF::Proposer: Proposer<B, Error = Error, Transaction = sp_api::TransactionFor<C, B>>,
	CIDP: CreateInherentDataProviders<B, ()> + Send,
	CIDP::InherentDataProviders: InherentDataProviderExt + Send,
	SO: SyncOracle + Send + Sync + Clone,
	CAW: CanAuthorWith<B> + Send,
	Error: std::error::Error + Send + From<sp_consensus::Error> + 'static,
{
	// TODO This should match what the runtime expects.
	// To enforce that we'll need a runtime api. Or maybe we can leave it up to the node operator...
	// In aura they get it from the genesis
	let slot_duration = SlotDuration::from_millis(6000);

	let worker = NimbusStandaloneWorker {
		client: client.clone(),
		block_import,
		proposer_factory,
		keystore,
		_phantom: PhantomData::<B>,
	};

	Ok(sc_consensus_slots::start_slot_worker(
		slot_duration,
		select_chain,
		worker,
		sync_oracle,
		create_inherent_data_providers,
		can_author_with,
	))
}

pub struct NimbusStandaloneWorker<B, C, PF, BI> {
	client: Arc<C>,
	block_import: BI,
	proposer_factory: PF,
	keystore: SyncCryptoStorePtr,
	_phantom: PhantomData<B>,
}

#[async_trait::async_trait]
impl<B, C, PF, BI> SlotWorker<B, <<PF as Environment<B>>::Proposer as Proposer<B>>::Proof>
	for NimbusStandaloneWorker<B, C, PF, BI>
where
	B: BlockT,
	BI: BlockImport<B> + Send + Sync + 'static,
	C: ProvideRuntimeApi<B> + Send + Sync,
	C::Api: NimbusApi<B>,
	C::Api: AuthorFilterAPI<B, NimbusId>, // Grrrrr. Remove this after https://github.com/PureStake/nimbus/pull/30 lands
	PF: Environment<B> + Send + Sync + 'static,
	PF::Proposer: Proposer<B, Transaction = BI::Transaction>,
{
	async fn on_slot(
		&mut self,
		slot_info: SlotInfo<B>,
	) -> Option<SlotResult<B, <<PF as Environment<B>>::Proposer as Proposer<B>>::Proof>> {
		// Here's the rough seam between nimnus's simple u32 and Substrate's `struct Slot<u64>`
		let slot: u32 = {
			let slot_u64: u64 = slot_info.slot.into();
			slot_u64 as u32
		};
		let parent = &slot_info.chain_head;

		// Call into the runtime to predict eligibility
		//TODO maybe offer a skip prediction feature. Not tackling that yet.
		let maybe_key =
			first_eligible_key::<B, C>(self.client.clone(), &*self.keystore, parent, slot);

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

		// Make the predigest (sc-consensus-slots tackles the inherent data)
		let inherent_digests = sp_runtime::generic::Digest {
			logs: vec![CompatibleDigestItem::nimbus_pre_digest(nimbus_id.clone())],
		};

		// Author the block
		let proposer_future = self.proposer_factory.init(&parent);

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
				slot_info.inherent_data,
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
		block_import_params.fork_choice = Some(ForkChoiceStrategy::LongestChain);

		// Import our own block
		if let Err(err) = self
			.block_import
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
			storage_proof: proof,
		})
	}
}
