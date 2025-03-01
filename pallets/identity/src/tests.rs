// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

// Tests for Identity Pallet

use super::*;
use crate::{
	self as pallet_identity,
	legacy::{IdentityField, IdentityInfo},
};

use codec::{Decode, Encode};
use frame_support::{
	assert_noop, assert_ok, derive_impl, parameter_types,
	traits::{ConstU32, ConstU64, Get, OnFinalize, OnInitialize},
};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_io::crypto::{sr25519_generate, sr25519_sign};
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::{
	traits::{BadOrigin, BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSignature, MultiSigner,
};

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;
pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Identity: pallet_identity,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = ();
}

parameter_types! {
	pub const MaxAdditionalFields: u32 = 2;
	pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxSubAccounts = ConstU32<2>;
	type IdentityInformation = IdentityInfo<MaxAdditionalFields>;
	type MaxRegistrars = MaxRegistrars;
	type RegistrarOrigin = EnsureRoot<Self::AccountId>;
	type OffchainSignature = MultiSignature;
	type SigningPublicKey = AccountPublic;
	type UsernameAuthorityOrigin = EnsureRoot<Self::AccountId>;
	type PendingUsernameExpiration = ConstU64<100>;
	type MaxSuffixLength = ConstU32<7>;
	type MaxUsernameLength = ConstU32<32>;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.register_extension(KeystoreExt::new(MemoryKeystore::new()));
	ext.execute_with(|| System::set_block_number(1));
	ext
}

fn run_to_block(n: u64) {
	while System::block_number() < n {
		Identity::on_finalize(System::block_number());
		System::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
		Identity::on_initialize(System::block_number());
	}
}

fn account(id: u8) -> AccountIdOf<Test> {
	[id; 32].into()
}

fn account_from_u32(id: u32) -> AccountIdOf<Test> {
	let mut buffer = [255u8; 32];
	let id_bytes = id.to_le_bytes();
	let id_size = id_bytes.len();
	for ii in 0..buffer.len() / id_size {
		let s = ii * id_size;
		let e = s + id_size;
		buffer[s..e].clone_from_slice(&id_bytes[..]);
	}
	buffer.into()
}

fn accounts() -> [AccountIdOf<Test>; 8] {
	[
		account(1),
		account(2),
		account(3),
		account(4), // unfunded
		account(10),
		account(20),
		account(30),
		account(40), // unfunded
	]
}

fn unfunded_accounts() -> [AccountIdOf<Test>; 2] {
	[account(100), account(101)]
}

// First return value is a username that would be submitted as a parameter to the dispatchable. As
// in, it has no suffix attached. Second is a full BoundedVec username with suffix, which is what a
// user would need to sign.
fn test_username_of(int: Vec<u8>, suffix: Vec<u8>) -> (Vec<u8>, Username<Test>) {
	let base = b"testusername";
	let mut username = Vec::with_capacity(base.len() + int.len());
	username.extend(base);
	username.extend(int);

	let mut bounded_username = Vec::with_capacity(username.len() + suffix.len() + 1);
	bounded_username.extend(username.clone());
	bounded_username.extend(b"@");
	bounded_username.extend(suffix);
	let bounded_username = Username::<Test>::try_from(bounded_username)
		.expect("test usernames should fit within bounds");

	(username, bounded_username)
}
fn infoof_ten() -> IdentityInfo<MaxAdditionalFields> {
	IdentityInfo {
		display: Data::Raw(b"ten".to_vec().try_into().unwrap()),
		legal: Data::Raw(b"The Right Ordinal Ten, Esq.".to_vec().try_into().unwrap()),
		..Default::default()
	}
}

fn infoof_twenty() -> IdentityInfo<MaxAdditionalFields> {
	IdentityInfo {
		display: Data::Raw(b"twenty".to_vec().try_into().unwrap()),
		legal: Data::Raw(b"The Right Ordinal Twenty, Esq.".to_vec().try_into().unwrap()),
		..Default::default()
	}
}

#[test]
fn identity_fields_repr_works() {
	// `SimpleIdentityField` sanity checks.
	assert_eq!(IdentityField::Display as u64, 1 << 0);
	assert_eq!(IdentityField::Legal as u64, 1 << 1);
	assert_eq!(IdentityField::Web as u64, 1 << 2);
	assert_eq!(IdentityField::Email as u64, 1 << 3);
	assert_eq!(IdentityField::Image as u64, 1 << 4);

	let fields = IdentityField::Legal | IdentityField::Web;

	assert!(!fields.contains(IdentityField::Display));
	assert!(fields.contains(IdentityField::Legal));
	assert!(fields.contains(IdentityField::Web));
	assert!(!fields.contains(IdentityField::Email));
	assert!(!fields.contains(IdentityField::Image));

	// Ensure that the `u64` representation matches what we expect.
	assert_eq!(
		fields.bits(),
		0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000110
	);
}

