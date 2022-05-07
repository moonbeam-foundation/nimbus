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
use nimbus_primitives::{NimbusApi, NimbusId};
use parking_lot::Mutex;
use sc_consensus::{BlockImport, BlockImportParams};
use sc_consensus_slots::{self, SlotInfo, SlotResult, SlotWorker};
use sp_api::ProvideRuntimeApi;
use sp_api::{BlockT, HeaderT};
use sp_application_crypto::ByteArray;
use sp_consensus::{BlockOrigin, Environment, Proposal, Proposer};
use sp_inherents::CreateInherentDataProviders;
use sp_inherents::InherentDataProvider;
use sp_keystore::SyncCryptoStorePtr;
use std::{marker::PhantomData, sync::Arc, time::Duration};
use tracing::error;

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
pub struct NimbusStandaloneWorker<B, C, PF, BI, CIDP> {
	client: Arc<C>,
	block_import: BI,
	proposer_factory: Arc<Mutex<PF>>,
	keystore: SyncCryptoStorePtr,
	// TODO, why does AuraWorker not have CIDP?
	create_inherent_data_providers: Arc<CIDP>,
	_phantom: PhantomData<B>,
}

#[async_trait::async_trait]
impl<B, C, PF, BI, CIDP> SlotWorker<B, <<PF as Environment<B>>::Proposer as Proposer<B>>::Proof>
	for NimbusStandaloneWorker<B, C, PF, BI, CIDP>
where
	B: BlockT,
	BI: BlockImport<B>,
	C: ProvideRuntimeApi<B>,
	C::Api: NimbusApi<B>,
	PF: Environment<B> + Send + Sync + 'static,
	PF::Proposer: Proposer<B, Transaction = BI::Transaction>,
	CIDP: CreateInherentDataProviders<B, ()>,
{
	async fn on_slot(
		&mut self,
		slot_info: SlotInfo<B>,
	) -> Option<SlotResult<B, <<PF as Environment<B>>::Proposer as Proposer<B>>::Proof>> {
		//TODO should we consult a SyncOracle and not author if we're syncing?

		// Here's the rough seam between nimnus's simple u32 and Substrate's `struct Slot<u64>`
		let slot = slot_info.slot.into() as u32;
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

		// Make the predigest and inherent data

		let inherent_digests = sp_runtime::generic::Digest {
			logs: vec![CompatibleDigestItem::nimbus_pre_digest(nimbus_id)],
		};

		let inherent_data_providers = self
			.create_inherent_data_providers
			.create_inherent_data_providers(parent.hash(), ())
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
			.ok()?;

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
