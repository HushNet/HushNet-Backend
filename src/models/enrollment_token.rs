use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnrollmentClaims {
    pub sub: String,
    pub exp: usize,
}

pub struct UsedToken {
    pub token: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrollment_claims_construction() {
        let claims = EnrollmentClaims {
            sub: String::from("user123"),
            exp: 1234567890,
        };
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.exp, 1234567890);
    }
}
