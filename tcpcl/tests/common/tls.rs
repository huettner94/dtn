use openssl::{
    asn1::Asn1Time,
    hash::MessageDigest,
    nid::Nid,
    pkey::{PKey, Private},
    rsa::Rsa,
    x509::{X509Extension, X509Name, X509},
};

fn get_cert_with_san(sanname: &str) -> (PKey<Private>, X509) {
    let cert_rsa = Rsa::generate(2048).unwrap();
    let pkey = PKey::from_rsa(cert_rsa).unwrap();

    let mut name = X509Name::builder().unwrap();
    name.append_entry_by_nid(Nid::COMMONNAME, "nobody_cares")
        .unwrap();
    let name = name.build();

    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder.set_subject_name(&name).unwrap();
    builder.set_issuer_name(&name).unwrap();

    #[allow(deprecated)] // Depending on https://github.com/sfackler/rust-openssl/issues/1911 to fix
    let subject_alternative_name = X509Extension::new_nid(
        None,
        Some(&builder.x509v3_context(None, None)),
        Nid::SUBJECT_ALT_NAME,
        &format!("otherName:1.3.6.1.5.5.7.8.11;IA5STRING:{}", sanname),
    )
    .unwrap();
    builder.append_extension(subject_alternative_name).unwrap();

    builder
        .set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    builder
        .set_not_after(&Asn1Time::days_from_now(365).unwrap())
        .unwrap();
    builder.set_pubkey(&pkey).unwrap();
    builder.sign(&pkey, MessageDigest::sha256()).unwrap();
    let x509 = builder.build();

    (pkey, x509)
}

#[allow(dead_code)]
pub fn get_server_cert() -> (PKey<Private>, X509) {
    get_cert_with_san("dtn://server")
}

#[allow(dead_code)]
pub fn get_client_cert() -> (PKey<Private>, X509) {
    get_cert_with_san("dtn://client")
}
