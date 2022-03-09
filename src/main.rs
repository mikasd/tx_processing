use csv::Trim;
use log::error;
use rand::{prelude::ThreadRng, Rng};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, error::Error, ffi::OsString, io, process};

#[derive(Debug, Deserialize, Clone)]
struct Record {
    #[serde(rename = "type")]
    tx_type: String,
    #[serde(deserialize_with = "csv::invalid_option")]
    client: Option<u16>,
    tx: u32,
    #[serde(deserialize_with = "csv::invalid_option")]
    amount: Option<f32>,
}

struct ClientInfo {
    history: Vec<Record>,
    available_funds: f32,
    held_funds: f32,
    total_funds: f32,
    locked: bool,
}

#[derive(Serialize, Debug)]
struct OutputInfo {
    client: u16,
    available: f32,
    held: f32,
    total: f32,
    locked: bool,
}

fn main() {
    if let Err(err) = run() {
        error!("{}", err);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut client_map: HashMap<u16, ClientInfo> = HashMap::new();

    let file_path = get_first_arg()?;

    let mut reader = csv::ReaderBuilder::new()
        .trim(Trim::All)
        .from_path(file_path)?;

    for result in reader.deserialize() {
        let mut record: Record = result?;
        // if recorded transaction does not have a client id provided, generate a new one
        if record.client == None {
            record.client = generate_new_client_id(&mut client_map);
        }
        match record.tx_type.as_str() {
            "deposit" => handle_deposit(&mut client_map, record),
            "withdrawal" => handle_widthdrawal(&mut client_map, record),
            "dispute" => handle_dispute(&mut client_map, record),
            "resolve" => handle_resolve(&mut client_map, record),
            "chargeback" => handle_chargeback(&mut client_map, record),
            _ => {
                // this should be logged/sent into some secondary transaction validation queue for further review
                error!(
                    "transaction type not specified in tx number: {:?}",
                    record.tx
                )
            }
        }
    }

    let mut wtr = csv::Writer::from_writer(io::stdout());

    for (k, v) in client_map.iter() {
        wtr.serialize(OutputInfo {
            client: *k,
            available: v.available_funds,
            held: v.held_funds,
            total: v.total_funds,
            locked: v.locked,
        })?;
    }

    wtr.flush()?;
    Ok(())
}

fn gen_random_id(rng: &mut ThreadRng) -> u16 {
    rng.gen()
}

fn generate_new_client_id(client_map: &mut HashMap<u16, ClientInfo>) -> Option<u16> {
    let mut rng = rand::thread_rng();
    // attempt to generate random new id
    let mut new_id = gen_random_id(&mut rng);
    // if client map already contains randomly generated value, generate a new one until you find a unique value
    while client_map.contains_key(&new_id) {
        new_id = gen_random_id(&mut rng);
    }
    // once unique value reached, return it
    Some(new_id)
}

/// Returns the first positional argument sent to this process. If there are no
/// positional arguments, then this returns an error.
fn get_first_arg() -> Result<OsString, Box<dyn Error>> {
    match env::args_os().nth(1) {
        None => Err(From::from("File path for input CSV expected.")),
        Some(file_path) => Ok(file_path),
    }
}

fn handle_chargeback(client_map: &mut HashMap<u16, ClientInfo>, record: Record) {
    if let Some(client_id) = &record.client {
        // is client has transacted so far
        if let Some(current_client_info) = client_map.get_mut(client_id) {
            if !current_client_info.locked {
                let history = &current_client_info.history;
                let tx_to_resolve = &history.iter().find(|&r| r.tx == record.tx);
                if let Some(tx) = tx_to_resolve {
                    let chargeback_amount = tx.amount;
                    if let Some(val) = chargeback_amount {
                        current_client_info.total_funds -= val;
                        current_client_info.held_funds -= val;
                    } else {
                        error!("chargeback amount value not found")
                    }
                    // lock account after chargeback
                    current_client_info.locked = true;
                } else {
                    // transaction to dispute not found
                    error!("tx id: {:} not found handle errors here", record.tx);
                }
            } else {
                error!(
                    "locked account id: {:} attempted chargeback, handle errors here",
                    &client_id
                );
            }
        } else {
            //client has no recorded transactions
            error!("Client has no transactions to chargeback on {:?}", record);
        }
    }
}

fn handle_resolve(client_map: &mut HashMap<u16, ClientInfo>, record: Record) {
    if let Some(client_id) = &record.client {
        // is client has transacted so far
        if let Some(current_client_info) = client_map.get_mut(client_id) {
            if !current_client_info.locked {
                let history = &current_client_info.history;
                // this will sometimes find the transaction request for the dispute which might not have a value field.
                let tx_to_resolve = &history
                    .iter()
                    .find(|&r| r.tx == record.tx && r.tx_type != "dispute");
                if let Some(tx) = tx_to_resolve {
                    let resolved_amount = tx.amount;
                    if let Some(amt) = resolved_amount {
                        current_client_info.available_funds += amt;
                        current_client_info.held_funds -= amt;
                    } else {
                        error!("resolved amount not found");
                    }
                    current_client_info.history.push(record);
                } else {
                    // transaction to dispute not found
                    error!("Tx ID: not found {:} in handle resolve", record.tx,);
                }
            } else {
                // TODO
                error!(
                    "locked account attempted to resolve transaction resolve {:?}",
                    record
                );
            }
        } else {
            // no client id found w that info
            error!(
                "Client ID: {:} not found while processing resolve tx request",
                client_id,
            );
        }
    }
}

fn handle_dispute(client_map: &mut HashMap<u16, ClientInfo>, record: Record) {
    if let Some(client_id) = &record.client {
        // is client has transacted so far
        if let Some(current_client_info) = client_map.get_mut(client_id) {
            if !current_client_info.locked {
                let history = &current_client_info.history;
                let tx_to_dispute = &history.iter().find(|&r| r.tx == record.tx);
                if let Some(tx) = tx_to_dispute {
                    let disputed_amount = tx.amount;

                    if let Some(amount) = disputed_amount {
                        current_client_info.available_funds -= amount;
                        current_client_info.held_funds += amount;
                    } else {
                        error!("disputed amount not found");
                    }

                    current_client_info.history.push(record);
                } else {
                    // transaction to dispute not found
                    error!(
                        "Tx ID: not found {:} within historical transactions while processing dispute",
                        record.tx
                    );
                }
            } else {
                // TODO
                error!("locked account attempted dispute {:?}", record);
            }
        } else {
            // no client id found w that info
            error!(
                "Client ID: {:} not found in client map, handle errors here {:?}",
                client_id, &record
            );
        }
    }
}

fn handle_deposit(client_map: &mut HashMap<u16, ClientInfo>, record: Record) {
    if let Some(client_id) = &record.client {
        // is client has transacted so far
        if let Some(current_client_info) = client_map.get_mut(client_id) {
            if !current_client_info.locked {
                if let Some(value) = record.amount {
                    current_client_info.available_funds += value;
                    current_client_info.total_funds += value;
                } else {
                    error!("deposit value not provided, balances not modified");
                }
                // push to history anyways to save tx
                current_client_info.history.push(record);
            } else {
                // handle locked account
                error!(
                    "Locked account with id: {:} attempted deposit {:?}",
                    client_id, &record
                );
            }
        } else {
            // else, first tx with that id, set up initial history
            let mut new_info: ClientInfo = ClientInfo {
                history: Vec::new(),
                available_funds: 0.0,
                held_funds: 0.0,
                total_funds: 0.0,
                locked: false,
            };
            if let Some(value) = record.amount {
                new_info.available_funds += value;
                new_info.total_funds += value;
            } else {
                error!("no amount provided in transaction")
            }
            // push tx to history of client id regardless of amount being present
            new_info.history.push(record.clone());
            // insert value into client map to track client activity
            client_map.insert(*client_id, new_info);
        }
    }
}

fn handle_widthdrawal(client_map: &mut HashMap<u16, ClientInfo>, record: Record) {
    if let Some(client_id) = &record.client {
        // is client has transacted so far
        if let Some(current_client_info) = client_map.get_mut(client_id) {
            if !current_client_info.locked {
                if let Some(amount) = record.amount {
                    if amount <= current_client_info.available_funds {
                        current_client_info.available_funds -= amount;
                        current_client_info.total_funds -= amount;
                    } else {
                        error!("OVERDRAFT: Client ID: {:?}, attempted to withdraw more funds than available {:?}", client_id, record);
                    }
                } else {
                    error!("amount not provided for withdrawal tx {:?}", record);
                }
                // add tx to client history
                current_client_info.history.push(record);
            } else {
                // TODO
                error!(
                    "locked account with id: {:} attempted withdrawal {:?}, handle errors here",
                    client_id, record
                );
            }
        } else {
            // first tx with that id, set up initial history
            // log withdrawl attempt
            error!(
                "Client Id without history attempted withdrawl, logging client id and attempt {:?}",
                record
            );
            let mut new_info: ClientInfo = ClientInfo {
                history: Vec::new(),
                available_funds: 0.0,
                held_funds: 0.0,
                total_funds: 0.0,
                locked: false,
            };
            new_info.history.push(record.clone());
            client_map.insert(*client_id, new_info);
        }
    }
}
