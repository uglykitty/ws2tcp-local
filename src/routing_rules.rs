use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use tracing::{info, warn};

const GFWLIST_URL: &str = "https://gitlab.com/gfwlist/gfwlist/raw/master/gfwlist.txt";

#[derive(Debug, Clone)]
pub(crate) enum RoutingRules {
    Domains {
        rules: DomainRules,
        custom_domain_rules: Option<PathBuf>,
    },
    AllProxy,
}

impl RoutingRules {
    pub(crate) async fn load(custom_domain_rules: Option<&Path>) -> Self {
        match Self::download_and_parse(custom_domain_rules).await {
            Ok(rules) => {
                info!(
                    url = GFWLIST_URL,
                    custom_domain_rules =
                        custom_domain_rules.map(|path| path.display().to_string()),
                    domain_count = rules.len(),
                    "loaded proxy routing rules"
                );
                Self::Domains {
                    rules,
                    custom_domain_rules: custom_domain_rules.map(Path::to_path_buf),
                }
            }
            Err(err) => {
                warn!(
                    url = GFWLIST_URL,
                    custom_domain_rules = custom_domain_rules.map(|path| path.display().to_string()),
                    error = %format_args!("{err:#}"),
                    "failed to load proxy routing rules; proxying all domains"
                );
                Self::AllProxy
            }
        }
    }

    pub(crate) fn should_proxy_host(&self, host: &str) -> bool {
        match self {
            Self::Domains { rules, .. } => rules.matches(host),
            Self::AllProxy => true,
        }
    }

    fn mode(&self) -> &'static str {
        match self {
            Self::Domains { .. } => "gfwlist",
            Self::AllProxy => "all-proxy",
        }
    }

    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Domains {
                rules,
                custom_domain_rules: Some(path),
            } => format!(
                "{} domains from {} plus custom rules from {}",
                rules.len(),
                GFWLIST_URL,
                path.display()
            ),
            Self::Domains {
                rules,
                custom_domain_rules: None,
            } => format!("{} domains from {}", rules.len(), GFWLIST_URL),
            Self::AllProxy => format!("all domains via proxy; failed to load {GFWLIST_URL}"),
        }
    }

    async fn download_and_parse(custom_domain_rules: Option<&Path>) -> Result<DomainRules> {
        let response = reqwest::get(GFWLIST_URL)
            .await
            .with_context(|| format!("failed to download {GFWLIST_URL}"))?
            .error_for_status()
            .with_context(|| format!("failed to download {GFWLIST_URL}"))?;
        let body = response
            .bytes()
            .await
            .context("failed to read gfwlist response body")?;

        let mut rules = parse_gfwlist(&body)?;
        if let Some(path) = custom_domain_rules {
            let custom_domains = read_custom_domain_rules(path)?;
            let custom_count = custom_domains.len();
            rules.extend(custom_domains);
            info!(
                path = %path.display(),
                custom_domain_count = custom_count,
                "merged custom proxy routing rules"
            );
        }

        Ok(rules)
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

    fn extend(&mut self, domains: HashSet<String>) {
        self.domains.extend(domains);
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

fn read_custom_domain_rules(path: &Path) -> Result<HashSet<String>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read custom domain rules {}", path.display()))?;
    Ok(parse_custom_domain_rules_text(&text))
}

fn parse_custom_domain_rules_text(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(parse_custom_domain_rule_domain)
        .collect()
}

fn parse_custom_domain_rule_domain(line: &str) -> Option<String> {
    let rule = line.split('#').next().unwrap_or_default().trim();
    let domain = normalize_host_for_match(rule);
    is_domain_like(&domain).then_some(domain)
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
    fn parses_custom_domain_rules() {
        let domains = parse_custom_domain_rules_text(
            "\
# One Squid dstdomain entry per line.
.paypal.com
.www.paypal.com

.googleadservices.com # inline comment
127.0.0.1
bad:domain
",
        );
        let rules = DomainRules::new(domains).unwrap();

        assert!(rules.matches("paypal.com"));
        assert!(rules.matches("checkout.paypal.com"));
        assert!(rules.matches("www.paypal.com"));
        assert!(rules.matches("pagead.googleadservices.com"));
        assert!(!rules.matches("127.0.0.1"));
        assert!(!rules.matches("bad:domain"));
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
