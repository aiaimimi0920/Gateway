use super::*;
use serde_json::json;

fn answers(addresses: &[&str]) -> serde_json::Value {
    json!({"Status":0, "TC":false, "Question":[{"name":"status.example.com.", "type":1}],
        "Answer":addresses.iter().map(|ip| json!({"type":1,"data":ip})).collect::<Vec<_>>()})
}

#[test]
fn only_pure_fake_dns_results_for_public_names_trigger_recovery() {
    let fake: Vec<SocketAddr> = ["198.18.0.1:443", "198.19.255.255:443"]
        .iter()
        .map(|value| value.parse().unwrap())
        .collect();
    assert!(recoverable("status.example.com", &fake));
    for host in [
        "198.18.0.1",
        "localhost",
        "printer",
        "printer.local",
        "x.home.arpa",
        "X.INTERNAL.",
    ] {
        assert!(!recoverable(host, &fake), "{host}");
    }
    assert!(!recoverable("status.example.com", &[]));
    assert!(!recoverable("status.example.com", &vec![fake[0]; 17]));
    for other in [
        "127.0.0.1:443",
        "169.254.169.254:443",
        "10.0.0.1:443",
        "8.8.8.8:443",
    ] {
        assert!(!recoverable(
            "status.example.com",
            &[fake[0], other.parse().unwrap()]
        ));
    }
    assert!(fake_ip("::ffff:198.18.0.1".parse().unwrap()));
    assert!(!fake_ip("198.20.0.1".parse().unwrap()));
}

#[test]
fn public_dns_pins_all_addresses_and_preserves_the_requested_port() {
    let mut body = answers(&["1.1.1.1", "8.8.8.8"]);
    body["Answer"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({"type":5,"data":"cdn.example.com."}));
    let result = parse(&body, "status.example.com", 8443, 1).unwrap();
    assert_eq!(
        result,
        vec![
            "1.1.1.1:8443".parse().unwrap(),
            "8.8.8.8:8443".parse().unwrap()
        ]
    );
    let body = json!({"Status":0,"TC":false,"Question":[{"name":"status.example.com.","type":28}],
        "Answer":[{"type":28,"data":"2606:4700:4700::1111"}]});
    assert_eq!(
        parse(&body, "status.example.com", 443, 28).unwrap().len(),
        1
    );
}

#[test]
fn resolver_cannot_smuggle_private_metadata_or_fake_addresses() {
    for ip in [
        "127.0.0.1",
        "10.0.0.1",
        "169.254.169.254",
        "100.100.100.200",
        "198.18.0.1",
        "198.19.0.1",
        "0.0.0.0",
        "::ffff:169.254.169.254",
    ] {
        assert!(
            parse(&answers(&["1.1.1.1", ip]), "status.example.com", 443, 1).is_err(),
            "{ip}"
        );
    }
    assert!(parse(&answers(&["1.1.1.1"; 17]), "status.example.com", 443, 1).is_err());
}

#[test]
fn malformed_or_mismatched_dns_fails_closed() {
    for (field, value) in [
        ("Status", json!(3)),
        ("TC", json!(true)),
        ("Answer", json!({})),
        ("Question", json!([])),
    ] {
        let mut body = answers(&["1.1.1.1"]);
        body[field] = value;
        assert!(
            parse(&body, "status.example.com", 443, 1).is_err(),
            "{field}"
        );
    }
    assert!(parse(&answers(&["1.1.1.1"]), "other.example.com", 443, 1).is_err());
    assert!(parse(&answers(&["1.1.1.1"]), "status.example.com", 443, 28).is_err());
    assert!(parse(&answers(&["bad-ip"]), "status.example.com", 443, 1).is_err());
    let mut body = answers(&[]);
    body["Answer"] = json!([{"type":16,"data":"not-an-address"}]);
    assert!(parse(&body, "status.example.com", 443, 1).is_err());
    for (kind, record_type, ip) in [(1, 28, "1.1.1.1"), (28, 1, "2606:4700:4700::1111")] {
        let body = json!({"Status":0,"TC":false,"Question":[{"name":"status.example.com.","type":kind}],
            "Answer":[{"type":record_type,"data":ip}]});
        assert!(parse(&body, "status.example.com", 443, kind).is_err());
    }
}
