// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

use crate::{
	configuration::TestAuthorities,
	dummy_builder,
	environment::{TestEnvironment, TestEnvironmentDependencies, GENESIS_HASH},
	mock::{
		chain_api::{ChainApiState, MockChainApi},
		network_bridge::{MockNetworkBridgeRx, MockNetworkBridgeTx},
		prospective_parachains::MockProspectiveParachains,
		runtime_api::{MockRuntimeApi, MockRuntimeApiCoreState},
		AlwaysSupportsParachains,
	},
	network::new_network,
	usage::BenchmarkUsage,
	NODE_UNDER_TEST,
};
use colored::Colorize;
use itertools::Itertools;
use polkadot_node_metrics::metrics::Metrics;
use polkadot_node_network_protocol::{
	grid_topology::{SessionGridTopology, TopologyPeerInfo},
	request_response::{IncomingRequest, ReqProtocolNames},
	view, View,
};
use polkadot_node_primitives::{SignedFullStatementWithPVD, Statement};
use polkadot_node_subsystem::messages::{
	network_bridge_event::NewGossipTopology, AllMessages, NetworkBridgeEvent,
	StatementDistributionMessage,
};
use polkadot_overseer::{
	Handle as OverseerHandle, Overseer, OverseerConnector, OverseerMetrics, SpawnGlue,
};
use polkadot_primitives::{
	AuthorityDiscoveryId, Block, Hash, SigningContext, ValidatorId, ValidatorIndex,
};
use polkadot_statement_distribution::StatementDistributionSubsystem;
use rand::SeedableRng;
use sc_keystore::LocalKeystore;
use sc_network::request_responses::ProtocolConfig;
use sc_network_types::PeerId;
use sc_service::SpawnTaskHandle;
use sp_core::Pair;
use sp_keystore::{Keystore, KeystorePtr};
use sp_runtime::RuntimeAppPublic;
use std::{
	sync::{atomic::Ordering, Arc},
	time::{Duration, Instant},
};
pub use test_state::TestState;
use tokio::time::sleep;

mod test_state;

const LOG_TARGET: &str = "subsystem-bench::statement";

pub fn make_keystore() -> KeystorePtr {
	let keystore: KeystorePtr = Arc::new(LocalKeystore::in_memory());
	Keystore::sr25519_generate_new(&*keystore, ValidatorId::ID, Some("//Node0"))
		.expect("Insert key into keystore");
	Keystore::sr25519_generate_new(&*keystore, AuthorityDiscoveryId::ID, Some("//Node0"))
		.expect("Insert key into keystore");
	keystore
}

fn build_overseer(
	state: &TestState,
	network_bridge: (MockNetworkBridgeTx, MockNetworkBridgeRx),
	dependencies: &TestEnvironmentDependencies,
) -> (
	Overseer<SpawnGlue<SpawnTaskHandle>, AlwaysSupportsParachains>,
	OverseerHandle,
	Vec<ProtocolConfig>,
) {
	let overseer_connector = OverseerConnector::with_event_capacity(64000);
	let overseer_metrics = OverseerMetrics::try_register(&dependencies.registry).unwrap();
	let spawn_task_handle = dependencies.task_manager.spawn_handle();
	let mock_runtime_api = MockRuntimeApi::new(
		state.config.clone(),
		state.test_authorities.clone(),
		state.candidate_receipts.clone(),
		Default::default(),
		Default::default(),
		0,
		MockRuntimeApiCoreState::Scheduled,
	);
	let chain_api_state = ChainApiState { block_headers: state.block_headers.clone() };
	let mock_chain_api = MockChainApi::new(chain_api_state);
	let mock_prospective_parachains = MockProspectiveParachains::new();
	let (statement_req_receiver, statement_req_cfg) = IncomingRequest::get_config_receiver::<
		Block,
		sc_network::NetworkWorker<Block, Hash>,
	>(&ReqProtocolNames::new(GENESIS_HASH, None));
	let (candidate_req_receiver, candidate_req_cfg) = IncomingRequest::get_config_receiver::<
		Block,
		sc_network::NetworkWorker<Block, Hash>,
	>(&ReqProtocolNames::new(GENESIS_HASH, None));
	let keystore = make_keystore();
	let subsystem = StatementDistributionSubsystem::new(
		keystore.clone(),
		statement_req_receiver,
		candidate_req_receiver,
		Metrics::try_register(&dependencies.registry).unwrap(),
		rand::rngs::StdRng::from_entropy(),
	);
	let dummy = dummy_builder!(spawn_task_handle, overseer_metrics)
		.replace_runtime_api(|_| mock_runtime_api)
		.replace_chain_api(|_| mock_chain_api)
		.replace_prospective_parachains(|_| mock_prospective_parachains)
		.replace_statement_distribution(|_| subsystem)
		.replace_network_bridge_tx(|_| network_bridge.0)
		.replace_network_bridge_rx(|_| network_bridge.1);
	let (overseer, raw_handle) =
		dummy.build_with_connector(overseer_connector).expect("Should not fail");
	let overseer_handle = OverseerHandle::new(raw_handle);

	(overseer, overseer_handle, vec![statement_req_cfg, candidate_req_cfg])
}

pub fn prepare_test(
	state: &TestState,
	with_prometheus_endpoint: bool,
) -> (TestEnvironment, Vec<ProtocolConfig>) {
	let dependencies = TestEnvironmentDependencies::default();
	let (network, network_interface, network_receiver) = new_network(
		&state.config,
		&dependencies,
		&state.test_authorities,
		vec![Arc::new(state.clone())],
	);
	let network_bridge_tx = MockNetworkBridgeTx::new(
		network.clone(),
		network_interface.subsystem_sender(),
		state.test_authorities.clone(),
	);
	let network_bridge_rx = MockNetworkBridgeRx::new(network_receiver, None);
	let (overseer, overseer_handle, cfg) =
		build_overseer(state, (network_bridge_tx, network_bridge_rx), &dependencies);

	(
		TestEnvironment::new(
			dependencies,
			state.config.clone(),
			network,
			overseer,
			overseer_handle,
			state.test_authorities.clone(),
			with_prometheus_endpoint,
		),
		cfg,
	)
}

