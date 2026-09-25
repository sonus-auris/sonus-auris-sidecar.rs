#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Region {
    AppStorage,
    SharedExternal,
    Unknown,
}

const REGIONS: [Region; 3] = [Region::AppStorage, Region::SharedExternal, Region::Unknown];

fn delete_allowed(region: Region, delete_requested: bool, owned: bool) -> bool {
    if !delete_requested {
        return true;
    }

    return region == Region::AppStorage && owned;
}

fn validate_state(region: Region, delete_requested: bool, owned: bool) -> Result<(), &'static str> {
    let allowed = delete_allowed(region, delete_requested, owned);

    if delete_requested && allowed && region != Region::AppStorage {
        return Err("delete admitted outside app storage");
    }

    if delete_requested && allowed && !owned {
        return Err("delete admitted for unowned artifact");
    }

    return Ok(());
}

fn run_model() -> Result<usize, &'static str> {
    let mut checked = 0usize;

    for region in REGIONS {
        for delete_requested in [false, true] {
            for owned in [false, true] {
                validate_state(region, delete_requested, owned)?;
                checked += 1;
            }
        }
    }

    return Ok(checked);
}

fn main() {
    match run_model() {
        Ok(checked) => {
            println!("storage confinement model: {checked} states");
            return;
        }
        Err(message) => {
            eprintln!("storage confinement model failed: {message}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{delete_allowed, run_model, Region};

    #[test]
    fn checks_all_twelve_states() {
        let checked = run_model().expect("bounded model must satisfy its invariants");
        assert_eq!(checked, 12);
        return;
    }

    #[test]
    fn delete_requires_owned_app_storage() {
        assert!(delete_allowed(Region::AppStorage, true, true));
        assert!(!delete_allowed(Region::AppStorage, true, false));
        assert!(!delete_allowed(Region::SharedExternal, true, true));
        assert!(!delete_allowed(Region::Unknown, true, true));
        return;
    }

    #[test]
    fn non_delete_operations_remain_admissible() {
        for region in [Region::AppStorage, Region::SharedExternal, Region::Unknown] {
            assert!(delete_allowed(region, false, false));
            assert!(delete_allowed(region, false, true));
        }

        return;
    }
}
