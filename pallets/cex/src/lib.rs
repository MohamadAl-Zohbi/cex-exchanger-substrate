#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
#[frame_support::pallet]
pub mod pallet {
	use frame_support::codec::{Decode, Encode};
	use frame_support::pallet_prelude::{DispatchResult, *};
	use frame_support::sp_runtime::traits::AccountIdConversion;
	use frame_support::sp_runtime::SaturatedConversion;
	use frame_support::traits::{Currency as NativeCurrency, IsSubType};
	use frame_support::PalletId;
	use frame_support::{dispatch::GetDispatchInfo, traits::Get};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_core::U256;
	use sp_runtime::traits::Dispatchable;
	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_assets::Config {
		type NativeCurrency: NativeCurrency<Self::AccountId>;
		type PalletId: Get<PalletId>;
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>
			+ IsSubType<Call<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>;
	}
	pub type AssetIdOf<T> = <T as pallet_assets::Config>::AssetId;
	pub type AssetBalanceOf<T> = <T as pallet_assets::Config>::Balance;
	type BalanceOf<T> = <<T as Config>::NativeCurrency as NativeCurrency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		AdminRegistred {
			admin_account: T::AccountId,
			exchange_wallet: T::AccountId,
		},
		LiquidityRegistred {
			id: u64,
			token1: T::AssetId,
			token2: T::AssetId,
		},
		AddLiquidity {
			id: u64,
			amount: u128,
			by: T::AccountId,
			token1_balance: AssetBalanceOf<T>,
			token2_balance: AssetBalanceOf<T>,
		},
		RemoveLiquidity {
			id: u64,
			amount: u128,
			by: T::AccountId,
			token1_balance: AssetBalanceOf<T>,
			token2_balance: AssetBalanceOf<T>,
		},
		TokenSwaped {
			pool_id: u64,
			token_in: T::AssetId,
			token_out: T::AssetId,
			amount_in: AssetBalanceOf<T>,
			amount_out: AssetBalanceOf<T>,
			account: T::AccountId,
		},
		NewAdmin {
			who: T::AccountId,
		},
		TransferAmount {
			who: T::AccountId,
			to: T::AccountId,
			amount: AssetBalanceOf<T>,
			token_id: T::AssetId,
		},
	}

	#[derive(Encode, Decode, Clone, Eq, PartialEq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
	pub struct LiquidityPool<AssetId, TokenBalance> {
		token1: AssetId,
		token2: AssetId,
		reserve1: TokenBalance,
		reserve2: TokenBalance,
		total_liquidity: u128,
	}

	pub type TokenLiquidityPool<T> = LiquidityPool<AssetIdOf<T>, AssetBalanceOf<T>>;
	#[pallet::storage]
	#[pallet::getter(fn admin_account)]
	pub type Admin<T: Config> = StorageValue<_, T::AccountId>;
	#[pallet::storage]
	#[pallet::getter(fn next_pool_id)]
	pub type NextPoolId<T: Config> = StorageValue<_, u64>;
	#[pallet::storage]
	#[pallet::getter(fn admin_wallet)]
	pub type PalletWallet<T: Config> = StorageValue<_, T::AccountId>;
	#[pallet::storage]
	#[pallet::getter(fn users)]
	pub type Users<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, bool>;
	#[pallet::storage]
	#[pallet::getter(fn user_balances)]
	pub type UserBalances<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u64, Twox64Concat, T::AccountId, u128>;
	#[pallet::storage]
	#[pallet::getter(fn registred_pairs)]
	pub type RegistredPairs<T: Config> =
		StorageDoubleMap<_, Twox64Concat, AssetIdOf<T>, Twox64Concat, AssetIdOf<T>, bool>;
	#[pallet::storage]
	#[pallet::getter(fn pools)]
	pub type Pools<T: Config> = StorageMap<_, Twox64Concat, u64, TokenLiquidityPool<T>>;

	#[pallet::storage]
	#[pallet::getter(fn admins)]
	pub type Admins<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, bool>;

	#[pallet::error]
	pub enum Error<T> {
		NoneValue,
		StorageOverflow,
		NoPermission,
		LowBalance,
		DuplicatedToken,
		PairsAlreadyRegistred,
		PoolAlreadyRegistred,
		AccountAlreadyRegistred,
		AccountNotFound,
	}
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn register_admin(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			match PalletWallet::<T>::get() {
				Some(_) => return Err(Error::<T>::NoPermission.into()),
				None => {
					<Admin<T>>::put(who.clone());
					<Admins<T>>::insert(who.clone(), true);
					let wallet_account = Self::pallet_account();
					<PalletWallet<T>>::put(wallet_account.clone());
					T::NativeCurrency::deposit_creating(
						&wallet_account,
						Self::u128_to_native_currency_saturated(10000),
					);
					Self::deposit_event(Event::AdminRegistred {
						admin_account: who,
						exchange_wallet: wallet_account,
					});
				},
			};
			Ok(())
		}
		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn register_liquidity(
			origin: OriginFor<T>,
			token1: AssetIdOf<T>,
			token2: AssetIdOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let pool_id: u64 = NextPoolId::<T>::get().unwrap_or(1);
			let is_admin: bool = Admins::<T>::contains_key(who);
			ensure!(is_admin == true, Error::<T>::NoPermission);
			ensure!(
				!(RegistredPairs::<T>::contains_key(token1, token2)
					|| RegistredPairs::<T>::contains_key(token2, token1)),
				Error::<T>::PairsAlreadyRegistred
			);
			ensure!(token1 != token2, Error::<T>::DuplicatedToken);
			let new_pool: LiquidityPool<AssetIdOf<T>, AssetBalanceOf<T>> = LiquidityPool {
				token1,
				token2,
				reserve1: Self::u128_to_assets_balance_saturated(0),
				reserve2: Self::u128_to_assets_balance_saturated(0),
				total_liquidity: 0,
			};
			<RegistredPairs<T>>::insert(token1, token2, true);
			Self::deposit_event(Event::LiquidityRegistred { id: pool_id, token1, token2 });
			NextPoolId::<T>::put(pool_id + 1);
			ensure!(!Pools::<T>::contains_key(pool_id), Error::<T>::PoolAlreadyRegistred);
			<Pools<T>>::insert(pool_id, new_pool);
			Ok(())
		}
		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn add_liquidity(
			origin: OriginFor<T>,
			pool_id: u64,
			account: T::AccountId,
			mut token1_balance: AssetBalanceOf<T>,
			mut token2_balance: AssetBalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let is_admin: bool = Admins::<T>::contains_key(who);
			ensure!(is_admin == true, Error::<T>::NoPermission);
			ensure!(Users::<T>::contains_key(account.clone()), Error::<T>::AccountNotFound);
			ensure!(Pools::<T>::contains_key(pool_id), Error::<T>::NoneValue);
			let pallet_wallet = PalletWallet::<T>::get().unwrap();
			let mut pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::NoneValue).unwrap();
			ensure!(
				token1_balance > Self::u128_to_assets_balance_saturated(0)
					&& token2_balance > Self::u128_to_assets_balance_saturated(0),
				Error::<T>::LowBalance
			);
			let first_liquidity_balance = Self::assets_balance_to_u256_saturated(token1_balance)
				* Self::assets_balance_to_u256_saturated(pool.reserve2);
			let second_liquidity_balance = Self::assets_balance_to_u256_saturated(token2_balance)
				* Self::assets_balance_to_u256_saturated(pool.reserve1);
			ensure!(
				first_liquidity_balance >= ((second_liquidity_balance * 995) / 1000)
					&& first_liquidity_balance <= ((second_liquidity_balance * 1005) / 1000),
				"Unbalanced Liquidity Provided"
			);
			if first_liquidity_balance > second_liquidity_balance {
				token1_balance = Self::u256_to_assets_balance_saturated(
					(Self::assets_balance_to_u256_saturated(token2_balance)
						* Self::assets_balance_to_u256_saturated(pool.reserve2))
						/ Self::assets_balance_to_u256_saturated(pool.reserve1),
				);
			} else if second_liquidity_balance > first_liquidity_balance {
				token2_balance = Self::u256_to_assets_balance_saturated(
					(Self::assets_balance_to_u256_saturated(token1_balance)
						* Self::assets_balance_to_u256_saturated(pool.reserve1))
						/ Self::assets_balance_to_u256_saturated(pool.reserve2),
				);
			}
			Self::transfer_token(
				account.clone(),
				pallet_wallet.clone(),
				pool.token1.into(),
				token1_balance,
			);
			Self::transfer_token(
				account.clone(),
				pallet_wallet.clone(),
				pool.token2.into(),
				token2_balance,
			);
			let total_liquidity = pool.total_liquidity;
			let liquidity_shares: u128;
			if total_liquidity == 0 {
				liquidity_shares = Self::sqrt(
					Self::assets_balance_to_u256_saturated(token1_balance)
						* Self::assets_balance_to_u256_saturated(token2_balance),
				);
			} else {
				liquidity_shares = Self::min(
					(Self::assets_balance_to_u256_saturated(token1_balance)
						* Self::u128_to_u256_saturated(total_liquidity))
						/ Self::assets_balance_to_u256_saturated(pool.reserve1),
					(Self::assets_balance_to_u256_saturated(token2_balance)
						* Self::u128_to_u256_saturated(total_liquidity))
						/ Self::assets_balance_to_u256_saturated(pool.reserve2),
				);
			}
			pool.total_liquidity = liquidity_shares + pool.total_liquidity;
			let old_balance = <UserBalances<T>>::get(pool_id, account.clone()).unwrap_or(0);
			<UserBalances<T>>::insert(pool_id, account.clone(), old_balance + liquidity_shares);
			pool.reserve1 = pool.reserve1 + token1_balance;
			pool.reserve2 = pool.reserve2 + token2_balance;
			Pools::<T>::insert(pool_id, pool);
			Self::deposit_event(Event::AddLiquidity {
				id: pool_id,
				amount: liquidity_shares,
				by: account,
				token1_balance,
				token2_balance,
			});

			Ok(Pays::No.into())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(0)]
		pub fn remove_liquidity(
			origin: OriginFor<T>,
			pool_id: u64,
			account: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let is_admin: bool = Admins::<T>::contains_key(who);
			ensure!(is_admin == true, Error::<T>::NoPermission);
			ensure!(Users::<T>::contains_key(account.clone()), Error::<T>::AccountNotFound);
			ensure!(Pools::<T>::contains_key(pool_id), Error::<T>::NoneValue);
			let pallet_wallet = PalletWallet::<T>::get().unwrap();
			let mut pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::NoneValue).unwrap();
			let liquidity_shares = <UserBalances<T>>::get(pool_id, account.clone()).unwrap_or(0);

			let amount_token1 = Self::u256_to_assets_balance_saturated(
				(Self::u128_to_u256_saturated(liquidity_shares)
					* Self::assets_balance_to_u256_saturated(pool.reserve1))
					/ Self::u128_to_u256_saturated(pool.total_liquidity),
			);

			let amount_token2 = Self::u256_to_assets_balance_saturated(
				(Self::u128_to_u256_saturated(liquidity_shares)
					* Self::assets_balance_to_u256_saturated(pool.reserve2))
					/ Self::u128_to_u256_saturated(pool.total_liquidity),
			);
			pool.total_liquidity = pool.total_liquidity - liquidity_shares;
			<UserBalances<T>>::remove(pool_id, account.clone());
			pool.reserve1 = pool.reserve1 - amount_token1;
			pool.reserve2 = pool.reserve2 - amount_token2;
			Self::transfer_token(
				pallet_wallet.clone(),
				account.clone(),
				pool.token1.into(),
				amount_token1.clone(),
			);
			Self::transfer_token(
				pallet_wallet.clone(),
				account.clone(),
				pool.token2.into(),
				amount_token2.clone(),
			);
			Pools::<T>::insert(pool_id, pool);
			Self::deposit_event(Event::RemoveLiquidity {
				id: pool_id,
				amount: liquidity_shares,
				by: account,
				token1_balance: amount_token1,
				token2_balance: amount_token2,
			});
			Ok(Pays::No.into())
		}
		//token_to_exchange =1 want token 2 else want token 1
		#[pallet::call_index(4)]
		#[pallet::weight(0)]
		pub fn swap_token(
			origin: OriginFor<T>,
			pool_id: u64,
			account: T::AccountId,
			token_to_exchange: u8,
			amount_in: AssetBalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let is_admin: bool = Admins::<T>::contains_key(who);
			ensure!(is_admin == true, Error::<T>::NoPermission);
			ensure!(Users::<T>::contains_key(account.clone()), Error::<T>::AccountNotFound);
			ensure!(amount_in > Self::u128_to_assets_balance_saturated(0), Error::<T>::LowBalance);
			ensure!(Pools::<T>::contains_key(pool_id), Error::<T>::NoneValue);
			let pallet_wallet = PalletWallet::<T>::get().unwrap();
			let mut pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::NoneValue).unwrap();
			let mut token_in = pool.token1;
			let mut token_out = pool.token2;
			let mut reserve_in = pool.reserve1;
			let mut reserve_out = pool.reserve2;
			if token_to_exchange == 2 {
				token_in = pool.token2;
				token_out = pool.token1;
				reserve_in = pool.reserve2;
				reserve_out = pool.reserve1;
			}

			let amount_in_after_fee = (Self::assets_balance_to_u256_saturated(amount_in)
				* Self::u128_to_u256_saturated(98))
				/ Self::u128_to_u256_saturated(100);
			let amount_out = Self::u256_to_assets_balance_saturated(
				(Self::assets_balance_to_u256_saturated(reserve_out) * amount_in_after_fee)
					/ (Self::assets_balance_to_u256_saturated(reserve_in) + amount_in_after_fee),
			);
			ensure!(amount_out < reserve_out, "Insufficient Liquidity");

			if token_to_exchange == 1 {
				pool.reserve1 = pool.reserve1 + amount_in;
				pool.reserve2 = pool.reserve2 - amount_out;
			} else {
				pool.reserve2 = pool.reserve2 + amount_in;
				pool.reserve1 = pool.reserve1 - amount_out;
			}
			Pools::<T>::insert(pool_id, pool);
			Self::transfer_token(
				account.clone(),
				pallet_wallet.clone(),
				token_in.into(),
				amount_in.clone(),
			);
			Self::transfer_token(
				pallet_wallet.clone(),
				account.clone(),
				token_out.into(),
				amount_out.clone(),
			);
			Self::deposit_event(Event::TokenSwaped {
				pool_id,
				token_in,
				token_out,
				amount_in,
				amount_out,
				account,
			});
			Ok(Pays::No.into())
		}
		#[pallet::call_index(5)]
		#[pallet::weight(0)]
		pub fn withdraw_token(
			origin: OriginFor<T>,
			from: T::AccountId,
			to: T::AccountId,
			token: AssetIdOf<T>,
			amount: AssetBalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let admin = Admin::<T>::get().unwrap();
			ensure!(admin == who, Error::<T>::NoPermission);
			ensure!(Users::<T>::contains_key(from.clone()), Error::<T>::AccountNotFound);
			Self::transfer_token(from, to, token, amount);
			Ok(Pays::No.into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(0)]
		pub fn register_user(
			origin: OriginFor<T>,
			account: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let is_admin: bool = Admins::<T>::contains_key(who);
			ensure!(is_admin == true, Error::<T>::NoPermission);
			ensure!(
				!Users::<T>::contains_key(account.clone()),
				Error::<T>::AccountAlreadyRegistred
			);
			Users::<T>::insert(account.clone(), true);
			T::NativeCurrency::deposit_creating(
				&account,
				Self::u128_to_native_currency_saturated(10000),
			);
			Ok(Pays::No.into())
		}
		#[pallet::call_index(7)]
		#[pallet::weight(0)]
		pub fn add_fund(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let admin = Admin::<T>::get().unwrap();
			ensure!(admin == who, Error::<T>::NoPermission);
			T::NativeCurrency::deposit_creating(
				&admin,
				Self::u128_to_native_currency_saturated(1000000000000000000),
			);
			Ok(Pays::No.into())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(0)]
		pub fn register_sub_admin(origin: OriginFor<T>, sub_admin: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = Admin::<T>::get().unwrap();
			ensure!(admin == who, Error::<T>::NoPermission);
			ensure!(
				!Admins::<T>::contains_key(sub_admin.clone()),
				Error::<T>::AccountAlreadyRegistred
			);
			<Admins<T>>::insert(sub_admin.clone(), true);
			Self::deposit_event(Event::NewAdmin { who: sub_admin });
			Ok(())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(0)]
		pub fn transfer_token_to(
			origin: OriginFor<T>,
			amount: AssetBalanceOf<T>,
			token_id: T::AssetId,
			to: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = Admin::<T>::get().unwrap();
			ensure!(admin == who, Error::<T>::NoPermission);
			let pallet_id = Self::pallet_account();
			Self::transfer_token(pallet_id.clone(), to.clone(), token_id, amount);
			Self::deposit_event(Event::TransferAmount { who, to, amount, token_id});
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn transfer_token(
			from: T::AccountId,
			to: T::AccountId,
			token: AssetIdOf<T>,
			amount: AssetBalanceOf<T>,
		) {
			let call: T::RuntimeOrigin = frame_system::RawOrigin::Signed(from).into();
			pallet_assets::Pallet::<T>::transfer(
				call,
				token.into(),
				<T::Lookup as sp_runtime::traits::StaticLookup>::unlookup(to),
				amount,
			)
			.unwrap();
		}
		fn pallet_account() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}
		fn sqrt(y: U256) -> u128 {
			if y > U256::from(3) {
				let mut z = y;
				let mut x = (y / U256::from(2)) + U256::from(1);
				while x < z {
					z = x;
					x = (y / x + x) / U256::from(2);
				}
				return z.as_u128();
			} else if y != U256::from(0) {
				return 1;
			}
			return 0;
		}
		fn min(x: U256, y: U256) -> u128 {
			if x > y {
				x.as_u128()
			} else {
				y.as_u128()
			}
		}
		pub fn u128_to_assets_balance_saturated(input: u128) -> AssetBalanceOf<T> {
			input.saturated_into::<AssetBalanceOf<T>>()
		}
		pub fn assets_balance_to_u128_saturated(input: AssetBalanceOf<T>) -> u128 {
			input.saturated_into::<u128>()
		}
		pub fn assets_balance_to_u256_saturated(input: AssetBalanceOf<T>) -> U256 {
			U256::from(input.saturated_into::<u128>())
		}
		pub fn u256_to_assets_balance_saturated(input: U256) -> AssetBalanceOf<T> {
			input.as_u128().saturated_into::<AssetBalanceOf<T>>()
		}
		pub fn u128_to_u256_saturated(input: u128) -> U256 {
			U256::from(input)
		}

		pub fn u128_to_native_currency_saturated(input: u128) -> BalanceOf<T> {
			input.saturated_into::<BalanceOf<T>>()
		}
	}
}
