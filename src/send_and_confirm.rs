use std::{io::Write, time::Duration};

use colored::*;
use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult}, nonblocking::rpc_client::RpcClient, rpc_config::{RpcSendTransactionConfig, RpcSimulateTransactionAccountsConfig, RpcSimulateTransactionConfig}
};
use solana_program::{
    instruction::Instruction,
    native_token::{lamports_to_sol, sol_to_lamports},
};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    compute_budget::ComputeBudgetInstruction,
    signature::{Signature, Signer},
    transaction::Transaction,
};
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};

use crate::Miner;

const MIN_SOL_BALANCE: f64 = 0.005;

const RPC_RETRIES: usize = 0;
const _SIMULATION_RETRIES: usize = 4;
const GATEWAY_RETRIES: usize = usize::MAX-1;
const CONFIRM_RETRIES: usize = usize::MAX-1;
#[derive(Debug)]
pub enum ComputeBudget {
    Dynamic,
    Fixed(u32),
}

const NONCE_RENT: u64 = 1_447_680;

pub struct NonceManager {
    pub rpc_client: std::sync::Arc<RpcClient>,
    pub authority: solana_sdk::pubkey::Pubkey,
    pub capacity: u64,
    pub idx: u64,
}

impl NonceManager {
    pub fn new(rpc_client: std::sync::Arc<RpcClient>, authority: solana_sdk::pubkey::Pubkey, capacity: u64) -> Self {
        NonceManager {
            rpc_client,
            authority,
            capacity,
            idx: 0,
        }
    }

    pub async fn try_init_all(&mut self, payer: &solana_sdk::signer::keypair::Keypair) -> Vec<Result<Signature, solana_client::client_error::ClientError>> {
        let (blockhash, _) = self.rpc_client
            .get_latest_blockhash_with_commitment(CommitmentConfig::finalized()).await
            .unwrap_or_default();
        let mut sigs = vec![];
        for _ in 0..self.capacity {
            let nonce_account = self.next();
            let ixs = self.maybe_create_ixs(&nonce_account.pubkey()).await;
            if ixs.is_none() {
                continue;
            }
            let ixs = ixs.unwrap();
            let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[&payer, &nonce_account], blockhash);
            sigs.push(self.rpc_client.send_transaction(&tx).await);
        }
        sigs
    }

    fn next_seed(&mut self) -> u64 {
        let ret = self.idx;
        self.idx = (self.idx + 1) % self.capacity;
        ret
    }

    pub fn next(&mut self) -> solana_sdk::signer::keypair::Keypair {
        let seed = format!("Nonce:{}:{}", self.authority.clone(), self.next_seed());
        let seed = sha256::digest(seed.as_bytes());
        let kp = solana_sdk::signer::keypair::keypair_from_seed(&seed.as_ref()).unwrap();
        kp
    }

    pub async fn maybe_create_ixs(&mut self, nonce: &solana_sdk::pubkey::Pubkey) -> Option<Vec<Instruction>> {
        if solana_client::nonce_utils::nonblocking::get_account(&self.rpc_client, nonce).await.is_ok() {
            None
        } else {
            Some(solana_sdk::system_instruction::create_nonce_account(
                    &self.authority,
                    &nonce,
                    &self.authority,
                    NONCE_RENT,
            ))
        }
    }
}
impl Miner {
    pub async fn send_and_confirm(
        &self,
        ixs: &[Instruction],
        compute_budget: ComputeBudget,
        _skip_confirm: bool,
    ) -> ClientResult<Signature> {
        let signer = self.signer();
        let client =  std::sync::Arc::new(RpcClient::new_with_commitment(self.rpc_client.url(), CommitmentConfig::confirmed()));
        let mut nonce_manager = NonceManager::new(client.clone(), signer.pubkey(), 10 as u64);
            nonce_manager.try_init_all(&signer).await; 

            nonce_manager.try_init_all(&signer).await; 


            nonce_manager.try_init_all(&signer).await; 


            nonce_manager.try_init_all(&signer).await; 


            nonce_manager.try_init_all(&signer).await; 



        // Return error, if balance is zero
        if let Ok(balance) = client.get_balance(&signer.pubkey()).await {
            if balance <= sol_to_lamports(MIN_SOL_BALANCE) {
                panic!(
                    "{} Insufficient balance: {} SOL\nPlease top up with at least {} SOL",
                    "ERROR".bold().red(),
                    lamports_to_sol(balance),
                    MIN_SOL_BALANCE
                );
            }
        }

        // Set compute units
        let mut final_ixs = vec![];
        let mut cus = 0;
        match compute_budget {
            ComputeBudget::Dynamic => {
                // TODO simulate? and now; magick
                let mut to_sim = final_ixs.clone();
                to_sim.push(ComputeBudgetInstruction::set_compute_unit_price(
                    self.priority_fee,
                ));
                to_sim.extend_from_slice(ixs);

                        
                // Now you can use this sim_cfg in your simulate_transaction_with_config call
                let blockhash = client.get_latest_blockhash().await.unwrap();
                let signers = &[&signer  as &dyn Signer];
                let tx = Transaction::new_signed_with_payer(&to_sim, Some(&signer.pubkey()), signers, blockhash);
                cus = client.simulate_transaction(&tx).await.unwrap().value.units_consumed.unwrap();
                if cus == 0 {
                    cus = 1_400_000;
                }
                final_ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(cus as u32));

            }
            ComputeBudget::Fixed(cus) => {
                final_ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(cus))
            }
        }
        final_ixs.push(ComputeBudgetInstruction::set_compute_unit_price(
            self.priority_fee,
        ));
        final_ixs.extend_from_slice(ixs);

        // Build tx
        let send_cfg = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(RPC_RETRIES),
            min_context_slot: None,
        };
        
       let msg = solana_sdk::message::Message::new_with_nonce( 
        ixs.to_vec(),
        Some(&signer.pubkey()), 
            &nonce_manager.next().pubkey(), 
            &signer.pubkey());

        let mut tx = Transaction::new_unsigned(msg.clone());

        // Sign tx
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();
        tx.sign(&[&signer], hash);
        let mut attempts = 0;

        let res = client.send_transaction_with_config(&tx, send_cfg.clone()).await;
        match res {
            Ok(sig) => {

             let client_clone = client.clone();
                    let sig_clone = sig;
                    let tx_clone = tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::confirm_transaction(client_clone, &mut vec![sig_clone], tx_clone).await {
                            println!("Background confirmation error: {:?}", e);
                        }
                    });
                    Ok(sig)
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    attempts += 1;
                    if attempts.gt(&GATEWAY_RETRIES) {
                        return Err(ClientError {
                            request: None,
                            kind: ClientErrorKind::Custom("Max retries".into()),
                        });
                    }
                    println!("Transaction did not land");
                    return Err(err);
                }
            }
    }
