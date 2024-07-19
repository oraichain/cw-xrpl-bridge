use super::helper::{generate_invalid_xrpl_address, generate_xrpl_address};
use crate::{address::validate_xrpl_address_format, contract::INITIAL_PROHIBITED_XRPL_ADDRESSES};

#[test]
fn validate_xrpl_addresses() {
    let mut valid_addresses = vec![
        "rU6K7V3Po4snVhBBaU29sesqs2qTQJWDw1".to_string(),
        "rLUEXYuLiQptky37CqLcm9USQpPiz5rkpD".to_string(),
        "rBTwLga3i2gz3doX6Gva3MgEV8ZCD8jjah".to_string(),
        "rDxMt25DoKeNv7te7WmLvWwsmMyPVBctUW".to_string(),
        "rPbPkTSrAqANkoTFpwheTxRyT8EQ38U5ok".to_string(),
        "rQ3fNyLjbvcDaPNS4EAJY8aT9zR3uGk17c".to_string(),
        "rnATJKpFCsFGfEvMC3uVWHvCEJrh5QMuYE".to_string(),
        generate_xrpl_address(),
        generate_xrpl_address(),
        generate_xrpl_address(),
        generate_xrpl_address(),
    ];

    // Add the current prohibited address and check that they are valid generated xrpl addresses
    for prohibited_address in INITIAL_PROHIBITED_XRPL_ADDRESSES {
        valid_addresses.push(prohibited_address.to_string());
    }

    for address in valid_addresses.iter() {
        validate_xrpl_address_format(address).unwrap();
    }

    let mut invalid_addresses: Vec<String> = vec![
        "zDTXLQ7ZKZVKz33zJbHjgVShjsBnqMBhmN".to_string(), // Invalid prefix
        "rf1BiGeXwwQoi8Z2u".to_string(),                  // Too short
        "rU6K7V3Po4snVhBBaU29sesqs2qTQJWDw1hBBaU29".to_string(), // Too long
        "rU6K7V3Po4snVhBBa029sesqs2qTQJWDw1".to_string(), // Contains invalid character 0
        "rU6K7V3Po4snVhBBaU29sesql2qTQJWDw1".to_string(), // Contains invalid character l
        "rLUEXYuLiQptky37OqLcm9USQpPiz5rkpD".to_string(), // Contains invalid character O
        "rLUEXYuLiQpIky37CqLcm9USQpPiz5rkpD".to_string(), // Contains invalid character I
    ];

    for _ in 0..100 {
        invalid_addresses.push(generate_invalid_xrpl_address()); // Just random address without checksum calculation
    }

    for address in invalid_addresses.iter() {
        validate_xrpl_address_format(address).unwrap_err();
    }
}
