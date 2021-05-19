//! Autogenerated weights for pallet_utility
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! DATE: 2021-05-14, STEPS: [3, ], REPEAT: 2, LOW RANGE: [], HIGH RANGE: []
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev"), DB
//! CACHE: 128

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --execution=wasm
// --pallet=pallet_utility
// --extrinsic=*
// --steps=3
// --repeat=2
// --output=./runtime/src/weights/

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for pallet_utility.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_utility::WeightInfo for WeightInfo<T> {
	fn batch(c: u32) -> Weight {
		(0 as Weight)
			// Standard Error: 1_113_000
			.saturating_add((66_627_000 as Weight).saturating_mul(c as Weight))
	}
	fn as_derivative() -> Weight {
		(62_902_000 as Weight)
	}
	fn batch_all(c: u32) -> Weight {
		(0 as Weight)
			// Standard Error: 1_200_000
			.saturating_add((67_084_000 as Weight).saturating_mul(c as Weight))
	}
}
