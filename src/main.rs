use solana_client::{rpc_client::RpcClient, tpu_client::{TpuClient, TpuClientConfig}};
use solana_sdk::{transaction::Transaction, message::Message, signature::Keypair, signer::Signer, hash::Hash, commitment_config::CommitmentConfig};
use dotenv::dotenv;
use std::thread;
use solana_transaction_status::UiTransactionEncoding;

fn main() {
    println!("load env variables");
    dotenv().ok();

    let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set.");
    let ws_url = std::env::var("WS_URL").expect("WS_URL must be set.");
    let pvt_key = std::env::var("PVT_KEY").expect("PVT_KEY must be set.");
    
    let signer = Keypair::from_base58_string(&pvt_key);
    let rpc_client = RpcClient::new(&rpc_url);

    // build tpu client
    println!("building tpu client");
    let tpu_client = TpuClient::new(
        RpcClient::new(&rpc_url).into(),
        &ws_url,
        TpuClientConfig::default()).unwrap();
    // warm up tpu client
    println!("warm up tpu client");
    for i in 0..3 {
        tpu_client.send_transaction(&build_test_tx(&rpc_client.get_latest_blockhash().unwrap(), &signer,  &format!("quic warm up {}", i)));
    }

    println!("building test txs");
    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let [rpc_tx, quic_tx] = [
        build_test_tx(&recent_blockhash, &signer, "rpc_"),
        build_test_tx(&recent_blockhash, &signer, "quic")
    ];
    let rpc_sig = rpc_tx.signatures[0];
    let quic_sig = quic_tx.signatures[0];
    println!("+ rpc signature: {:#?}", rpc_sig);
    println!("+ quic signature: {:#?}", quic_sig);

    // send txs async in 2 threads
    let rpc_thread = thread::spawn(move || {
        println!("send tx via rpc");
        let _ = rpc_client.send_transaction(&rpc_tx);
    });
    let quic_thread = thread::spawn(move || {
        println!("send tx via quic");
        tpu_client.send_transaction(&quic_tx);
    });

    // join threads
    let _ = quic_thread.join();
    let _ = rpc_thread.join();

    let rpc_client2 = RpcClient::new_with_commitment(&rpc_url, CommitmentConfig {
        ..CommitmentConfig::finalized()
    });

    // poll rpc for txs confrimations
    println!("poll txs confirmations");
    let _ = rpc_client2.poll_for_signature(&rpc_sig);
    let _ = rpc_client2.poll_for_signature(&quic_sig);
    
    println!("get sigs statuses");
    let sig_statuses = rpc_client2.get_signature_statuses( &[rpc_sig, quic_sig]).unwrap().value;

    let rpc_tx_slot = if let Some(r) = &sig_statuses[0] {
        r.slot
    } else { 0 };

    let quic_tx_slot = if let Some(r) = &sig_statuses[1] {
        r.slot
    } else { 0 };

    if rpc_tx_slot != quic_tx_slot {
        println!("rpc_tx_slot: {:}", rpc_tx_slot);
        println!("quic_tx_slot: {:}", quic_tx_slot);
    } else {
        let executed_slot = quic_tx_slot;
        
        println!("both txs executed in slot: {:}", executed_slot);

        match rpc_client2.get_block_with_encoding(executed_slot, UiTransactionEncoding::Binary) {
            Ok(block_info) => {
                let mut rpc_tx_index: usize = 0;
                let mut quic_tx_index = 0;
                
                for (i, t) in block_info.transactions.iter().enumerate() {

                    if let Some(decoded_tx) = t.transaction.decode() {
                        if decoded_tx.signatures[0] == rpc_sig {
                            rpc_tx_index = i;
                        }
                        if decoded_tx.signatures[0] == quic_sig {
                            quic_tx_index = i;
                        }
                    }
                }
        
                println!("+ rpc  tx_index in slot {:}/{:}", rpc_tx_index, block_info.transactions.len());
                println!("+ quic tx_index in slot {:}/{:}", quic_tx_index, block_info.transactions.len());
            },
            _ => println!("error fetching block")
       }
    }

}

// fn print_signer_balance(rpc_client: &RpcClient, signer: &Keypair) {
//     let signer_balance = rpc_client.get_balance(&signer.pubkey());
//     match signer_balance {
//         Ok(b) => *println!("signer {:?} balance: {} lamports", signer.pubkey(), b),
//         _ => println!("error fetching signer balance")
//     };
// }

fn build_test_tx(recent_blockhash: &Hash, signer: &Keypair, str: &str) -> Transaction {
    Transaction::new(
        &[signer],
        Message::new(
            &[spl_memo::build_memo(str.as_bytes(), &[&signer.pubkey()])],
            Some(&signer.pubkey()),
        ),
        *recent_blockhash)
}