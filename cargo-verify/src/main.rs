use anyhow::Result;

fn main() -> Result<()> {
    // --monomorphize?
    // --no-ops-to-function-calls?
    // --raw-boxes?
    charon::main_(["charon", "cargo", "--translate-all-methods"])
}
