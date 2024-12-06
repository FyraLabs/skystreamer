/// Tries to normalize a language code to its main language code
#[tracing::instrument]
pub fn handle_language(lang: &str) -> Option<String> {
    // for some reason, langtag::Language::new("jp") is still a valid language
    // shouldn't it be converted to "ja"?
    // wtf?
    let special_cases = [("jp", "ja"), ("angika", "anp")];
    let lang = special_cases
        .iter()
        .find_map(|(from, to)| {
            if lang.to_lowercase() == *from {
                Some(to)
            } else {
                None
            }
        })
        .map_or(lang, |v| v);

    // tracing::debug!(?lang, "lang_tag");
    let lang = langtag::LangTag::new(lang).ok()?.language()?;

    // let lang_tag = langtag::LangTag::new(lang).ok()?;
    // println!("{:?}", lang_tag.as_normal());
    let primary = lang.primary();
    let subtags = lang.extension_subtags().collect::<Vec<_>>();
    tracing::trace!(?subtags, ?lang, ?primary, "lang_tag");
    Some(primary.to_string())
}

#[cfg(test)]

mod tests {
    use super::*;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_basic_normalization() {
        assert_eq!(handle_language("en"), Some("en".to_string()));
        assert_eq!(handle_language("en"), Some("en".to_string()));
    }
    // Test alternate shortcodes, like jp for ja (Japanese)
    #[test]
    #[traced_test]
    fn test_alt_language_normalization() {
        assert_eq!(handle_language("jp"), Some("ja".to_string()));
    }

    #[test]
    #[traced_test]
    fn test_subscript_normalization() {
        assert_eq!(handle_language("en-US"), Some("en".to_string()));
        assert_eq!(handle_language("en-GB"), Some("en".to_string()));
    }

    #[test]
    #[traced_test]
    /// Serializing weird Indian codes (i.e Angika)
    ///
    /// ATProto does 0 checking on the language codes, so we need to normalize them for analytics
    fn test_obscure_in_normalization() {
        assert_eq!(handle_language("Angika"), Some("anp".to_string()));
    }

    #[test]
    #[traced_test]
    fn test_null_normalization() {
        assert_eq!(handle_language(""), None);
    }
}