#[test]
fn editing_subaccounts_should_work() {
	new_test_ext().execute_with(|| {
		let data = |x| Data::Raw(vec![x; 1].try_into().unwrap());
		let [one, two, three, _, ten, twenty, _, _] = accounts();

		assert_noop!(
			Identity::add_sub(RuntimeOrigin::signed(ten.clone()), twenty.clone(), data(1)),
			Error::<Test>::NoIdentity
		);

		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(ten_info.clone())
		));
		// first sub account
		assert_ok!(Identity::add_sub(RuntimeOrigin::signed(ten.clone()), one.clone(), data(1)));
		assert_eq!(SuperOf::<Test>::get(one.clone()), Some((ten.clone(), data(1))));

		// second sub account
		assert_ok!(Identity::add_sub(RuntimeOrigin::signed(ten.clone()), two.clone(), data(2)));
		assert_eq!(SuperOf::<Test>::get(one.clone()), Some((ten.clone(), data(1))));
		assert_eq!(SuperOf::<Test>::get(two.clone()), Some((ten.clone(), data(2))));

		// third sub account is too many
		assert_noop!(
			Identity::add_sub(RuntimeOrigin::signed(ten.clone()), three.clone(), data(3)),
			Error::<Test>::TooManySubAccounts
		);

		// rename first sub account
		assert_ok!(Identity::rename_sub(RuntimeOrigin::signed(ten.clone()), one.clone(), data(11)));
		assert_eq!(SuperOf::<Test>::get(one.clone()), Some((ten.clone(), data(11))));
		assert_eq!(SuperOf::<Test>::get(two.clone()), Some((ten.clone(), data(2))));

		// remove first sub account
		assert_ok!(Identity::remove_sub(RuntimeOrigin::signed(ten.clone()), one.clone()));
		assert_eq!(SuperOf::<Test>::get(one.clone()), None);
		assert_eq!(SuperOf::<Test>::get(two.clone()), Some((ten.clone(), data(2))));

		// add third sub account
		assert_ok!(Identity::add_sub(RuntimeOrigin::signed(ten.clone()), three.clone(), data(3)));
		assert_eq!(SuperOf::<Test>::get(one), None);
		assert_eq!(SuperOf::<Test>::get(two), Some((ten.clone(), data(2))));
		assert_eq!(SuperOf::<Test>::get(three), Some((ten.clone(), data(3))));
	});
}

#[test]
fn resolving_subaccount_ownership_works() {
	new_test_ext().execute_with(|| {
		let data = |x| Data::Raw(vec![x; 1].try_into().unwrap());
		let [one, _, _, _, ten, twenty, _, _] = accounts();

		let ten_info = infoof_ten();
		let twenty_info = infoof_twenty();
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(ten.clone()), Box::new(ten_info)));
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(twenty.clone()),
			Box::new(twenty_info)
		));

		// 10 claims 1 as a subaccount
		assert_ok!(Identity::add_sub(RuntimeOrigin::signed(ten.clone()), one.clone(), data(1)));
		// 20 cannot claim 1 now
		assert_noop!(
			Identity::add_sub(RuntimeOrigin::signed(twenty.clone()), one.clone(), data(1)),
			Error::<Test>::AlreadyClaimed
		);
		// 1 wants to be with 20 so it quits from 10
		assert_ok!(Identity::quit_sub(RuntimeOrigin::signed(one.clone())));
		// 20 can claim 1 now
		assert_ok!(Identity::add_sub(RuntimeOrigin::signed(twenty), one, data(1)));
	});
}

#[test]
fn setting_identity_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, _, _, ten, twenty, _, _] = accounts();

		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(ten), Box::new(infoof_ten())));
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(twenty),
			Box::new(infoof_twenty())
		));
	});
}

#[test]
fn trailing_zeros_decodes_into_default_data() {
	let encoded = Data::Raw(b"Hello".to_vec().try_into().unwrap()).encode();
	assert!(<(Data, Data)>::decode(&mut &encoded[..]).is_err());
	let input = &mut &encoded[..];
	let (a, b) = <(Data, Data)>::decode(&mut AppendZerosInput::new(input)).unwrap();
	assert_eq!(a, Data::Raw(b"Hello".to_vec().try_into().unwrap()));
	assert_eq!(b, Data::None);
}

#[test]
fn adding_registrar_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, three, _, _, _, _, _] = accounts();
		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));

		let fields = IdentityField::Display | IdentityField::Legal;
		assert_ok!(Identity::set_fields(RuntimeOrigin::signed(three.clone()), fields.bits()));
		assert_eq!(
			Identity::registrars(),
			vec![Some(RegistrarInfo { account: three, fields: fields.bits() })]
		);
	});
}

#[test]
fn removing_registrar_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, three, _, _, _, _, _] = accounts();
		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));

		let fields = IdentityField::Display | IdentityField::Legal;
		assert_ok!(Identity::set_fields(RuntimeOrigin::signed(three.clone()), fields.bits()));
		assert_eq!(
			Identity::registrars(),
			vec![Some(RegistrarInfo { account: three.clone(), fields: fields.bits() })]
		);

		assert_ok!(Identity::remove_registrar(RuntimeOrigin::root(), three.clone()));
		assert_eq!(Identity::registrars(), vec![]);
	});
}