pub fn generate_peer_view_change(block_hash: Hash, peer_id: PeerId) -> AllMessages {
	let network = NetworkBridgeEvent::PeerViewChange(peer_id, View::new([block_hash], 0));

	AllMessages::StatementDistribution(StatementDistributionMessage::NetworkBridgeUpdate(network))
}

pub fn generate_new_session_topology(
	test_authorities: &TestAuthorities,
	test_node: ValidatorIndex,
) -> Vec<AllMessages> {
	let topology = generate_topology(test_authorities);

	let event = NetworkBridgeEvent::NewGossipTopology(NewGossipTopology {
		session: 0,
		topology,
		local_index: Some(test_node),
	});
	vec![AllMessages::StatementDistribution(StatementDistributionMessage::NetworkBridgeUpdate(
		event,
	))]
}

/// Generates a topology to be used for this benchmark.
pub fn generate_topology(test_authorities: &TestAuthorities) -> SessionGridTopology {
	let keyrings = test_authorities
		.validator_authority_id
		.clone()
		.into_iter()
		.zip(test_authorities.peer_ids.clone())
		.collect_vec();

	let topology = keyrings
		.clone()
		.into_iter()
		.enumerate()
		.map(|(index, (discovery_id, peer_id))| TopologyPeerInfo {
			peer_ids: vec![peer_id],
			validator_index: ValidatorIndex(index as u32),
			discovery_id,
		})
		.collect_vec();
	let shuffled = (0..keyrings.len()).collect_vec();

	SessionGridTopology::new(shuffled, topology)
}

pub async fn benchmark_statement_distribution(
	benchmark_name: &str,
	env: &mut TestEnvironment,
	state: &TestState,
) -> BenchmarkUsage {
	let config = env.config().clone();

	env.metrics().set_n_validators(config.n_validators);
	env.metrics().set_n_cores(config.n_cores);

	// First create the initialization messages that make sure that then node under
	// tests receives notifications about the topology used and the connected peers.
	let mut initialization_messages =
		env.network().generate_statement_distribution_peer_connected();
	initialization_messages.extend(generate_new_session_topology(
		&state.test_authorities,
		ValidatorIndex(NODE_UNDER_TEST),
	));
	for message in initialization_messages {
		env.send_message(message).await;
	}
	let pair = state.validator_pairs.get(NODE_UNDER_TEST as usize).unwrap();

	let test_start = Instant::now();
	for block_info in state.block_infos.iter() {
		let block_num = block_info.number as usize;
		gum::info!(target: LOG_TARGET, "Current block {}/{} {}", block_num, config.num_blocks, block_info.hash);
		env.metrics().set_current_block(block_num);
		env.import_block(block_info.clone()).await;

		for update in env
			.network()
			.generate_statement_distribution_peer_view_change(view![block_info.hash])
		{
			env.send_message(update).await;
		}

		let receipt = state
			.commited_candidate_receipts
			.get(&block_info.hash)
			.expect("Pregenerated")
			.clone();
		let candidate_hash = receipt.hash();
		let statement = Statement::Seconded(receipt.clone());
		let context = SigningContext { parent_hash: block_info.hash, session_index: 0 };
		let payload = statement.to_compact().signing_payload(&context);
		let signature = pair.sign(&payload[..]);
		let message = AllMessages::StatementDistribution(StatementDistributionMessage::Share(
			block_info.hash,
			SignedFullStatementWithPVD::new(
				statement.supply_pvd(state.persisted_validation_data.clone()),
				ValidatorIndex(0),
				signature,
				&context,
				&pair.public(),
			)
			.unwrap(),
		));
		env.send_message(message).await;

		loop {
			let seconded_count = state
				.seconded_count
				.get(&candidate_hash)
				.expect("Pregenerated")
				.load(Ordering::SeqCst);
			gum::info!(target: LOG_TARGET, seconded_count = ?seconded_count);
			if seconded_count < 4 {
				sleep(Duration::from_millis(50)).await;
			} else {
				break;
			}
		}

		loop {
			let statements_count = state
				.statements_count
				.get(&candidate_hash)
				.expect("Pregenerated")
				.load(Ordering::SeqCst);
			gum::info!(target: LOG_TARGET, statements_count = ?statements_count);
			if statements_count < 9 {
				sleep(Duration::from_millis(50)).await;
			} else {
				break;
			}
		}

		let message = AllMessages::StatementDistribution(StatementDistributionMessage::Backed(
			candidate_hash,
		));
		env.send_message(message).await;

		loop {
			let known_count = state
				.known_count
				.get(&candidate_hash)
				.expect("Pregenerated")
				.load(Ordering::SeqCst);
			gum::info!(target: LOG_TARGET, known_count = ?known_count);
			if known_count < 16 {
				sleep(Duration::from_millis(50)).await;
			} else {
				break;
			}
		}
	}

	let duration: u128 = test_start.elapsed().as_millis();
	gum::info!(target: LOG_TARGET, "All blocks processed in {}", format!("{:?}ms", duration).cyan());
	gum::info!(target: LOG_TARGET,
		"Avg block time: {}",
		format!("{} ms", test_start.elapsed().as_millis() / env.config().num_blocks as u128).red()
	);

	env.stop().await;
	env.collect_resource_usage(benchmark_name, &["statement-distribution"])
}
