use binascii::hex2bin;
use dtn::bp7::{bundle::Bundle, Validate};

const  TEST: &str = "9f88071a00020004008201702f2f6e6f646533312f6d61766c696e6b8201702f2f6e6f6465322f696e636f6d696e678201702f2f6e6f6465322f696e636f6d696e67821b0000009e9de3defe001a0036ee80850a020000448218200085010100004443414243ff";

fn main() {
    let mut val = [0; TEST.len() / 2];
    hex2bin(TEST.as_bytes(), &mut val).unwrap();

    let recovered: Bundle = serde_cbor::from_slice(&val).unwrap();
    println!("{:?}", &recovered);
    println!("{}", recovered.validate());

    let reserialized = serde_cbor::to_vec(&recovered).unwrap();
    let mut outstr = format!("{:02X?}", &reserialized);
    outstr = outstr.replace(|p| p == ' ' || p == ',', "");
    println!("{}", outstr);
    println!("{}", reserialized == val);

    let recovered2: Bundle = serde_cbor::from_slice(&reserialized).unwrap();
    println!("{:?}", recovered2);
}
