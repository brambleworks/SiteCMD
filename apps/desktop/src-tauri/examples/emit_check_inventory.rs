//! Prints the web check inventory as JSON; `check-inventory.json` is its
//! committed output and the product-facts generator reads that file.
fn main() {
    let ids = app_lib::checks::inventory::web_check_ids();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "web": ids })).expect("inventory json")
    );
}
