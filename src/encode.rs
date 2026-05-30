const CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
pub fn encode(id: u64) -> String {
    let mut quotient = id;
    let mut remainders = Vec::new();
    while quotient >= 62 {
        remainders.push(quotient % 62);
        quotient = quotient / 62;
    }

    remainders.push(quotient);

    remainders.reverse();

    let encoded_val: Vec<char> = remainders
        .iter()
        .map(|f| CHARSET[*f as usize] as char)
        .collect();

    encoded_val.into_iter().collect()
}

pub fn decode(encoded_str: &str) -> Option<u64> {
    encoded_str.chars().try_fold(0u64, |acc, c| {
        let pos = CHARSET.iter().position(|&p| p == c as u8)? as u64;
        Some(acc * 62 + pos)
    })
}
