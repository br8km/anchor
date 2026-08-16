use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use url::Url;
use urlencoding::{decode, encode};

type HmacSha1 = Hmac<Sha1>;

pub fn canonicalize_input(input: &str, default_label: Option<&str>) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("TOTP input is required");
    }

    if trimmed.starts_with("otpauth://") {
        return canonicalize_uri(trimmed);
    }

    let label = default_label.context("a label is required for raw TOTP seeds")?;
    let secret = normalize_secret(trimmed)?;
    Ok(build_uri(label, None, &secret, 6, 30))
}

pub fn canonicalize_uri(input: &str) -> Result<String> {
    parse_uri(input).map(|parsed| parsed.canonical_uri())
}

pub fn validate_uri(input: &str) -> Result<()> {
    canonicalize_uri(input).map(|_| ())
}

pub fn current_code(uri: &str) -> Result<String> {
    code_at(uri, SystemTime::now())
}

pub fn code_at(uri: &str, now: SystemTime) -> Result<String> {
    let parsed = parse_uri(uri)?;
    let since_epoch = now
        .duration_since(UNIX_EPOCH)
        .context("system time is before the Unix epoch")?;
    parsed.code_at(since_epoch.as_secs())
}

struct ParsedTotp {
    label: String,
    issuer: Option<String>,
    secret: Vec<u8>,
    digits: u32,
    period: u64,
}

impl ParsedTotp {
    fn canonical_uri(&self) -> String {
        build_uri(
            &self.label,
            self.issuer.as_deref(),
            &self.secret,
            self.digits,
            self.period,
        )
    }

    fn code_at(&self, unix_seconds: u64) -> Result<String> {
        if self.period == 0 {
            bail!("TOTP period must be positive");
        }

        let counter = unix_seconds / self.period;
        let mut mac = HmacSha1::new_from_slice(&self.secret)
            .map_err(|_| anyhow!("failed to initialize TOTP HMAC"))?;
        mac.update(&counter.to_be_bytes());
        let hash = mac.finalize().into_bytes();
        let offset = (hash[19] & 0x0f) as usize;
        let binary = ((u32::from(hash[offset]) & 0x7f) << 24)
            | ((u32::from(hash[offset + 1]) & 0xff) << 16)
            | ((u32::from(hash[offset + 2]) & 0xff) << 8)
            | (u32::from(hash[offset + 3]) & 0xff);
        let modulus = 10u64.pow(self.digits);
        let code = u64::from(binary) % modulus;
        Ok(format!("{code:0width$}", width = self.digits as usize))
    }
}

fn parse_uri(input: &str) -> Result<ParsedTotp> {
    let url = Url::parse(input).with_context(|| format!("invalid TOTP URI: {input}"))?;

    if url.scheme() != "otpauth" {
        bail!("TOTP URI must use the otpauth scheme");
    }

    if !url
        .host_str()
        .map(|host| host.eq_ignore_ascii_case("totp"))
        .unwrap_or(false)
    {
        bail!("only TOTP URIs are supported");
    }

    let label = {
        let segments = url.path_segments().context("TOTP URI is missing a label")?;
        let label = segments
            .filter(|segment| !segment.is_empty())
            .map(|segment| {
                decode(segment)
                    .map(|label| label.into_owned())
                    .map_err(|err| anyhow!("invalid TOTP label: {err}"))
            })
            .collect::<Result<Vec<_>>>()?
            .join("/");

        if label.is_empty() {
            bail!("TOTP URI is missing a label");
        }

        label
    };

    let mut params = std::collections::BTreeMap::new();
    for (key, value) in url.query_pairs() {
        let key = key.into_owned();
        let value = value.into_owned();
        if params.insert(key.clone(), value).is_some() {
            bail!("duplicate TOTP query parameter: {key}");
        }
    }

    let secret = params
        .remove("secret")
        .context("TOTP URI is missing a secret")?;
    let secret = normalize_secret(&secret)?;

    let issuer = params.remove("issuer").filter(|issuer| !issuer.is_empty());

    let algorithm = params
        .remove("algorithm")
        .unwrap_or_else(|| "SHA1".to_string());
    if !algorithm.eq_ignore_ascii_case("SHA1") {
        bail!("only SHA1 TOTP URIs are supported");
    }

    let digits = parse_positive_u32(params.remove("digits").as_deref(), 6, "digits")?;
    let period = parse_positive_u64(params.remove("period").as_deref(), 30, "period")?;

    if let Some(extra) = params.keys().next().cloned() {
        bail!("unsupported TOTP query parameter: {extra}");
    }

    Ok(ParsedTotp {
        label,
        issuer,
        secret,
        digits,
        period,
    })
}