#[test]
fn amount_of_registrars_is_limited() {
	new_test_ext().execute_with(|| {
		for i in 1..MaxRegistrars::get() + 1 {
			assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), account_from_u32(i)));
		}
		let last_registrar = MaxRegistrars::get() + 1;
		assert_noop!(
			Identity::add_registrar(RuntimeOrigin::root(), account_from_u32(last_registrar)),
			Error::<Test>::TooManyRegistrars
		);
	});
}

#[test]
fn registration_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, three, _, ten, _, _, _] = accounts();
		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));
		let mut three_fields = infoof_ten();
		three_fields.additional.try_push(Default::default()).unwrap();
		three_fields.additional.try_push(Default::default()).unwrap();
		assert!(three_fields.additional.try_push(Default::default()).is_err());
		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(ten_info.clone())
		));
		assert_eq!(Identity::identity(ten.clone()).unwrap().0.info, ten_info);
		assert_ok!(Identity::clear_identity(RuntimeOrigin::signed(ten.clone())));
		assert_noop!(
			Identity::clear_identity(RuntimeOrigin::signed(ten)),
			Error::<Test>::NoIdentity
		);
	});
}
//
#[test]
fn uninvited_judgement_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, three, _, ten, _, _, _] = accounts();

		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(three.clone()),
				ten.clone(),
				Judgement::Reasonable,
				H256::random()
			),
			Error::<Test>::RegistrarNotFound
		);

		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));
		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(three.clone()),
				ten.clone(),
				Judgement::Reasonable,
				H256::random()
			),
			Error::<Test>::InvalidTarget
		);

		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(infoof_ten())
		));
		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(three.clone()),
				ten.clone(),
				Judgement::Reasonable,
				H256::random()
			),
			Error::<Test>::JudgementForDifferentIdentity
		);

		let identity_hash = BlakeTwo256::hash_of(&infoof_ten());

		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(ten.clone()),
				ten.clone(),
				Judgement::Reasonable,
				identity_hash
			),
			Error::<Test>::RegistrarNotFound
		);
		assert_noop!(
			Identity::provide_judgement(
				RuntimeOrigin::signed(three.clone()),
				ten.clone(),
				Judgement::Requested,
				identity_hash
			),
			Error::<Test>::InvalidJudgement
		);

		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(three.clone()),
			ten.clone(),
			Judgement::Reasonable,
			identity_hash
		));
		assert_eq!(
			Identity::identity(ten.clone()).unwrap().0.judgements,
			vec![(three, Judgement::Reasonable)]
		);
	});
}

#[test]
fn clearing_identity_and_judgement_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, three, _, ten, _, _, _] = accounts();
		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(infoof_ten())
		));
		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(three.clone()),
			ten.clone(),
			Judgement::Reasonable,
			BlakeTwo256::hash_of(&infoof_ten())
		));
		assert_ok!(Identity::clear_identity(RuntimeOrigin::signed(ten.clone())));
		assert_eq!(Identity::identity(ten), None);
	});
}

#[test]
fn killing_account_should_work() {
	new_test_ext().execute_with(|| {
		let [one, _, _, _, ten, _, _, _] = accounts();
		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(ten.clone()), Box::new(ten_info)));
		assert_noop!(Identity::kill_identity(RuntimeOrigin::signed(one), ten.clone()), BadOrigin);
		assert_ok!(Identity::kill_identity(RuntimeOrigin::root(), ten.clone()));
		assert_eq!(Identity::identity(ten.clone()), None);
		assert_noop!(
			Identity::kill_identity(RuntimeOrigin::root(), ten),
			Error::<Test>::NoIdentity
		);
	});
}

//[TODO]
// #[test]
// fn setting_subaccounts_should_work() {
// 	new_test_ext().execute_with(|| {
// 		let [_, _, _, _, ten, twenty, thirty, forty] = accounts();
// 		let ten_info = infoof_ten();
// 		let mut subs = vec![(twenty.clone(), Data::Raw(vec![40; 1].try_into().unwrap()))];
// 		assert_noop!(
// 			Identity::set_subs(RuntimeOrigin::signed(ten.clone()), subs.clone()),
// 			Error::<Test>::NotFound
// 		);

// 		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(ten.clone()), Box::new(ten_info)));
// 		assert_ok!(Identity::set_subs(RuntimeOrigin::signed(ten.clone()), subs.clone()));
// 		// assert_eq!(Identity::subs_of(ten.clone()), BoundedVec::try_from(vec![twenty.clone()]));
// 		assert_eq!(Identity::subs_of(ten.clone()), (vec![twenty.clone()].try_into().unwrap()));
// 		assert_eq!(
// 			Identity::super_of(twenty.clone()),
// 			Some((ten.clone(), Data::Raw(vec![40; 1].try_into().unwrap())))
// 		);

