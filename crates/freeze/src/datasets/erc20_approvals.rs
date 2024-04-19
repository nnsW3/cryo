use crate::*;
use ethers::prelude::*;
use polars::prelude::*;

/// columns for transactions
#[cryo_to_df::to_df(Datatype::Erc20Approvals)]
#[derive(Default)]
pub struct Erc20Approvals {
    n_rows: u64,
    block_number: Vec<u32>,
    block_hash: Vec<Option<Vec<u8>>>,
    transaction_index: Vec<u32>,
    log_index: Vec<u32>,
    transaction_hash: Vec<Vec<u8>>,
    erc20: Vec<Vec<u8>>,
    from_address: Vec<Vec<u8>>,
    to_address: Vec<Vec<u8>>,
    value: Vec<U256>,
    chain_id: Vec<u64>,
}

#[async_trait::async_trait]
impl Dataset for Erc20Approvals {
    fn default_columns() -> Option<Vec<&'static str>> {
        Some(vec![
            "block_number",
            // "block_hash",
            "transaction_index",
            "log_index",
            "transaction_hash",
            "erc20",
            "from_address",
            "to_address",
            "value",
            "chain_id",
        ])
    }

    fn optional_parameters() -> Vec<Dim> {
        vec![Dim::Address, Dim::Topic0, Dim::Topic1, Dim::Topic2, Dim::FromAddress, Dim::ToAddress]
    }

    fn use_block_ranges() -> bool {
        true
    }

    fn arg_aliases() -> Option<std::collections::HashMap<Dim, Dim>> {
        Some([(Dim::Contract, Dim::Address)].into_iter().collect())
    }
}

#[async_trait::async_trait]
impl CollectByBlock for Erc20Approvals {
    type Response = Vec<Log>;

    async fn extract(request: Params, source: Arc<Source>, _: Arc<Query>) -> R<Self::Response> {
        let mut topics = [Some(ValueOrArray::Value(Some(*EVENT_ERC20_TRANSFER))), None, None, None];
        if let Some(from_address) = &request.from_address {
            let mut v = vec![0u8; 12];
            v.append(&mut from_address.to_owned());
            topics[1] = Some(ValueOrArray::Value(Some(H256::from_slice(&v[..]))));
        }
        if let Some(to_address) = &request.to_address {
            let mut v = vec![0u8; 12];
            v.append(&mut to_address.to_owned());
            topics[2] = Some(ValueOrArray::Value(Some(H256::from_slice(&v[..]))));
        }
        let filter = Filter { topics, ..request.ethers_log_filter()? };
        let logs = source.get_logs(&filter).await?;

        Ok(logs.into_iter().filter(|x| x.topics.len() == 3 && x.data.len() == 32).collect())
    }

    fn transform(response: Self::Response, columns: &mut Self, query: &Arc<Query>) -> R<()> {
        let schema = query.schemas.get_schema(&Datatype::Erc20Approvals)?;
        process_erc20_approval(response, columns, schema)
    }
}

#[async_trait::async_trait]
impl CollectByTransaction for Erc20Approvals {
    type Response = Vec<Log>;

    async fn extract(request: Params, source: Arc<Source>, _: Arc<Query>) -> R<Self::Response> {
        let logs = source.get_transaction_logs(request.transaction_hash()?).await?;
        Ok(logs.into_iter().filter(is_erc20_approval).collect())
    }
}

fn is_erc20_approval(log: &Log) -> bool {
    log.topics.len() == 3 && log.data.len() == 32 && log.topics[0] == *EVENT_ERC20_APPROVAL
}

fn process_erc20_approval(logs: Vec<Log>, columns: &mut Erc20Approvals, schema: &Table) -> R<()> {
    for log in logs.iter() {
        if let (Some(bn), Some(tx), Some(ti), Some(li)) =
            (log.block_number, log.transaction_hash, log.transaction_index, log.log_index)
        {
            columns.n_rows += 1;
            store!(schema, columns, block_number, bn.as_u32());
            store!(schema, columns, block_hash, log.block_hash.map(|bh| bh.as_bytes().to_vec()));
            store!(schema, columns, transaction_index, ti.as_u32());
            store!(schema, columns, log_index, li.as_u32());
            store!(schema, columns, transaction_hash, tx.as_bytes().to_vec());
            store!(schema, columns, erc20, log.address.as_bytes().to_vec());
            store!(schema, columns, from_address, log.topics[1].as_bytes()[12..].to_vec());
            store!(schema, columns, to_address, log.topics[2].as_bytes()[12..].to_vec());
            store!(schema, columns, value, log.data.to_vec().as_slice().into());
        }
    }
    Ok(())
}
