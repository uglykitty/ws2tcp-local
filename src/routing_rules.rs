use std::collections::HashSet;

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use tracing::{info, warn};

const GFWLIST_URL: &str = "https://gitlab.com/gfwlist/gfwlist/raw/master/gfwlist.txt";

#[derive(Debug, Clone)]
pub(crate) enum RoutingRules {
    Domains(DomainRules),
    AllProxy,
}

impl RoutingRules {
    pub(crate) async fn load() -> Self {
        match Self::download_and_parse().await {
            Ok(rules) => {
                info!(
                    url = GFWLIST_URL,
                    domain_count = rules.len(),
                    "loaded proxy routing rules"
                );
                Self::Domains(rules)
            }
            Err(err) => {
                warn!(
                    url = GFWLIST_URL,
                    error = %format_args!("{err:#}"),
                    "failed to load proxy routing rules; proxying all domains"
                );
                Self::AllProxy
            }
        }
    }

    pub(crate) fn should_proxy_host(&self, host: &str) -> bool {
        match self {
            Self::Domains(rules) => rules.matches(host),
            Self::AllProxy => true,
        }
    }

    fn mode(&self) -> &'static str {
        match self {
            Self::Domains(_) => "gfwlist",
            Self::AllProxy => "all-proxy",
        }
    }

    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Domains(rules) => format!("{} domains from {}", rules.len(), GFWLIST_URL),
            Self::AllProxy => format!("all domains via proxy; failed to load {GFWLIST_URL}"),
        }
    }

    async fn download_and_parse() -> Result<DomainRules> {
        let response = reqwest::get(GFWLIST_URL)
            .await
            .with_context(|| format!("failed to download {GFWLIST_URL}"))?
            .error_for_status()
            .with_context(|| format!("failed to download {GFWLIST_URL}"))?;
        let body = response
            .bytes()
            .await
            .context("failed to read gfwlist response body")?;

        parse_gfwlist(&body)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DomainRules {
    domains: HashSet<String>,
}

impl DomainRules {
    fn new(domains: HashSet<String>) -> Result<Self> {
        if domains.is_empty() {
            bail!("gfwlist did not contain any usable domain rules");
        }

        Ok(Self { domains })
    }

    fn len(&self) -> usize {
        self.domains.len()
    }

    fn matches(&self, host: &str) -> bool {
        let host = normalize_host_for_match(host);
        if host.is_empty() {
            return false;
        }

        if self.domains.contains(&host) {
            return true;
        }

        host.match_indices('.')
            .any(|(idx, _)| self.domains.contains(&host[idx + 1..]))
    }
}

fn parse_gfwlist(encoded: &[u8]) -> Result<DomainRules> {
    let compact: Vec<u8> = encoded
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let decoded = STANDARD
        .decode(compact)
        .context("failed to decode gfwlist")?;
    let text = String::from_utf8(decoded).context("decoded gfwlist is not valid UTF-8")?;
    parse_gfwlist_text(&text)
}

fn parse_gfwlist_text(text: &str) -> Result<DomainRules> {
    let domains = text
        .lines()
        .filter_map(parse_proxy_rule_domain)
        .collect::<HashSet<_>>();

    DomainRules::new(domains)
}

fn parse_proxy_rule_domain(line: &str) -> Option<String> {
    let rule = line.strip_prefix("||").or_else(|| line.strip_prefix('.'))?;
    if rule.contains('*') {
        return None;
    }

    let domain = rule
        .split(['/', '^', '$'])
        .next()
        .unwrap_or_default()
        .trim_matches('.');
    let domain = normalize_host_for_match(domain);
    is_domain_like(&domain).then_some(domain)
}

fn normalize_host_for_match(host: &str) -> String {
    host.trim().trim_matches('.').to_ascii_lowercase()
}

fn is_domain_like(domain: &str) -> bool {
    if domain.is_empty() || domain.contains(':') || domain.parse::<std::net::IpAddr>().is_ok() {
        return false;
    }

    domain
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
}

pub(crate) fn host_from_authority(authority: &str) -> Result<&str> {
    if let Some(rest) = authority.strip_prefix('[') {
        return rest
            .split_once("]:")
            .map(|(host, _)| host)
            .ok_or_else(|| anyhow!("IPv6 authority must be [host]:port"));
    }

    authority
        .rsplit_once(':')
        .map(|(host, _)| host)
        .ok_or_else(|| anyhow!("authority must include :port"))
}

impl std::fmt::Display for RoutingRules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.mode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;

    #[test]
    fn parses_gfwlist_domain_rules() {
        let text = "\
! comment
||example.com
||example.net/path
||example.org^
||example.edu$third-party
||wild*.blocked.test
.leading-dot.example
|http://ignored.example
";
        let encoded = STANDARD.encode(text);
        let rules = parse_gfwlist(encoded.as_bytes()).unwrap();

        assert!(rules.matches("example.com"));
        assert!(rules.matches("www.example.com"));
        assert!(rules.matches("example.net"));
        assert!(rules.matches("a.example.org"));
        assert!(rules.matches("example.edu"));
        assert!(rules.matches("www.leading-dot.example"));
        assert!(!rules.matches("wild.blocked.test"));
        assert!(!rules.matches("ignored.example"));
    }

    #[test]
    fn matches_case_insensitively_and_on_suffix_boundary() {
        let rules = parse_gfwlist_text("||example.com\n").unwrap();

        assert!(rules.matches("WWW.Example.Com."));
        assert!(!rules.matches("badexample.com"));
    }

    #[test]
    fn extracts_host_from_authority() {
        assert_eq!(
            host_from_authority("example.com:443").unwrap(),
            "example.com"
        );
        assert_eq!(
            host_from_authority("[2001:db8::1]:443").unwrap(),
            "2001:db8::1"
        );
    }
}
