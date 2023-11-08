(function() {var implementors = {
"cumulus_test_client":[],
"polkadot_test_client":[],
"sc_basic_authorship":[["impl&lt;A, Block, C, PR&gt; <a class=\"trait\" href=\"sp_consensus/trait.Proposer.html\" title=\"trait sp_consensus::Proposer\">Proposer</a>&lt;Block&gt; for <a class=\"struct\" href=\"sc_basic_authorship/struct.Proposer.html\" title=\"struct sc_basic_authorship::Proposer\">Proposer</a>&lt;Block, C, A, PR&gt;<span class=\"where fmt-newline\">where\n    A: <a class=\"trait\" href=\"sc_transaction_pool_api/trait.TransactionPool.html\" title=\"trait sc_transaction_pool_api::TransactionPool\">TransactionPool</a>&lt;Block = Block&gt; + 'static,\n    Block: <a class=\"trait\" href=\"sp_runtime/traits/trait.Block.html\" title=\"trait sp_runtime::traits::Block\">BlockT</a>,\n    C: HeaderBackend&lt;Block&gt; + ProvideRuntimeApi&lt;Block&gt; + CallApiAt&lt;Block&gt; + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.73.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.73.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a> + 'static,\n    C::Api: ApiExt&lt;Block&gt; + <a class=\"trait\" href=\"sp_block_builder/trait.BlockBuilder.html\" title=\"trait sp_block_builder::BlockBuilder\">BlockBuilderApi</a>&lt;Block&gt;,\n    PR: <a class=\"trait\" href=\"sp_consensus/trait.ProofRecording.html\" title=\"trait sp_consensus::ProofRecording\">ProofRecording</a>,</span>"]]
};if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()