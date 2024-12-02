use bigdecimal::{BigDecimal, Zero};
use orm::helpers::OrderByDb;
use orm::validators::{ValidatorSortByDb, ValidatorStateDb};

use crate::appstate::AppState;
use crate::dto::pos::{OrderByDto, ValidatorSortFieldDto, ValidatorStateDto};
use crate::error::pos::PoSError;
use crate::repository::chain::{ChainRepository, ChainRepositoryTrait};
use crate::repository::pos::{PosRepository, PosRepositoryTrait};
use crate::response::pos::{
    Bond, BondStatus, MergedBond, Reward, Unbond, ValidatorWithId, Withdraw,
};

#[derive(Clone)]
pub struct PosService {
    pos_repo: PosRepository,
    chain_repo: ChainRepository,
}

impl PosService {
    pub fn new(app_state: AppState) -> Self {
        Self {
            pos_repo: PosRepository::new(app_state.clone()),
            chain_repo: ChainRepository::new(app_state),
        }
    }

    pub async fn get_validators(
        &self,
        page: u64,
        states: Vec<ValidatorStateDto>,
        sort_field: Option<ValidatorSortFieldDto>,
        sort_order: Option<OrderByDto>,
    ) -> Result<(Vec<ValidatorWithId>, u64, u64), PoSError> {
        let validator_states = states
            .into_iter()
            .map(Self::to_validator_state_db)
            .collect();
        let validator_sort_by = sort_field.map(|field| {
            let order = sort_order.unwrap_or(OrderByDto::Asc);
            Self::to_validator_sort_by_db(field, order)
        });
        let (db_validators, total_pages, total_items) = self
            .pos_repo
            .find_validators(page as i64, validator_states, validator_sort_by)
            .await
            .map_err(PoSError::Database)?;

        let validators_rank = self
            .pos_repo
            .get_validators_rank()
            .await
            .map_err(PoSError::Database)?;

        let validators = db_validators
            .into_iter()
            .map(|v| {
                let rank = validators_rank
                    .iter()
                    .position(|v_id| v_id == &v.id)
                    .map(|r| (r + 1) as i32);
                ValidatorWithId::from(v, rank)
            })
            .collect();

        Ok((validators, total_pages as u64, total_items as u64))
    }

    pub async fn get_all_validators(
        &self,
        states: Vec<ValidatorStateDto>,
    ) -> Result<Vec<ValidatorWithId>, PoSError> {
        let validator_states = states
            .into_iter()
            .map(Self::to_validator_state_db)
            .collect();
        let db_validators = self
            .pos_repo
            .find_all_validators(validator_states)
            .await
            .map_err(PoSError::Database)?;
        let validators_rank = self
            .pos_repo
            .get_validators_rank()
            .await
            .map_err(PoSError::Database)?;
        let validators = db_validators
            .into_iter()
            .map(|v| {
                let rank = validators_rank
                    .iter()
                    .position(|v_id| v_id == &v.id)
                    .map(|r| (r + 1) as i32);
                ValidatorWithId::from(v, rank)
            })
            .collect();

        Ok(validators)
    }

    pub async fn get_bonds_by_address(
        &self,
        address: String,
        page: u64,
    ) -> Result<(Vec<Bond>, u64, u64), PoSError> {
        let pos_state = self
            .pos_repo
            .get_state()
            .await
            .map_err(PoSError::Database)?;

        let (db_bonds, total_pages, total_items) = self
            .pos_repo
            .find_bonds_by_address(address, page as i64)
            .await
            .map_err(PoSError::Database)?;

        let bonds: Vec<Bond> = db_bonds
            .into_iter()
            .map(|(validator, bond)| {
                let bond_status = BondStatus::from((&bond, &pos_state));
                Bond::from(bond, bond_status, validator)
            })
            .collect();

        Ok((bonds, total_pages as u64, total_items as u64))
    }

    pub async fn get_merged_bonds_by_address(
        &self,
        address: String,
        page: u64,
    ) -> Result<(Vec<MergedBond>, u64, u64), PoSError> {
        let (db_bonds, total_pages, total_items) = self
            .pos_repo
            .find_merged_bonds_by_address(address, page as i64)
            .await
            .map_err(PoSError::Database)?;

        let bonds: Vec<MergedBond> = db_bonds
            .into_iter()
            .map(|(_, validator, amount)| {
                MergedBond::from(
                    amount.unwrap_or(BigDecimal::zero()),
                    validator,
                )
            })
            .collect();

        Ok((bonds, total_pages as u64, total_items as u64))
    }

    pub async fn get_unbonds_by_address(
        &self,
        address: String,
        page: u64,
    ) -> Result<(Vec<Unbond>, u64, u64), PoSError> {
        let (db_unbonds, total_pages, total_items) = self
            .pos_repo
            .find_unbonds_by_address(address, page as i64)
            .await
            .map_err(PoSError::Database)?;

        let chain_state = self
            .chain_repo
            .get_state()
            .await
            .map_err(PoSError::Database)?;

        let parameters = self
            .chain_repo
            .find_chain_parameters()
            .await
            .map_err(PoSError::Database)?;

        let unbonds: Vec<Unbond> = db_unbonds
            .into_iter()
            .map(|(validator, unbond)| {
                Unbond::from(
                    unbond.raw_amount,
                    unbond.withdraw_epoch,
                    validator,
                    &chain_state,
                    parameters.max_block_time,
                    parameters.min_duration,
                )
            })
            .collect();

        Ok((unbonds, total_pages as u64, total_items as u64))
    }