// 		// push another item and re-set it.
// 		subs.push((thirty.clone(), Data::Raw(vec![50; 1].try_into().unwrap())));
// 		assert_ok!(Identity::set_subs(RuntimeOrigin::signed(ten.clone()), subs.clone()));
// 		assert_eq!(
// 			Identity::subs_of(ten.clone()),
// 			vec![twenty.clone(), thirty.clone()].try_into().unwrap()
// 		);
// 		assert_eq!(
// 			Identity::super_of(twenty.clone()),
// 			Some((ten.clone(), Data::Raw(vec![40; 1].try_into().unwrap())))
// 		);
// 		assert_eq!(
// 			Identity::super_of(thirty.clone()),
// 			Some((ten.clone(), Data::Raw(vec![50; 1].try_into().unwrap())))
// 		);

// 		// switch out one of the items and re-set.
// 		subs[0] = (forty.clone(), Data::Raw(vec![60; 1].try_into().unwrap()));
// 		assert_ok!(Identity::set_subs(RuntimeOrigin::signed(ten.clone()), subs.clone()));
// 		assert_eq!(
// 			Identity::subs_of(ten.clone()),
// 			vec![forty.clone(), thirty.clone()].try_into().unwrap()
// 		);
// 		assert_eq!(Identity::super_of(twenty.clone()), None);
// 		assert_eq!(
// 			Identity::super_of(thirty.clone()),
// 			Some((ten.clone(), Data::Raw(vec![50; 1].try_into().unwrap())))
// 		);
// 		assert_eq!(
// 			Identity::super_of(forty.clone()),
// 			Some((ten.clone(), Data::Raw(vec![60; 1].try_into().unwrap())))
// 		);

// 		// clear
// 		assert_ok!(Identity::set_subs(RuntimeOrigin::signed(ten.clone()), vec![]));
// 		assert_eq!(Identity::subs_of(ten.clone()), BoundedVec::default());
// 		assert_eq!(Identity::super_of(thirty.clone()), None);
// 		assert_eq!(Identity::super_of(forty), None);

// 		subs.push((twenty, Data::Raw(vec![40; 1].try_into().unwrap())));
// 		assert_noop!(
// 			Identity::set_subs(RuntimeOrigin::signed(ten), subs.clone()),
// 			Error::<Test>::TooManySubAccounts
// 		);
// 	});
// }

#[test]
fn clearing_account_should_remove_subaccounts_and_refund() {
	new_test_ext().execute_with(|| {
		let [_, _, _, _, ten, twenty, _, _] = accounts();
		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(ten_info.clone())
		));
		assert_ok!(Identity::set_subs(
			RuntimeOrigin::signed(ten.clone()),
			vec![(twenty.clone(), Data::Raw(vec![40; 1].try_into().unwrap()))]
		));
		assert_ok!(Identity::clear_identity(RuntimeOrigin::signed(ten.clone())));
		assert!(Identity::super_of(twenty).is_none());
	});
}

#[test]
fn killing_account_should_remove_subaccounts_and_not_refund() {
	new_test_ext().execute_with(|| {
		let [_, _, _, _, ten, twenty, _, _] = accounts();
		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(RuntimeOrigin::signed(ten.clone()), Box::new(ten_info)));
		assert_ok!(Identity::set_subs(
			RuntimeOrigin::signed(ten.clone()),
			vec![(twenty.clone(), Data::Raw(vec![40; 1].try_into().unwrap()))]
		));
		assert_ok!(Identity::kill_identity(RuntimeOrigin::root(), ten.clone()));
		assert!(Identity::super_of(twenty).is_none());
	});
}

#[test]
fn cancelling_requested_judgement_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, three, _, ten, _, _, _] = accounts();
		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));
		assert_noop!(
			Identity::cancel_request(RuntimeOrigin::signed(ten.clone()), three.clone()),
			Error::<Test>::NoIdentity
		);
		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(ten_info.clone())
		));
		assert_ok!(Identity::request_judgement(RuntimeOrigin::signed(ten.clone()), three.clone()));
		assert_ok!(Identity::cancel_request(RuntimeOrigin::signed(ten.clone()), three.clone()));
		assert_noop!(
			Identity::cancel_request(RuntimeOrigin::signed(ten.clone()), three.clone()),
			Error::<Test>::NotFound
		);

		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(three.clone()),
			ten.clone(),
			Judgement::Reasonable,
			BlakeTwo256::hash_of(&ten_info)
		));
		assert_noop!(
			Identity::cancel_request(RuntimeOrigin::signed(ten), three),
			Error::<Test>::JudgementGiven
		);
	});
}

