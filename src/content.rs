use meval::Context;
use regex::Regex;

use crate::{static_units::Distance, units::Unit};

#[derive(Debug, Clone)]
pub enum Content {
    /// Input string contained only digits, basic arithmetic operators, and parentheses.
    BasicExpression(f64),
    /// Input string started with a `=`; it is assumed that it's an expression lead
    /// by the equal sign, but it is not verified.
    LeadExpression(Result<f64, meval::Error>),
    /// Input string is a number with a unit.
    DefaultConversion(f64, Unit),
    /// Input string is a number, with an optional unit, followed by `[to/in] <unit>`
    Conversion(f64, Option<Unit>, Unit),
    // TODO:
    // The usefulness of this is dubious at the moment, since it requires a
    // full path ~~and there isn't even pasting~~.  Providing full suggestions
    // would probably blow the scope of a single-process program and would
    // require a background process to hold onto the file index.  Optionally
    // it could only index the users multimedia directories (Documents,
    // Downloads, Pictures, etc.), since I also thought building the desktop
    // entry cache would be too slow which it wasn't this may be fine as well.
    /// The input string is a valid path. (`access(2)` reports read access)
    Path,
    /// The input string is a valid URL.
    URL,
    /// The input string starts with a `$`
    Command,
}

#[derive(Debug, Clone)]
pub struct ContentOptions {
    /// Whether to allow dynamic conversions, that is, conversions that are not
    /// based on a fixed conversion rate, meaning they will need to be fetched
    /// from the internet.
    pub dynamic_conversions: bool,
    /// What URLs to allow.
    pub url_mode: UrlMode,
}

impl Default for ContentOptions {
    fn default() -> Self {
        Self {
            dynamic_conversions: true,
            url_mode: UrlMode::Loose,
        }
    }
}

impl ContentOptions {
    pub fn is_allowed_unit(&self, unit: &Unit) -> bool {
        if !self.dynamic_conversions && matches!(unit, Unit::Currency(_)) {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UrlMode {
    None,
    Http,
    Loose,
}

impl UrlMode {
    // https://stackoverflow.com/a/3809435
    const LOOSE_URL_REGEX: &str =
        r#"[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=]*)"#;

    const HTTP_URL_REGEX: &str = r#"https?:\/\/(www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_\+.~#?&//=]*)"#;

    fn regex(self) -> Option<&'static str> {
        match self {
            UrlMode::None => None,
            UrlMode::Http => Some(Self::HTTP_URL_REGEX),
            UrlMode::Loose => Some(Self::LOOSE_URL_REGEX),
        }
    }
}

fn consider_for_basic_expression(s: &str) -> bool {
    // Filter out strings with just a single number, these would of course
    // evaluate correctly but it's not useful.
    !s.trim().bytes().all(|b| b.is_ascii_digit())
}

//
// We do some basic tokenization of the input; we can't do that much here
// since we don't know what the content types want.
//

#[derive(Debug, Copy, Clone)]
enum Token<'a> {
    /// A run of digits, thousands separators, and a decimal point.
    Number(f64),
    /// A run of letters
    Text(&'a str),
    /// Anything else, only a single character.
    Symbol(char),
}

fn utf8_len(b: u8) -> usize {
    const LOOKUP: [u8; 16] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 4];
    LOOKUP[(b >> 4) as usize] as usize
}

const fn is_extended_text_char(b: u8) -> bool {
    // We don't currently use this a symbol and including it in text allows us
    // parse `km/h` while keeping units contained to a single token.  Better
    // hope this stays true.
    b == b'/'
}

/// `None` if not a letter, otherwise number of bytes to skip.
fn is_letter(b: u8) -> Option<usize> {
    if b.is_ascii_alphabetic() || is_extended_text_char(b) {
        Some(1)
    } else if (b & 0xC0) != 0 {
        Some(utf8_len(b))
    } else {
        None
    }
}

// We could use an iterator to lex without allocating but it really doesn't
// matter and random access is convenient.
fn lex(s: &str) -> Vec<Token> {
    fn tostr(b: &[u8]) -> &str {
        unsafe { std::str::from_utf8_unchecked(b) }
    }
    let mut tokens = Vec::new();
    let mut bytes = s.as_bytes();
    while !bytes.is_empty() {
        let b = bytes[0];
        if b.is_ascii_digit() {
            // str::parse<f64> does not support thousands separators (despite
            // rust having them?)
            let mut string = Vec::new();
            let mut saw_decimal_point = false;
            let mut overhead = 0;
            for i in 0..bytes.len() {
                match bytes[i] {
                    b'0'..=b'9' => string.push(bytes[i]),
                    b'.' | b',' if !saw_decimal_point => {
                        saw_decimal_point = true;
                        string.push(b'.');
                    }
                    b'_' => overhead += 1,
                    _ => {
                        break;
                    }
                }
            }
            let num = tostr(&string);
            tokens.push(Token::Number(unsafe { num.parse().unwrap_unchecked() }));
            bytes = &bytes[(string.len() + overhead)..];
        } else if b.is_ascii_alphabetic() || (b & 0xC0) != 0 || is_extended_text_char(b) {
            // assume any unicode to be a letter
            let mut len = 0;
            while let Some(skip) = is_letter(bytes[len]) {
                len += skip;
                if len >= bytes.len() {
                    break;
                }
            }
            tokens.push(Token::Text(tostr(&bytes[..len])));
            bytes = &bytes[len..];
        } else if b == b' ' {
            bytes = &bytes[1..];
        } else {
            let len = utf8_len(b);
            let char_str = tostr(&bytes[..len]);
            bytes = &bytes[len..];
            tokens.push(Token::Symbol(unsafe {
                char_str.chars().next().unwrap_unchecked()
            }));
        }
    }
    tokens
}

#[derive(Debug, Copy, Clone)]
pub enum ClassificationError {
    // Note: currently displayed the same as an error, when writing this
    // description I was thinking about an error being red and a hint being
    // blue, but there are currently no colors at all.
    /// User entered `1centmeter`; could be intented just display the fact
    /// it's not a unit as a hint.
    InvalidUnit,
    /// User entered `1inch to centmeter`; we can be 100% sure this is a
    /// mistake and display it as an error.
    InvalidToUnit,
    /// 2 valid units but they can't be converted.
    InvalidConversion,
    /// User entered `1cm to`; this will likely be removed
    MissingToUnit,
}

impl std::fmt::Display for ClassificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClassificationError::InvalidUnit => write!(f, "Invalid unit"),
            ClassificationError::InvalidToUnit => write!(f, "Invalid `to` unit"),
            ClassificationError::InvalidConversion => write!(f, "Invalid conversion"),
            ClassificationError::MissingToUnit => write!(f, "Missing or invalid `to` unit"),
        }
    }
}

