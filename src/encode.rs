const CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
pub fn encode(id: u64) -> String {
    let mut quotient = id;
    let mut remainders = Vec::new();
    while quotient >= 62 {
        remainders.push(quotient % 62);
        quotient /= 62;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_id_success() {
        let id = 12765;
        assert_eq!(encode(id), "3Jt");
    }

    #[test]
    fn test_decode_string_success() {
        let code = "3Jt";
        let decoded_string = decode(code);

        assert_eq!(decoded_string, Some(12765));
    }

    #[test]
    fn test_decode_invalid_string_failure() {
        let code = "*abc";
        let res = decode(code);

        assert!(res.is_none());
    }

    #[test]
    fn test_round_trip() {
        let id = 12765;

        let encoded_string = encode(id);
        let decoded_string = decode(&encoded_string);

        assert_eq!(decoded_string, Some(id));
    }
}