#[test]
fn requesting_judgement_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, three, four, ten, _, _, _] = accounts();
		let ten_info = infoof_ten();
		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(ten_info.clone())
		));
		assert_ok!(Identity::request_judgement(RuntimeOrigin::signed(ten.clone()), three.clone()));
		// Re-requesting won't work.
		assert_noop!(
			Identity::request_judgement(RuntimeOrigin::signed(ten.clone()), three.clone()),
			Error::<Test>::StickyJudgement
		);

		// Re-requesting won't work.
		assert_noop!(
			Identity::request_judgement(RuntimeOrigin::signed(ten.clone()), three.clone()),
			Error::<Test>::StickyJudgement
		);
		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(three.clone()),
			ten.clone(),
			Judgement::Erroneous,
			BlakeTwo256::hash_of(&ten_info)
		));
		// Re-requesting still won't work as it's erroneous.
		assert_noop!(
			Identity::request_judgement(RuntimeOrigin::signed(ten.clone()), three.clone()),
			Error::<Test>::StickyJudgement
		);

		// Requesting from a second registrar still works.
		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), four.clone()));
		assert_ok!(Identity::request_judgement(RuntimeOrigin::signed(ten.clone()), four));

		// Re-requesting after the judgement has been reduced works.
		assert_ok!(Identity::provide_judgement(
			RuntimeOrigin::signed(three.clone()),
			ten.clone(),
			Judgement::OutOfDate,
			BlakeTwo256::hash_of(&ten_info)
		));
		assert_ok!(Identity::request_judgement(RuntimeOrigin::signed(ten), three));
	});
}

#[test]
fn add_registrar_should_fail_if_registrar_already_exists() {
	new_test_ext().execute_with(|| {
		let [_, _, three, _, _, _, _, _] = accounts();

		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));
		assert_noop!(
			Identity::add_registrar(RuntimeOrigin::root(), three.clone()),
			Error::<Test>::RegistrarAlreadyExists
		);
	});
}
#[test]
fn setting_account_id_should_work() {
	new_test_ext().execute_with(|| {
		let [_, _, three, four, _, _, _, _] = accounts();
		assert_ok!(Identity::add_registrar(RuntimeOrigin::root(), three.clone()));
		// account 4 cannot change the first registrar's identity since it's owned by 3.
		assert_noop!(
			Identity::set_account_id(RuntimeOrigin::signed(four.clone()), three.clone()),
			Error::<Test>::RegistrarNotFound
		);
		// account 3 can, because that's the registrar's current account.
		assert_ok!(Identity::set_account_id(RuntimeOrigin::signed(three.clone()), four.clone()));
		// account 4 can now, because that's their new ID.
		assert_ok!(Identity::set_account_id(RuntimeOrigin::signed(four), three));
	});
}

#[test]
fn test_has_identity() {
	new_test_ext().execute_with(|| {
		let [_, _, _, _, ten, _, _, _] = accounts();
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(infoof_ten())
		));
		assert!(Identity::has_identity(&ten, IdentityField::Display as u64));
		assert!(Identity::has_identity(&ten, IdentityField::Legal as u64));
		assert!(Identity::has_identity(
			&ten,
			IdentityField::Display as u64 | IdentityField::Legal as u64
		));
		assert!(!Identity::has_identity(
			&ten,
			IdentityField::Display as u64 | IdentityField::Legal as u64 | IdentityField::Web as u64
		));
	});
}
#[test]
fn reap_identity_works() {
	new_test_ext().execute_with(|| {
		let [_, _, _, _, ten, twenty, _, _] = accounts();
		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(ten.clone()),
			Box::new(ten_info.clone())
		));
		assert_ok!(Identity::set_subs(
			RuntimeOrigin::signed(ten.clone()),
			vec![(twenty.clone(), Data::Raw(vec![40; 1].try_into().unwrap()))]
		));
		// reap
		assert_ok!(Identity::reap_identity(&ten));
		// no identity or subs
		assert!(Identity::identity(ten.clone()).is_none());
		assert!(Identity::super_of(twenty).is_none());
		// balance is unreserved
	});
}

#[test]
fn adding_and_removing_authorities_should_work() {
	new_test_ext().execute_with(|| {
		let [authority, _] = unfunded_accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 10;

		// add
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));
		assert_eq!(
			UsernameAuthorities::<Test>::get(&authority),
			Some(AuthorityPropertiesOf::<Test> {
				suffix: suffix.clone().try_into().unwrap(),
				allocation
			})
		);

		// update allocation
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			11u32
		));
		assert_eq!(
			UsernameAuthorities::<Test>::get(&authority),
			Some(AuthorityPropertiesOf::<Test> {
				suffix: suffix.try_into().unwrap(),
				allocation: 11
			})
		);

		// remove
		assert_ok!(Identity::remove_username_authority(RuntimeOrigin::root(), authority.clone(),));
		assert!(UsernameAuthorities::<Test>::get(&authority).is_none());
	});
}

