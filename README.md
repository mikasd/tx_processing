csv reader/writer
expects one argument, input file path
outputs to stdout

example run command
```
cargo run -- transactions.csv > accounts.csv
```

expects the following headers format for CSV input
```
type,client,tx,amount
``` 

type: String, 
client: u16 Optional,
tx: u32, Required
amount: f32, Optional

CSV Reader is NOT flexible in number of columns per row, but does handle null value on optional types.
Comments within the input file are not currently supported but can be added per request and discussion on standard comment formatting.

Possible improvements that this could make:
Explore multithreaded approach to handle concurrency as a requirement
Improve readability of code and reduce verbosity
Break into modules for easier maintnence and workspace management
Write good tests instead of solely relying on language features to verify performance
This implementation does not handle duplicate transaction IDs. This feature could be added but was not as the test doc did not cover that case. I can add this on request.
Implementation would be a Hashmap with tx id as key, with a txid History for any dispute/resolution/general history verification specific to one txId, such as duplicate transaction attempts with the same id. 
The current implementation verifies that the tx_id to be disputed exists as it relates to a client when a dispute is initially processed and when a transcation request for a resolution is executed.
