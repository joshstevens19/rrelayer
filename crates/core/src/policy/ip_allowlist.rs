use std::net::IpAddr;

use ipnet::IpNet;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum IpAllowlistError {
    #[error("Invalid IP/CIDR rule `{0}`")]
    InvalidRule(String),
}

fn rule_matches(client_ip: &IpAddr, rule: &str) -> Result<bool, IpAllowlistError> {
    let rule = rule.trim();
    if rule.is_empty() {
        return Err(IpAllowlistError::InvalidRule(rule.to_string()));
    }

    if let Ok(net) = rule.parse::<IpNet>() {
        return Ok(net.contains(client_ip));
    }
    if let Ok(ip) = rule.parse::<IpAddr>() {
        return Ok(&ip == client_ip);
    }

    Err(IpAllowlistError::InvalidRule(rule.to_string()))
}

pub fn validate_ip_allowlist(rules: &[String]) -> Result<(), IpAllowlistError> {
    let sentinel = IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);
    for rule in rules {
        rule_matches(&sentinel, rule)?;
    }
    Ok(())
}

/// Returns false for an explicitly empty list and errors for malformed rules.
pub fn ip_allowed(client_ip: &IpAddr, rules: &[String]) -> Result<bool, IpAllowlistError> {
    for rule in rules {
        if rule_matches(client_ip, rule)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        rule: String,
        valid: bool,
        matching_ip: Option<String>,
    }

    #[test]
    fn shared_rule_fixtures_define_the_authoritative_grammar() {
        let fixtures: Vec<Fixture> =
            serde_json::from_str(include_str!("../../../../fixtures/request-policy-ip-rules.json"))
                .unwrap();

        for fixture in fixtures {
            let validation = validate_ip_allowlist(std::slice::from_ref(&fixture.rule));
            assert_eq!(validation.is_ok(), fixture.valid, "rule: {:?}", fixture.rule);
            if let Some(matching_ip) = fixture.matching_ip {
                assert!(
                    ip_allowed(&matching_ip.parse().unwrap(), std::slice::from_ref(&fixture.rule))
                        .unwrap(),
                    "rule {:?} must contain {matching_ip}",
                    fixture.rule
                );
            }
        }
    }

    #[test]
    fn explicit_empty_rules_deny_every_source() {
        assert!(!ip_allowed(&"10.0.0.1".parse().unwrap(), &[]).unwrap());
    }
}