pub struct ContentClassifier {
    options: ContentOptions,
    url_regex: Option<Regex>,
    eval_cx: Context<'static>,
}

impl ContentClassifier {
    pub fn new(options: ContentOptions) -> Self {
        let url_regex = options.url_mode.regex().map(|r| Regex::new(r).unwrap());
        let eval_cx = Context::new();
        Self {
            options,
            url_regex,
            eval_cx,
        }
    }

    fn is_url(&self, s: &str) -> bool {
        if let Some(regex) = &self.url_regex {
            regex.is_match(s)
        } else {
            false
        }
    }

    /// Classify the input string without checking units.
    fn classify_unchecked(&self, s: &str) -> Result<Option<Content>, ClassificationError> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(None);
        } else if s.starts_with('=') {
            let expr = s[1..].trim();
            return Ok(Some(Content::LeadExpression(meval::eval_str(expr))));
        } else if s.starts_with('$') {
            return Ok(Some(Content::Command));
        } else if std::fs::metadata(s).is_ok() {
            // XXX: check read access?
            return Ok(Some(Content::Path));
        } else if self.is_url(s) {
            return Ok(Some(Content::URL));
        } else if consider_for_basic_expression(s) {
            if let Ok(result) = meval::eval_str_with_context(s, &self.eval_cx) {
                return Ok(Some(Content::BasicExpression(result)));
            }
        }
        fn get_unit(tokens: &mut [Token], index: &mut usize) -> Option<Unit> {
            match tokens.get(*index) {
                Some(Token::Text(t)) => {
                    if let Some(unit) = Unit::from_str(t) {
                        *index += 1;
                        Some(unit)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        let mut tokens = lex(s);
        let mut index = 1;
        let mut no_number = false;
        let num = match tokens.get(0) {
            Some(Token::Number(n)) => *n,
            _ => {
                index = 0;
                no_number = true;
                1.0
            }
        };
        let potentially_have_unit_a = matches!(tokens.get(1), Some(&Token::Text(_)));
        let unit_a = get_unit(&mut tokens, &mut index);
        let mut potentially_have_unit_b = false;
        let mut have_conversion_word = false;
        let unit_b = match tokens.get(index) {
            Some(&Token::Text(t)) if t == "to" || t == "in" || t == "as" => {
                if t == "in" && tokens.len() == index + 1 {
                    Some(Unit::Distance(Distance::Inch))
                } else {
                    index += 1;
                    potentially_have_unit_b = true;
                    have_conversion_word = true;
                    get_unit(&mut tokens, &mut index)
                }
            }
            Some(&Token::Text(t)) => {
                // we don't care about the index after this
                Unit::from_str(t)
            }
            _ => None,
        };
        if unit_a.is_none() && potentially_have_unit_a && unit_b.is_none() {
            return Err(ClassificationError::InvalidUnit);
        }
        if unit_a.is_some() && unit_b.is_none() && potentially_have_unit_b && !have_conversion_word
        {
            return Err(ClassificationError::InvalidToUnit);
        }
        let expected_token_count = !no_number as usize
            + unit_a.is_some() as usize
            + unit_b.is_some() as usize
            + have_conversion_word as usize;
        if tokens.len() != expected_token_count {
            return Ok(None);
        }
        if let Some(unit_b) = unit_b {
            return Ok(Some(Content::Conversion(num, unit_a, unit_b)));
        }
        if let Some(unit_a) = unit_a {
            // TODO:
            // `1cm to` is not a valid conversion... But maybe it should be;
            // just show the default conversion while the user is still typing,
            // especially since it will already have shown it at the point
            // where `1cm` was entered.
            //
            // Related: When typing `inch` it's valid `in`, invalid at `inc`,
            // and valid again at `inch`, maybe using the last valid unit for
            // 2 or 3 more letters of not getting a new one.
            if have_conversion_word {
                return Err(ClassificationError::MissingToUnit);
            }
            return Ok(Some(Content::DefaultConversion(num, unit_a)));
        }
        // Handle the quotation mark notation for feet and inches
        if matches!(tokens.get(1), Some(Token::Symbol('\''))) {
            if let Some(Token::Number(maybe_inch)) = tokens.get(2) {
                if matches!(tokens.get(3), Some(Token::Symbol('"'))) {
                    let total = num * 12.0 + maybe_inch;
                    return Ok(Some(Content::DefaultConversion(
                        total,
                        Unit::Distance(Distance::Inch),
                    )));
                }
            }
            return Ok(Some(Content::DefaultConversion(
                num,
                Unit::Distance(Distance::Feet),
            )));
        }
        Ok(None)
    }

    /// Classify the input string.
    ///
    /// Depending on content type this may also already do some of the work,
    /// for example instead of manually checking if an expression is valid it
    /// just gets evaluated immediately and that result used for the check,
    /// with the result already contained in the returned value.
    pub fn classify<'a>(&self, s: &'a str) -> Result<Option<Content>, ClassificationError> {
        let result = self.classify_unchecked(s);
        match result {
            Ok(Some(Content::DefaultConversion(_, unit))) => {
                if self.options.is_allowed_unit(&unit) {
                    result
                } else {
                    Ok(None)
                }
            }
            Ok(Some(Content::Conversion(_, a, b))) => {
                if let Some(a) = a {
                    if !self.options.is_allowed_unit(&a) {
                        return Ok(None);
                    }
                    if !a.valid_conversion(&b) {
                        return Err(ClassificationError::InvalidConversion);
                    }
                }
                if !self.options.is_allowed_unit(&b) {
                    return Ok(None);
                }
                result
            }
            _ => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::static_units::*;

    const CM: Unit = Unit::Distance(Distance::Meter(SiPrefix::Centi));
    const INCH: Unit = Unit::Distance(Distance::Inch);

    #[test]
    fn basic_expression() {
        let c = ContentClassifier::new(ContentOptions::default());
        assert!(matches!(c.classify("123"), Ok(None)));
        assert!(matches!(
            c.classify("123 + 456"),
            Ok(Some(Content::BasicExpression(579.0)))
        ));
        assert!(matches!(
            c.classify("123 + 456 * 789"),
            Ok(Some(Content::BasicExpression(359907.0)))
        ));
        assert!(matches!(
            c.classify("123 + 456 * 789 / (1 + 2)"),
            Ok(Some(Content::BasicExpression(120051.0)))
        ));
    }

    #[test]
    fn lead_expression() {
        let c = ContentClassifier::new(ContentOptions::default());
        assert!(matches!(
            c.classify("= 123"),
            Ok(Some(Content::LeadExpression(Ok(123.0))))
        ));
        assert!(matches!(
            c.classify("=123 + 456"),
            Ok(Some(Content::LeadExpression(Ok(579.0))))
        ));
        assert!(matches!(
            c.classify("= 123 + 456 * 789"),
            Ok(Some(Content::LeadExpression(Ok(359907.0))))
        ));
        assert!(matches!(
            c.classify("=123 + 456 * 789 / (1 + 2)"),
            Ok(Some(Content::LeadExpression(Ok(120051.0))))
        ));
        // We don't care, only the `=` matters
        assert!(matches!(
            c.classify("= definetly not an expression"),
            Ok(Some(Content::LeadExpression(Err(_))))
        ));
    }

    #[test]
    fn postfixed_number() {
        let c = ContentClassifier::new(ContentOptions::default());
        assert!(matches!(
            c.classify("123cm"),
            Ok(Some(Content::DefaultConversion(123.0, CM)))
        ));
        assert!(matches!(
            c.classify("123 cm"),
            Ok(Some(Content::DefaultConversion(123.0, CM)))
        ));
        assert!(matches!(
            c.classify("123xyz"),
            Err(ClassificationError::InvalidUnit),
        ));
    }

    #[test]
    fn conversion() {
        let c = ContentClassifier::new(ContentOptions::default());
        assert!(matches!(
            c.classify("123 to cm"),
            Ok(Some(Content::Conversion(123.0, None, CM)))
        ));
        assert!(matches!(
            c.classify("123 cm to inch"),
            Ok(Some(Content::Conversion(123.0, Some(CM), INCH)))
        ));
        assert!(matches!(
            c.classify("123cm in inch"),
            Ok(Some(Content::Conversion(123.0, Some(CM), INCH)))
        ));
        assert!(matches!(
            c.classify("123cm in"),
            Ok(Some(Content::Conversion(123.0, Some(CM), INCH)))
        ));
        assert!(matches!(
            c.classify("123cm in in"),
            Ok(Some(Content::Conversion(123.0, Some(CM), INCH)))
        ));
        assert!(matches!(
            c.classify("123cm to"),
            Err(ClassificationError::MissingToUnit)
        ));
    }

    #[test]
    fn path() {
        let c = ContentClassifier::new(ContentOptions::default());
        const VALID_PATH: &str = env!("CARGO_MANIFEST_DIR");
        assert!(matches!(c.classify(VALID_PATH), Ok(Some(Content::Path))));
        assert!(matches!(c.classify("/not/a/path"), Ok(None)));
    }

    #[test]
    fn url() {
        let never = ContentClassifier::new(ContentOptions {
            url_mode: UrlMode::None,
            ..ContentOptions::default()
        });
        let http = ContentClassifier::new(ContentOptions {
            url_mode: UrlMode::Http,
            ..ContentOptions::default()
        });
        let loose = ContentClassifier::new(ContentOptions {
            url_mode: UrlMode::Loose,
            ..ContentOptions::default()
        });
        assert!(matches!(never.classify("example"), Ok(None)));
        assert!(matches!(never.classify("https://example.com"), Ok(None)));
        assert!(matches!(never.classify("example.com"), Ok(None)));
        assert!(matches!(http.classify("example"), Ok(None)));
        assert!(matches!(
            http.classify("https://example.com"),
            Ok(Some(Content::URL))
        ));
        assert!(matches!(http.classify("example.com"), Ok(None)));
        assert!(matches!(loose.classify("example"), Ok(None)));
        assert!(matches!(
            loose.classify("https://example.com"),
            Ok(Some(Content::URL))
        ));
        assert!(matches!(
            loose.classify("example.com"),
            Ok(Some(Content::URL))
        ));
    }

    #[test]
    fn command() {
        let c = ContentClassifier::new(ContentOptions::default());
        assert!(matches!(c.classify("shutdown now"), Ok(None)));
        assert!(matches!(
            c.classify("$ rm -rf /"),
            Ok(Some(Content::Command))
        ));
        assert!(matches!(
            c.classify("$:(){ :|:& };:"),
            Ok(Some(Content::Command))
        ));
    }
}
