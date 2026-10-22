//! What hosts announce over mDNS: a browse of the common service types alongside the sweep,
//! and what each host's announcements say: which services it offers, and often its model and
//! maker in the TXT records (`ty=Brother HL-L2350DW series`, `md=Chromecast`, Apple's
//! `model=MacBookPro18,3`).

use std::net::IpAddr;
use std::time::Duration;

use netscli_core::{COMMON_SERVICE_TYPES, MdnsEngine, MdnsService};

/// How long the browse listens. Devices re-announce every few seconds; the sweep of a /24
/// takes about as long, so this rarely makes a scan longer.
const BROWSE: Duration = Duration::from_secs(3);

/// TXT keys that name a device's model, most specific first: printers' `ty` and `product`,
/// Apple's `model`, Google Cast's `md`.
const MODEL_KEYS: [&str; 4] = ["ty", "model", "md", "product"];
/// TXT keys that name its maker, as printers give it.
const MAKER_KEYS: [&str; 3] = ["usb_mfg", "manufacturer", "mfg"];

/// Every service announced on the local network during the browse.
pub(super) async fn browse() -> Vec<MdnsService> {
    // A failed browse (no multicast, say) means no announcements were heard, which is what
    // the hosts are then shown with.
    let heard = MdnsEngine::discover(COMMON_SERVICE_TYPES, BROWSE).await;
    heard.unwrap_or_default()
}

/// The announcements of the host at `ip`.
pub(super) fn of_host(all: &[MdnsService], ip: IpAddr) -> Vec<MdnsService> {
    let own = all.iter().filter(|s| s.addresses.contains(&ip));
    own.cloned().collect()
}

/// The service types announced, short (`_ipp._tcp.local.` is `ipp`), sorted, each once.
pub(super) fn service_names(announced: &[MdnsService]) -> Vec<String> {
    let mut names: Vec<String> = announced
        .iter()
        .filter_map(|s| s.service_type.strip_prefix('_')?.split('.').next())
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_graphic()))
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The model the announcements name, if any; untrusted text, to be cleaned.
pub(super) fn model(announced: &[MdnsService]) -> Option<&str> {
    first_value(announced, &MODEL_KEYS)
}

/// The maker the announcements name, if any; untrusted text, to be cleaned.
pub(super) fn maker(announced: &[MdnsService]) -> Option<&str> {
    first_value(announced, &MAKER_KEYS)
}

/// The first non-empty value of the first of `keys` any announcement has, keys compared
/// ignoring case. Printers wrap `product` in brackets, which are dropped.
fn first_value<'a>(announced: &'a [MdnsService], keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        announced.iter().find_map(|service| {
            let (_, value) = service
                .properties
                .iter()
                .find(|(k, v)| k.eq_ignore_ascii_case(key) && !v.trim().is_empty())?;
            Some(value.trim().trim_start_matches('(').trim_end_matches(')'))
        })
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn service(service_type: &str, ip: &str, properties: &[(&str, &str)]) -> MdnsService {
        MdnsService {
            full_name: format!("Host.{service_type}"),
            hostname: "Host.local.".into(),
            service_type: service_type.into(),
            addresses: vec![ip.parse().unwrap()],
            port: 80,
            properties: properties
                .iter()
                .map(|&(k, v)| (k.to_owned(), v.to_owned()))
                .collect::<HashMap<_, _>>(),
        }
    }

    fn printer() -> Vec<MdnsService> {
        vec![
            service("_http._tcp.local.", "192.168.1.30", &[]),
            service(
                "_ipp._tcp.local.",
                "192.168.1.30",
                &[
                    ("product", "(HL-L2350DW series)"),
                    ("ty", "Brother HL-L2350DW series"),
                    ("usb_MFG", "Brother"),
                ],
            ),
            service(
                "_googlecast._tcp.local.",
                "192.168.1.50",
                &[("md", "Chromecast")],
            ),
        ]
    }

    #[test]
    fn a_hosts_announcements_are_its_own() {
        let all = printer();
        let ip = "192.168.1.30".parse().unwrap();
        let own = of_host(&all, ip);
        assert_eq!(service_names(&own), ["http", "ipp"]);
        let none = of_host(&all, "192.168.1.99".parse().unwrap());
        assert_eq!(service_names(&none), Vec::<String>::new());
    }

    #[test]
    fn txt_records_name_the_model_and_maker() {
        let all = printer();
        let own = of_host(&all, "192.168.1.30".parse().unwrap());
        assert_eq!(model(&own), Some("Brother HL-L2350DW series"), "ty first");
        assert_eq!(maker(&own), Some("Brother"), "keys ignore case");
        let cast = of_host(&all, "192.168.1.50".parse().unwrap());
        assert_eq!((model(&cast), maker(&cast)), (Some("Chromecast"), None));
    }

    #[test]
    fn a_bracketed_product_loses_its_brackets() {
        let own = [service(
            "_ipp._tcp.local.",
            "10.0.0.2",
            &[("product", "(LaserJet)")],
        )];
        assert_eq!(model(&own), Some("LaserJet"));
    }
}