    pub async fn get_merged_unbonds_by_address(
        &self,
        address: String,
        page: u64,
    ) -> Result<(Vec<Unbond>, u64, u64), PoSError> {
        let chain_state = self
            .chain_repo
            .get_state()
            .await
            .map_err(PoSError::Database)?;

        let pos_state = self
            .pos_repo
            .get_state()
            .await
            .map_err(PoSError::Database)?;

        let (db_merged_unbonds, total_pages, total_items) = self
            .pos_repo
            .find_merged_unbonds_by_address(
                address,
                pos_state.last_processed_epoch,
                page as i64,
            )
            .await
            .map_err(PoSError::Database)?;

        let parameters = self
            .chain_repo
            .find_chain_parameters()
            .await
            .map_err(PoSError::Database)?;

        let unbonds: Vec<Unbond> = db_merged_unbonds
            .into_iter()
            .map(|(_, validator, raw_amount, withdraw_epoch)| {
                Unbond::from(
                    raw_amount.unwrap_or(BigDecimal::zero()),
                    withdraw_epoch,
                    validator,
                    &chain_state,
                    parameters.max_block_time,
                    parameters.min_duration,
                )
            })
            .collect();

        Ok((unbonds, total_pages as u64, total_items as u64))
    }

    pub async fn get_withdraws_by_address(
        &self,
        address: String,
        epoch: Option<u64>,
        page: u64,
    ) -> Result<(Vec<Withdraw>, u64, u64), PoSError> {
        let epoch = if let Some(epoch) = epoch {
            epoch as i32
        } else {
            self.chain_repo
                .get_state()
                .await
                .map_err(PoSError::Database)?
                .last_processed_epoch
        };

        let (db_withdraws, total_pages, total_items) = self
            .pos_repo
            .find_withdraws_by_address(address, epoch, page as i64)
            .await
            .map_err(PoSError::Database)?;

        let withdraws: Vec<Withdraw> = db_withdraws
            .into_iter()
            .map(|(validator, withdraw)| Withdraw::from(withdraw, validator))
            .collect();

        Ok((withdraws, total_pages as u64, total_items as u64))
    }

    pub async fn get_rewards_by_address(
        &self,
        address: String,
    ) -> Result<Vec<Reward>, PoSError> {
        // TODO: could optimize and make a single query
        let db_rewards = self
            .pos_repo
            .find_rewards_by_address(address)
            .await
            .map_err(PoSError::Database)?;

        let mut rewards = vec![];
        for db_reward in db_rewards {
            let db_validator = self
                .pos_repo
                .find_validator_by_id(db_reward.validator_id)
                .await;
            if let Ok(Some(db_validator)) = db_validator {
                rewards.push(Reward::from(db_reward.clone(), db_validator));
            } else {
                tracing::error!(
                    "Couldn't find validator with id {} in bond query",
                    db_reward.validator_id
                );
            }
        }

        Ok(rewards)
    }

    // TODO: maybe return object(struct) instead
    pub async fn get_total_voting_power(&self) -> Result<u64, PoSError> {
        let total_voting_power_db = self
            .pos_repo
            .get_total_voting_power()
            .await
            .map_err(PoSError::Database)?;

        Ok(total_voting_power_db.unwrap_or_default() as u64)
    }

    fn to_validator_state_db(value: ValidatorStateDto) -> ValidatorStateDb {
        match value {
            ValidatorStateDto::Consensus => ValidatorStateDb::Consensus,
            ValidatorStateDto::BelowCapacity => ValidatorStateDb::BelowCapacity,
            ValidatorStateDto::BelowThreshold => {
                ValidatorStateDb::BelowThreshold
            }
            ValidatorStateDto::Inactive => ValidatorStateDb::Inactive,
            ValidatorStateDto::Jailed => ValidatorStateDb::Jailed,
            ValidatorStateDto::Unknown => ValidatorStateDb::Unknown,
        }
    }

    fn to_validator_sort_by_db(
        field: ValidatorSortFieldDto,
        order: OrderByDto,
    ) -> (ValidatorSortByDb, OrderByDb) {
        (
            match field {
                ValidatorSortFieldDto::VotingPower => {
                    ValidatorSortByDb::VotingPower
                }
                ValidatorSortFieldDto::Commission => {
                    ValidatorSortByDb::Commission
                }
                ValidatorSortFieldDto::Rank => ValidatorSortByDb::Rank,
            },
            match order {
                OrderByDto::Asc => OrderByDb::Asc,
                OrderByDto::Desc => OrderByDb::Desc,
            },
        )
    }
}
