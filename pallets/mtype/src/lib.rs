// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! adding #MARK Types.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

pub mod mtypes;
pub mod weights;

#[cfg(any(feature = "mock", test))]
// pub mod mock;
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

/// Test module for MTYPEs
#[cfg(test)]
mod tests;

pub use crate::mtypes::*;
pub use pallet::*;
pub mod utils;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a MTYPE hash.
	pub type MtypeHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a MTYPE owner.
	pub type MtypeOwnerOf<T> = <T as Config>::CordAccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// CID Information
	pub type CidOf = Vec<u8>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type CordAccountId: Parameter + Default;
		type EnsureOrigin: EnsureOrigin<Success = MtypeOwnerOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// MTYPEs stored on chain.
	/// It maps from a MTYPE hash to its owner.
	#[pallet::storage]
	#[pallet::getter(fn mtypes)]
	pub type Mtypes<T> = StorageMap<_, Blake2_128Concat, MtypeHashOf<T>, MTypeDetails<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new mtype has been created.
		/// \[owner identifier, mtype hash\]
		MTypeAnchored(MtypeOwnerOf<T>, MtypeHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no MTYPE with the given hash.
		MTypeNotFound,
		/// The MTYPE already exists.
		MTypeAlreadyExists,
		/// Invalid StreamId encoding.
		InvalidStreamCidEncoding,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new MTYPE and associates it with its owner.
		///
		/// * origin: the identifier of the MTYPE owner
		/// * hash: the MTYPE hash. It has to be unique.
		/// * stream_cid: CID of MTYPE
		#[pallet::weight(<T as pallet::Config>::WeightInfo::anchor())]
		pub fn anchor(origin: OriginFor<T>, mtype_hash: MtypeHashOf<T>, stream_cid: CidOf) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<Mtypes<T>>::contains_key(&mtype_hash), Error::<T>::MTypeAlreadyExists);

			let cid_base = str::from_utf8(&stream_cid).unwrap();
			ensure!(
				utils::is_base_32(cid_base) || utils::is_base_58(cid_base),
				Error::<T>::InvalidStreamCidEncoding
			);
			let block_number = <frame_system::Pallet<T>>::block_number();

			log::debug!("Creating MTYPE with hash {:?} and owner {:?}", &mtype_hash, &owner);
			<Mtypes<T>>::insert(
				&mtype_hash,
				MTypeDetails {
					owner: owner.clone(),
					stream_cid,
					block_number,
				},
			);

			Self::deposit_event(Event::MTypeAnchored(owner, mtype_hash));

			Ok(())
		}
	}
}
