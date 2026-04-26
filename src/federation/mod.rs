pub mod client;
pub mod outbox;

/// Parse a federated user address into its local and node components.
///
/// "alice@node-a.hushnet.net" → ("alice", "node-a.hushnet.net")
///
/// Uses rfind('@') so that a username containing '@' (unlikely but possible)
/// is tolerated: the rightmost '@' is taken as the domain separator.
/// Returns None if the address contains no '@'.
pub fn parse_federated_address(addr: &str) -> Option<(&str, &str)> {
    let at = addr.rfind('@')?;
    Some((&addr[..at], &addr[at + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normal_address() {
        let (user, node) = parse_federated_address("alice@node-a.hushnet.net").unwrap();
        assert_eq!(user, "alice");
        assert_eq!(node, "node-a.hushnet.net");
    }

    #[test]
    fn parse_missing_at() {
        assert!(parse_federated_address("alice").is_none());
    }

    #[test]
    fn parse_rightmost_at() {
        // degenerate case: username itself contains '@'
        let (user, node) = parse_federated_address("a@b@node-a.hushnet.net").unwrap();
        assert_eq!(user, "a@b");
        assert_eq!(node, "node-a.hushnet.net");
    }
}