#[test]
fn set_username_with_signature_without_existing_identity_should_work() {
	new_test_ext().execute_with(|| {
		// set up authority
		let [authority, _] = unfunded_accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 10;
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));

		// set up username
		let (username, username_to_sign) = test_username_of(b"42".to_vec(), suffix);
		let encoded_username = Encode::encode(&username_to_sign.to_vec());

		// set up user and sign message
		let public = sr25519_generate(0.into(), None);
		let who_account: AccountIdOf<Test> = MultiSigner::Sr25519(public).into_account().into();
		let signature =
			MultiSignature::Sr25519(sr25519_sign(0.into(), &public, &encoded_username).unwrap());

		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority),
			who_account.clone(),
			username.clone(),
			Some(signature)
		));

		// Even though user has no balance and no identity, they get a default one for free.
		assert_eq!(
			Identity::identity(&who_account),
			Some((
				Registration { judgements: Default::default(), info: Default::default() },
				Some(username_to_sign.clone())
			))
		);
		// Lookup from username to account works.
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&username_to_sign),
			Some(who_account)
		);
	});
}

#[test]
fn set_username_with_signature_with_existing_identity_should_work() {
	new_test_ext().execute_with(|| {
		// set up authority
		let [authority, _] = unfunded_accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 10;
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));

		// set up username
		let (username, username_to_sign) = test_username_of(b"42".to_vec(), suffix);
		let encoded_username = Encode::encode(&username_to_sign.to_vec());

		// set up user and sign message
		let public = sr25519_generate(0.into(), None);
		let who_account: AccountIdOf<Test> = MultiSigner::Sr25519(public).into_account().into();
		let signature =
			MultiSignature::Sr25519(sr25519_sign(0.into(), &public, &encoded_username).unwrap());

		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(who_account.clone()),
			Box::new(ten_info.clone())
		));
		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority),
			who_account.clone(),
			username.clone(),
			Some(signature)
		));

		assert_eq!(
			Identity::identity(&who_account),
			Some((
				Registration { judgements: Default::default(), info: ten_info },
				Some(username_to_sign.clone())
			))
		);
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&username_to_sign),
			Some(who_account)
		);
	});
}

#[test]
fn set_username_with_bytes_signature_should_work() {
	new_test_ext().execute_with(|| {
		// set up authority
		let [authority, _] = unfunded_accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 10;
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));

		// set up user
		let public = sr25519_generate(0.into(), None);
		let who_account: AccountIdOf<Test> = MultiSigner::Sr25519(public).into_account().into();

		// set up username
		let (username, username_to_sign) = test_username_of(b"42".to_vec(), suffix);
		let unwrapped_username = username_to_sign.to_vec();

		// Sign an unwrapped version, as in `username.suffix`.
		let unwrapped_encoded = Encode::encode(&unwrapped_username);
		let signature_on_unwrapped =
			MultiSignature::Sr25519(sr25519_sign(0.into(), &public, &unwrapped_encoded).unwrap());

		// Trivial
		assert_ok!(Identity::validate_signature(
			&unwrapped_encoded,
			&signature_on_unwrapped,
			&who_account
		));

		// Here we are going to wrap the username and suffix in "<Bytes>" and verify that the
		// signature verification still works, but only the username gets set in storage.
		let prehtml = b"<Bytes>";
		let posthtml = b"</Bytes>";
		let mut wrapped_username: Vec<u8> =
			Vec::with_capacity(unwrapped_username.len() + prehtml.len() + posthtml.len());
		wrapped_username.extend(prehtml);
		wrapped_username.extend(unwrapped_encoded.clone());
		wrapped_username.extend(posthtml);
		let signature_on_wrapped =
			MultiSignature::Sr25519(sr25519_sign(0.into(), &public, &wrapped_username).unwrap());

		// We want to call `validate_signature` on the *unwrapped* username, but the signature on
		// the *wrapped* data.
		assert_ok!(Identity::validate_signature(
			&unwrapped_encoded,
			&signature_on_wrapped,
			&who_account
		));

		// Make sure it really works in context. Call `set_username_for` with the signature on the
		// wrapped data.
		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority),
			who_account.clone(),
			username,
			Some(signature_on_wrapped)
		));

		// The username in storage should not include `<Bytes>`. As in, it's the original
		// `username_to_sign`.
		assert_eq!(
			Identity::identity(&who_account),
			Some((
				Registration { judgements: Default::default(), info: Default::default() },
				Some(username_to_sign.clone())
			))
		);
		// Likewise for the lookup.
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&username_to_sign),
			Some(who_account)
		);
	});
}

