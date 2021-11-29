use openssl::{
    asn1::Asn1Time,
    hash::MessageDigest,
    nid::Nid,
    pkey::{PKey, Private},
    rsa::Rsa,
    x509::{extension::SubjectAlternativeName, X509Name, X509},
};

fn get_cert_with_san(sanname: &str) -> (PKey<Private>, X509) {
    let cert_rsa = Rsa::generate(1024).unwrap();
    let pkey = PKey::from_rsa(cert_rsa).unwrap();

    let mut name = X509Name::builder().unwrap();
    name.append_entry_by_nid(Nid::COMMONNAME, "nobody_cares")
        .unwrap();
    let name = name.build();

    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder.set_subject_name(&name).unwrap();
    builder.set_issuer_name(&name).unwrap();

    let subject_alternative_name = SubjectAlternativeName::new()
        .other_name(&format!("1.3.6.1.5.5.7.8.11;IA5STRING:{}", sanname))
        .build(&builder.x509v3_context(None, None))
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
