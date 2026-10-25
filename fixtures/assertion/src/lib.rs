pub fn expected_failure() {
    flux_rs::assert(false); // CI must identify this assertion's diagnostic.
}