#[test]
fn set_username_with_acceptance_should_work() {
	new_test_ext().execute_with(|| {
		// set up authority
		let [authority, who] = unfunded_accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 10;
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));

		// set up username
		let (username, full_username) = test_username_of(b"101".to_vec(), suffix);
		let now = frame_system::Pallet::<Test>::block_number();
		let expiration = now + <<Test as Config>::PendingUsernameExpiration as Get<u64>>::get();

		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority),
			who.clone(),
			username.clone(),
			None
		));

		// Should be pending
		assert_eq!(
			PendingUsernames::<Test>::get::<&Username<Test>>(&full_username),
			Some((who.clone(), expiration))
		);

		// Now the user can accept
		assert_ok!(Identity::accept_username(
			RuntimeOrigin::signed(who.clone()),
			full_username.clone()
		));

		// No more pending
		assert!(PendingUsernames::<Test>::get::<&Username<Test>>(&full_username).is_none());
		// Check Identity storage
		assert_eq!(
			Identity::identity(&who),
			Some((
				Registration { judgements: Default::default(), info: Default::default() },
				Some(full_username.clone())
			))
		);
		// Check reverse lookup
		assert_eq!(AccountOfUsername::<Test>::get::<&Username<Test>>(&full_username), Some(who));
	});
}

#[test]
fn invalid_usernames_should_be_rejected() {
	new_test_ext().execute_with(|| {
		let [authority, who] = unfunded_accounts();
		let allocation: u32 = 10;
		let valid_suffix = b"test".to_vec();
		let invalid_suffixes = [
			b"te.st".to_vec(), // not alphanumeric
			b"su:ffx".to_vec(),
			b"su_ffx".to_vec(),
			b"Suffix".to_vec(),   // capital
			b"suffixes".to_vec(), // too long
			b"suffix94".to_vec(), // alpha-numeric
		];
		for suffix in invalid_suffixes {
			assert_noop!(
				Identity::add_username_authority(
					RuntimeOrigin::root(),
					authority.clone(),
					suffix.clone(),
					allocation
				),
				Error::<Test>::InvalidSuffix
			);
		}

		// set a valid one now
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			valid_suffix,
			allocation
		));

		// set up usernames
		let invalid_usernames = [
			b"TestUsername".to_vec(),
			b"test_username".to_vec(),
			b"test-username".to_vec(),
			b"test:username".to_vec(),
			b"test@username".to_vec(),
			b"test!username".to_vec(),
			b"test$username".to_vec(),
			//0         1         2      v With `.test` this makes it too long.
			b"testusernametestusernametest".to_vec(),
		];
		for username in invalid_usernames {
			assert_noop!(
				Identity::set_username_for(
					RuntimeOrigin::signed(authority.clone()),
					who.clone(),
					username.clone(),
					None
				),
				Error::<Test>::InvalidUsername
			);
		}

		// valid one works
		let valid_username = b"testusernametestusernametes".to_vec();
		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority),
			who,
			valid_username,
			None
		));
	});
}

#[test]
fn authorities_should_run_out_of_allocation() {
	new_test_ext().execute_with(|| {
		// set up authority
		let [authority, _] = unfunded_accounts();
		let [pi, e, c, _, _, _, _, _] = accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 2;
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));

		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority.clone()),
			pi,
			b"username314159".to_vec(),
			None
		));
		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority.clone()),
			e,
			b"username271828".to_vec(),
			None
		));
		assert_noop!(
			Identity::set_username_for(
				RuntimeOrigin::signed(authority.clone()),
				c,
				b"username299792458".to_vec(),
				None
			),
			Error::<Test>::NoAllocation
		);
	});
}

#[test]
fn setting_primary_should_work() {
	new_test_ext().execute_with(|| {
		// set up authority
		let [authority, _] = unfunded_accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 10;
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));

		// set up user
		let public = sr25519_generate(0.into(), None);
		let who_account: AccountIdOf<Test> = MultiSigner::Sr25519(public).into_account().into();

		// set up username
		let (first_username, first_to_sign) = test_username_of(b"42".to_vec(), suffix.clone());
		let encoded_username = Encode::encode(&first_to_sign.to_vec());
		let first_signature =
			MultiSignature::Sr25519(sr25519_sign(0.into(), &public, &encoded_username).unwrap());

		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority.clone()),
			who_account.clone(),
			first_username.clone(),
			Some(first_signature)
		));

		// First username set as primary.
		assert_eq!(
			Identity::identity(&who_account),
			Some((
				Registration { judgements: Default::default(), info: Default::default() },
				Some(first_to_sign.clone())
			))
		);

		// set up username
		let (second_username, second_to_sign) = test_username_of(b"101".to_vec(), suffix);
		let encoded_username = Encode::encode(&second_to_sign.to_vec());
		let second_signature =
			MultiSignature::Sr25519(sr25519_sign(0.into(), &public, &encoded_username).unwrap());

		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority),
			who_account.clone(),
			second_username.clone(),
			Some(second_signature)
		));

		// The primary is still the first username.
		assert_eq!(
			Identity::identity(&who_account),
			Some((
				Registration { judgements: Default::default(), info: Default::default() },
				Some(first_to_sign.clone())
			))
		);

		// Lookup from both works.
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&first_to_sign),
			Some(who_account.clone())
		);
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&second_to_sign),
			Some(who_account.clone())
		);

		assert_ok!(Identity::set_primary_username(
			RuntimeOrigin::signed(who_account.clone()),
			second_to_sign.clone()
		));

		// The primary is now the second username.
		assert_eq!(
			Identity::identity(&who_account),
			Some((
				Registration { judgements: Default::default(), info: Default::default() },
				Some(second_to_sign.clone())
			))
		);

		// Lookup from both still works.
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&first_to_sign),
			Some(who_account.clone())
		);
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&second_to_sign),
			Some(who_account)
		);
	});
}

