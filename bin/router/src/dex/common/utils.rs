use anchor_lang::AccountDeserialize;
use solana_sdk::account::Account;

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

pub fn deserialize_anchor_bytes<T: AccountDeserialize>(data: &[u8]) -> anyhow::Result<T> {
    let mut data: &[u8] = data;
    T::try_deserialize(&mut data).map_err(Into::into)
}

pub fn deserialize_anchor_account<T: AccountDeserialize>(
    account: &Account,
) -> anchor_lang::Result<T> {
    let mut data: &[u8] = &account.data;
    T::try_deserialize(&mut data).map_err(Into::into)
}