fn parse_positive_u32(value: Option<&str>, default: u32, name: &str) -> Result<u32> {
    let value = value.unwrap_or("");
    if value.is_empty() {
        return Ok(default);
    }

    let parsed = value
        .parse::<u32>()
        .with_context(|| format!("{name} must be a positive integer"))?;
    if parsed == 0 {
        bail!("{name} must be positive");
    }
    Ok(parsed)
}

fn parse_positive_u64(value: Option<&str>, default: u64, name: &str) -> Result<u64> {
    let value = value.unwrap_or("");
    if value.is_empty() {
        return Ok(default);
    }

    let parsed = value
        .parse::<u64>()
        .with_context(|| format!("{name} must be a positive integer"))?;
    if parsed == 0 {
        bail!("{name} must be positive");
    }
    Ok(parsed)
}

fn normalize_secret(secret: &str) -> Result<Vec<u8>> {
    let cleaned = secret
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '=')
        .collect::<String>()
        .to_ascii_uppercase();

    if cleaned.is_empty() {
        bail!("TOTP secret is required");
    }

    BASE32_NOPAD
        .decode(cleaned.as_bytes())
        .map_err(|err| anyhow!("invalid TOTP secret: {err}"))
}

fn build_uri(label: &str, issuer: Option<&str>, secret: &[u8], digits: u32, period: u64) -> String {
    let mut uri = format!(
        "otpauth://totp/{}?secret={}&algorithm=SHA1&digits={digits}&period={period}",
        encode(label),
        BASE32_NOPAD.encode(secret),
    );

    if let Some(issuer) = issuer {
        uri.push_str("&issuer=");
        uri.push_str(&encode(issuer));
    }

    uri
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_encoding::BASE32_NOPAD;
    use std::time::Duration;

    #[test]
    fn canonicalize_raw_seed_uses_default_label() {
        let uri =
            canonicalize_input("jbswy3dpehpk3pxp", Some("services/email")).expect("canonical uri");
        assert_eq!(
            uri,
            "otpauth://totp/services%2Femail?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30"
        );
    }

    #[test]
    fn canonicalize_uri_normalizes_secret_and_query_values() {
        let uri = canonicalize_uri(
            "otpauth://totp/services%2Femail?period=30&digits=6&secret=jbswy3dpehpk3pxp",
        )
        .expect("canonical uri");
        assert_eq!(
            uri,
            "otpauth://totp/services%2Femail?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30"
        );
    }

    #[test]
    fn validate_uri_rejects_hotp() {
        let err = validate_uri("otpauth://hotp/account?secret=JBSWY3DPEHPK3PXP")
            .expect_err("hotp should be rejected");
        assert!(err.to_string().contains("only TOTP"));
    }

    #[test]
    fn code_at_matches_rfc_6238_vectors() {
        let secret = BASE32_NOPAD.encode(b"12345678901234567890");
        let uri =
            format!("otpauth://totp/Example?secret={secret}&algorithm=SHA1&digits=8&period=30");

        let cases = [
            (59, "94287082"),
            (1_111_111_109, "07081804"),
            (1_111_111_111, "14050471"),
            (1_234_567_890, "89005924"),
            (2_000_000_000, "69279037"),
        ];

        for (unix_seconds, expected) in cases {
            let code =
                code_at(&uri, UNIX_EPOCH + Duration::from_secs(unix_seconds)).expect("totp code");
            assert_eq!(code, expected);
        }
    }
}