#[test]
fn unaccepted_usernames_should_expire() {
	new_test_ext().execute_with(|| {
		// set up authority
		let [authority, who] = unfunded_accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 10;
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));

		// set up username
		let (username, full_username) = test_username_of(b"101".to_vec(), suffix);
		let now = frame_system::Pallet::<Test>::block_number();
		let expiration = now + <<Test as Config>::PendingUsernameExpiration as Get<u64>>::get();

		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority),
			who.clone(),
			username.clone(),
			None
		));

		// Should be pending
		assert_eq!(
			PendingUsernames::<Test>::get::<&Username<Test>>(&full_username),
			Some((who.clone(), expiration))
		);

		run_to_block(now + expiration - 1);

		// Cannot be removed
		assert_noop!(
			Identity::remove_expired_approval(
				RuntimeOrigin::signed(account(1)),
				full_username.clone()
			),
			Error::<Test>::NotExpired
		);

		run_to_block(now + expiration);

		// Anyone can remove
		assert_ok!(Identity::remove_expired_approval(
			RuntimeOrigin::signed(account(1)),
			full_username.clone()
		));

		// No more pending
		assert!(PendingUsernames::<Test>::get::<&Username<Test>>(&full_username).is_none());
	});
}

#[test]
fn removing_dangling_usernames_should_work() {
	new_test_ext().execute_with(|| {
		// set up authority
		let [authority, caller] = unfunded_accounts();
		let suffix: Vec<u8> = b"test".to_vec();
		let allocation: u32 = 10;
		assert_ok!(Identity::add_username_authority(
			RuntimeOrigin::root(),
			authority.clone(),
			suffix.clone(),
			allocation
		));

		// set up username
		let (username, username_to_sign) = test_username_of(b"42".to_vec(), suffix.clone());
		let encoded_username = Encode::encode(&username_to_sign.to_vec());

		// set up user and sign message
		let public = sr25519_generate(0.into(), None);
		let who_account: AccountIdOf<Test> = MultiSigner::Sr25519(public).into_account().into();
		let signature =
			MultiSignature::Sr25519(sr25519_sign(0.into(), &public, &encoded_username).unwrap());

		let ten_info = infoof_ten();
		assert_ok!(Identity::set_identity(
			RuntimeOrigin::signed(who_account.clone()),
			Box::new(ten_info.clone())
		));
		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority.clone()),
			who_account.clone(),
			username.clone(),
			Some(signature)
		));

		// Now they set up a second username.
		let (username_two, username_two_to_sign) = test_username_of(b"43".to_vec(), suffix);
		let encoded_username_two = Encode::encode(&username_two_to_sign.to_vec());

		// set up user and sign message
		let signature_two = MultiSignature::Sr25519(
			sr25519_sign(0.into(), &public, &encoded_username_two).unwrap(),
		);

		assert_ok!(Identity::set_username_for(
			RuntimeOrigin::signed(authority),
			who_account.clone(),
			username_two.clone(),
			Some(signature_two)
		));

		// The primary should still be the first one.
		assert_eq!(
			Identity::identity(&who_account),
			Some((
				Registration { judgements: Default::default(), info: ten_info },
				Some(username_to_sign.clone())
			))
		);

		// But both usernames should look up the account.
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&username_to_sign),
			Some(who_account.clone())
		);
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&username_two_to_sign),
			Some(who_account.clone())
		);

		// Someone tries to remove it, but they can't
		assert_noop!(
			Identity::remove_dangling_username(
				RuntimeOrigin::signed(caller.clone()),
				username_to_sign.clone()
			),
			Error::<Test>::InvalidUsername
		);

		// Now the user calls `clear_identity`
		assert_ok!(Identity::clear_identity(RuntimeOrigin::signed(who_account.clone()),));

		// Identity is gone
		assert!(Identity::identity(who_account.clone()).is_none());

		// The reverse lookup of the primary is gone.
		assert!(AccountOfUsername::<Test>::get::<&Username<Test>>(&username_to_sign).is_none());

		// But the reverse lookup of the non-primary is still there
		assert_eq!(
			AccountOfUsername::<Test>::get::<&Username<Test>>(&username_two_to_sign),
			Some(who_account)
		);

		// Now it can be removed
		assert_ok!(Identity::remove_dangling_username(
			RuntimeOrigin::signed(caller),
			username_two_to_sign.clone()
		));

		// And the reverse lookup is gone
		assert!(AccountOfUsername::<Test>::get::<&Username<Test>>(&username_two_to_sign).is_none());
	});
}
