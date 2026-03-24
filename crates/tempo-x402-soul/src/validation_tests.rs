
#[cfg(test)]
mod tests {
    use crate::validation::run_consistency_check;

    #[test]
    fn test_system_consistency() {
        let result = run_consistency_check();
        assert!(result.is_ok(), "Consistency check should pass with 42");
    }
}
