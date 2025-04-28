use serde::{Deserialize, Deserializer};
use solana_program::pubkey::Pubkey;
use std::str::FromStr;

pub fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(Pubkey::from_str(s.as_str()).unwrap())
}

pub fn change_option_ignore_none_old<T: PartialEq>(old: &mut Option<T>, new: Option<T>) -> bool {
    match (&old, new) {
        (Some(old_value), Some(new_value)) => {
            if *old_value != new_value {
                old.replace(new_value);
                true
            } else {
                false
            }
        }
        (None, Some(new_value)) => {
            old.replace(new_value);
            true
        }
        _ => false,
    }
}

pub fn change_data_if_not_same<T: PartialEq>(old: &mut T, new: T) -> bool {
    if *old != new {
        *old = new;
        true
    } else {
        false
    }
}
