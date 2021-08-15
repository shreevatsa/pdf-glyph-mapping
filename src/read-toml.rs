use anyhow::Result;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

fn get_font_mapping() -> Result<HashMap<u16, String>> {
    let ret = HashMap::<u16, String>::new();

    let input = r#"
        [0044]
        replacement_text = "a"
        replacement_codes = [97]
        replacement_desc = ["0061 LATIN SMALL LETTER A"]
        
        [00D7]
        replacement_text = "स्"
        replacement_codes = [2360, 2381]
        replacement_desc = ["0938 DEVANAGARI LETTER SA", "094D DEVANAGARI SIGN VIRAMA"]

        [0045]
        replacement_text = "b"
        replacement_codes = [98]
        replacement_desc = ["0062 LATIN SMALL LETTER B"]
    "#;

    // This works.
    let s: toml::Value = toml::from_str(input)?;
    println!("First is {:#?}", s);

    // This works too (Value is a table with three string)
    let s: HashMap<String, toml::Value> = toml::from_str(input)?;
    println!("Second is {:#?}", s);

    // This works too.
    let s: HashMap<String, HashMap<String, toml::Value>> = toml::from_str(input)?;
    println!("Third is {:#?}", s);

    // This is the final goal I guess?
    #[derive(Deserialize, Debug, Serialize)]
    struct Replacements {
        replacement_text: String,
        replacement_codes: Vec<i64>,
        replacement_desc: Vec<String>,
    }
    let mut m: HashMap<String, Replacements> = HashMap::new();
    m.insert(
        "0044".to_string(),
        Replacements {
            replacement_text: "a".to_string(),
            replacement_codes: vec![97],
            replacement_desc: vec!["0061 LATIN SMALL LETTER A".to_string()],
        },
    );
    m.insert(
        "00D7".to_string(),
        Replacements {
            replacement_text: "स्".to_string(),
            replacement_codes: vec![2360, 2381],
            replacement_desc: vec![
                "0938 DEVANAGARI LETTER SA".to_string(),
                "094D DEVANAGARI SIGN VIRAMA".to_string(),
            ],
        },
    );
    m.insert(
        "0045".to_string(),
        Replacements {
            replacement_text: "b".to_string(),
            replacement_codes: vec![98],
            replacement_desc: vec!["0062 LATIN SMALL LETTER B".to_string()],
        },
    );
    println!("Serialized:\n{}\n—serialized", toml::to_string(&m)?);

    let input = r#"
[0044]
replacement_text = "a"
replacement_codes = [97]
replacement_desc = ["0061 LATIN SMALL LETTER A"]

[00D7]
replacement_text = "स्"
replacement_codes = [2360, 2381]
replacement_desc = ["0938 DEVANAGARI LETTER SA", "094D DEVANAGARI SIGN VIRAMA"]

[0045]
replacement_text = "b"
replacement_codes = [98]
replacement_desc = ["0062 LATIN SMALL LETTER B"]
    "#;

    #[derive(Debug, Deserialize)]
    struct TmpReplacements {
        replacement_text: toml::Value,
        replacement_codes: toml::Value,
        replacement_desc: toml::Value,
    }
    let s: HashMap<String, TmpReplacements> = toml::from_str(input)?;
    println!("Fourth is {:#?}", s);

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use crate::get_font_mapping;

    #[test]
    fn it_works() {
        get_font_mapping().unwrap();
        assert_eq!(2 + 2, 4);
    }
}