async fn confirm_transaction(client: std::sync::Arc<RpcClient>, sigs : &mut Vec<Signature>,
tx: Transaction
) -> Result<(), ClientError> {
    let mut attempts = 0;
    loop {
        // Use async sleep to delay without blocking
       tokio::time::sleep(Duration::from_secs((1.1*attempts as f64) as u64)).await;
        println!("Checking transaction statuses {:?}", sigs);
        match client.get_signature_statuses(&sigs).await {
            Ok(statuses) => {
                // Process the statuses to check if the transaction is confirmed...
                for status in statuses.value.iter() {
                    if let Some(status) = status {
                        if let Some(confirmation_status) = &status.confirmation_status {
                            match confirmation_status {
                                TransactionConfirmationStatus::Confirmed
                                | TransactionConfirmationStatus::Finalized => {
                                    println!("---!");
                                    println!("Transaction confirmed!");
                                    println!("---!");
                                    println!("---!");
                                    // append it to file, appending txs.csv
                                    // append
                                    let mut file = std::fs::OpenOptions::new()
                                        .append(true)
                                        .create(true)
                                        .open("txs.csv")
                                        .unwrap();
                                    file.write_all(format!("{:?}\n", tx.signatures).as_bytes()).unwrap();




                                    return Ok(());
                                },
                                _ => {
                                    println!("Transaction not confirmed yet...");
                                    sigs.push(client.send_transaction(&tx.clone()).await?);
                                }
                            }
                        }
                    }
                }
            },
            Err(err) => {
                println!("Error checking transaction status: {:?}", err);
            }
 }

        attempts += 1;
        let _ = client.send_transaction(&tx.clone()).await;

        println!("Confirmation attempts: {:?}", attempts);
        if attempts >= CONFIRM_RETRIES as u64 {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom("Confirmation attempts exceeded".into()),
            });
        }
    }
}
    // TODO
    fn _simulate(&self) {

        // Simulate tx
        // let mut sim_attempts = 0;
        // 'simulate: loop {
        //     let sim_res = client
        //         .simulate_transaction_with_config(
        //             &tx,
        //             RpcSimulateTransactionConfig {
        //                 sig_verify: false,
        //                 replace_recent_blockhash: true,
        //                 commitment: Some(self.rpc_client.commitment()),
        //                 encoding: Some(UiTransactionEncoding::Base64),
        //                 accounts: None,
        //                 min_context_slot: Some(slot),
        //                 inner_instructions: false,
        //             },
        //         )
        //         .await;
        //     match sim_res {
        //         Ok(sim_res) => {
        //             if let Some(err) = sim_res.value.err {
        //                 println!("Simulaton error: {:?}", err);
        //                 sim_attempts += 1;
        //             } else if let Some(units_consumed) = sim_res.value.units_consumed {
        //                 if dynamic_cus {
        //                     println!("Dynamic CUs: {:?}", units_consumed);
        //                     let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(
        //                         units_consumed as u32 + 1000,
        //                     );
        //                     let cu_price_ix =
        //                         ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
        //                     let mut final_ixs = vec![];
        //                     final_ixs.extend_from_slice(&[cu_budget_ix, cu_price_ix]);
        //                     final_ixs.extend_from_slice(ixs);
        //                     tx = Transaction::new_with_payer(&final_ixs, Some(&signer.pubkey()));
        //                 }
        //                 break 'simulate;
        //             }
        //         }
        //         Err(err) => {
        //             println!("Simulaton error: {:?}", err);
        //             sim_attempts += 1;
        //         }
        //     }

        //     // Abort if sim fails
        //     if sim_attempts.gt(&SIMULATION_RETRIES) {
        //         return Err(ClientError {
        //             request: None,
        //             kind: ClientErrorKind::Custom("Simulation failed".into()),
        //         });
        //     }
        // }
    }
}
