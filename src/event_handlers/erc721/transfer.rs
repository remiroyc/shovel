use crate::{
    common::{starknet_constants::ZERO_FELT, types::CairoUint256},
    db::{
        collection::{ContractMetadataCollectionInterface, Erc721CollectionInterface},
        document::{ContractMetadata, Erc721, TokenMetadata},
    },
    event_handlers::context::Event,
    rpc::metadata::{contract, token},
};
use color_eyre::eyre::Result;
use mongodb::{ClientSession, Collection};
use starknet::{
    core::types::FieldElement,
    providers::jsonrpc::{models::BlockId, HttpTransport, JsonRpcClient},
};

pub async fn run(event_context: &Event<'_, '_>, session: &mut ClientSession) -> Result<()> {
    let contract_address = event_context.contract_address();
    let block_id = event_context.block_id();
    let block_number = event_context.block_number();
    let event_data = event_context.data();
    let db = event_context.db();
    let rpc = event_context.rpc();

    let sender = event_data[0];
    let recipient = event_data[1];
    let token_id = CairoUint256::new(event_data[2], *event_data.get(3).unwrap_or(&ZERO_FELT));

    let erc721_collection = db.collection::<Erc721>("erc721_tokens");
    let contract_metadata_collection = db.collection::<ContractMetadata>("contract_metadata");

    if sender == ZERO_FELT {
        handle_mint(
            contract_address,
            &block_id,
            block_number,
            recipient,
            token_id,
            rpc,
            &erc721_collection,
            &contract_metadata_collection,
            session,
        )
        .await
    } else if recipient == ZERO_FELT {
        handle_burn(contract_address, block_number, sender, token_id, &erc721_collection, session)
            .await
    } else {
        handle_transfer(
            contract_address,
            block_number,
            sender,
            recipient,
            token_id,
            &erc721_collection,
            session,
        )
        .await
    }
}

async fn handle_mint(
    contract_address: FieldElement,
    block_id: &BlockId,
    block_number: u64,
    recipient: FieldElement,
    token_id: CairoUint256,
    rpc: &JsonRpcClient<HttpTransport>,
    erc721_collection: &Collection<Erc721>,
    contract_metadata_collection: &Collection<ContractMetadata>,
    session: &mut ClientSession,
) -> Result<()> {
    let token_uri = token::get_erc721_uri(contract_address, block_id, rpc, token_id).await;
    let metadata_result = token::get_token_metadata(&token_uri).await;
    let metadata = match metadata_result {
        Ok(metadata) => metadata,
        Err(_) => TokenMetadata::default(),
    };

    let erc721_token =
        Erc721::new(contract_address, token_id, recipient, token_uri, metadata, block_number);

    erc721_collection.insert_erc721(erc721_token, session).await?;

    let contract_metadata_exists =
        contract_metadata_collection.contract_metadata_exists(contract_address, session).await?;

    if !contract_metadata_exists {
        let name = contract::get_name(contract_address, block_id, rpc).await;
        let symbol = contract::get_symbol(contract_address, block_id, rpc).await;
        let contract_metadata = ContractMetadata::new(contract_address, name, symbol, block_number);
        contract_metadata_collection.insert_contract_metadata(contract_metadata, session).await?;
    }

    Ok(())
}

async fn handle_burn(
    contract_address: FieldElement,
    block_number: u64,
    sender: FieldElement,
    token_id: CairoUint256,
    erc721_collection: &Collection<Erc721>,
    session: &mut ClientSession,
) -> Result<()> {
    erc721_collection
        .update_erc721_owner(contract_address, token_id, sender, ZERO_FELT, block_number, session)
        .await
}

async fn handle_transfer(
    contract_address: FieldElement,
    block_number: u64,
    sender: FieldElement,
    recipient: FieldElement,
    token_id: CairoUint256,
    erc721_collection: &Collection<Erc721>,
    session: &mut ClientSession,
) -> Result<()> {
    erc721_collection
        .update_erc721_owner(contract_address, token_id, sender, recipient, block_number, session)
        .await
}
