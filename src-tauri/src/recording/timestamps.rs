pub fn qpc_to_100ns(counter: i64, frequency: i64) -> i64 {
    if frequency <= 0 {
        return 0;
    }
    (counter * 10_000_000) / frequency
}
