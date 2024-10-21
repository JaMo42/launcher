use crate::static_units::*;
use libc::{localeconv, setlocale, LC_MONETARY};
use reqwest::blocking::get;
use slotmap::{new_key_type, SlotMap};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::CStr,
    mem::discriminant,
};

new_key_type! {
    pub struct CurrencyKey;
}

impl CurrencyKey {
    pub fn name(self) -> String {
        CURRENCIES.with_borrow(|c| c[self].full_name.clone())
    }

    //pub fn code(self) -> String {
    //    CURRENCIES.with_borrow(|c| c[self].currency_code.clone())
    //}

    pub fn rate(self) -> f64 {
        CURRENCIES.with_borrow(|c| c[self].rate)
    }
}

impl std::fmt::Display for CurrencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// Most country-specific keyboard layouts let you easily type the local
// currency symbol, should support that as well.
#[derive(Debug)]
pub struct Currency {
    /// The full name of the currency, may be empty.
    pub full_name: String,
    /// The currency code in lowercase.
    //pub currency_code: String,
    /// Conversion rate, based on the configured default currency.
    pub rate: f64,
}

thread_local! {
    pub static CURRENCIES: RefCell<SlotMap<CurrencyKey, Currency>> = Default::default();
    pub static CURRENCY_NAMES: RefCell<HashMap<String, CurrencyKey>> = Default::default();
    // Should these be called symbols? I've never dealt with this.
    pub static CURRENCY_CODES: RefCell<HashMap<String, CurrencyKey>> = Default::default();
    /// All names and codes.
    pub static CURRENCY_IDENTIFIERS: RefCell<HashSet<String>> = Default::default();
    static DEFAULT: RefCell<CurrencyKey> = Default::default();
}

/// Get the default default currenct from the locale.
pub fn user_currency() -> String {
    println!(
        "Getting default unit from locale: {}",
        std::env::var("LC_MONETARY").as_deref().unwrap_or("C")
    );
    unsafe {
        setlocale(LC_MONETARY, "\0".as_ptr() as *const _);
        let info = localeconv();
        let mut s = CStr::from_ptr((*info).int_curr_symbol)
            .to_string_lossy()
            .to_string();
        if s.is_empty() {
            return "EUR".to_string();
        }
        // no clue why but it's giving me a space at the end
        // also no clue why strings can't be trimmed in-place
        while s.ends_with(' ') {
            s.pop();
        }
        println!(" -> {}", s);
        s
    }
}

/// Get the currency key for a currency name or code.
pub fn currency(name_or_code: &str) -> Option<CurrencyKey> {
    CURRENCY_NAMES
        .with_borrow(|c| c.get(name_or_code).copied())
        .or_else(|| CURRENCY_CODES.with_borrow(|c| c.get(name_or_code).copied()))
}

/// Convert `amount` from `from` to `to`.
pub fn convert_currency(amount: f64, from: CurrencyKey, to: CurrencyKey) -> f64 {
    let from_rate = from.rate();
    let to_rate = to.rate();
    amount * to_rate / from_rate
}

mod currency_cache {
    use chrono::{DateTime, Datelike, NaiveDate, Utc};
    use std::{
        fs::{create_dir_all, read_to_string, write},
        time::SystemTime,
    };

    //
    // The conversion rate response from the api gives a date with day
    // granularity, so I guess that's a good heuristic for cache invalidation.
    // We completely base this off system time so we can avoid any api calls.
    // We could use the APIs date when saving the cache but it shouldn't matter.
    //

    fn path(file: &str) -> String {
        format!(
            "{}/.cache/launcher/{}",
            std::env::var("HOME").unwrap(),
            file
        )
    }

    pub fn is_up_to_date() -> bool {
        let mut dir = path("");
        dir.pop();
        if let Err(e) = create_dir_all(dir) {
            eprintln!("Failed to create cache directory: {}", e);
        }
        fn falliable() -> Option<bool> {
            let current_time = SystemTime::now();
            let current_time: DateTime<Utc> = current_time.into();
            let current_time = current_time.naive_utc().date();
            let cache_time = read_to_string(path("timestamp")).ok()?;
            let cache_time: NaiveDate = cache_time.parse().ok()?;
            Some(current_time.day() - cache_time.day() == 0)
        }
        falliable().unwrap_or(false)
    }

    pub fn units() -> Option<String> {
        read_to_string(path("units")).ok()
    }

    pub fn rates() -> Option<String> {
        read_to_string(path("rates")).ok()
    }

    pub fn put(units: &str, rates: &str) {
        let current_time = SystemTime::now();
        let current_time: DateTime<Utc> = current_time.into();
        let current_time = current_time.naive_utc().date();
        write(path("timestamp"), current_time.to_string()).unwrap();
        write(path("units"), units).unwrap();
        write(path("rates"), rates).unwrap();
        println!("Saved currency cache");
    }

    pub fn invalidate() {
        let bad_time = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        write(path("timestamp"), bad_time.to_string()).unwrap();
        println!("Invalidated currency cache");
    }
}

fn get_currencies(reference: &str) -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::*;
    // We can't combine `if let` with another condition so we have to use
    // `and_then` in order to have a single `else` branch.
    let can_use_cached = if currency_cache::is_up_to_date() {
        Some(())
    } else {
        None
    };
    macro_rules! get {
        ($what:ident, $url:expr,) => {
            if let Some($what) = can_use_cached.and_then(|_| currency_cache::$what()) {
                println!("Using cached currency {}", stringify!($what));
                let res = from_str(&$what);
                if res.is_err() {
                    eprintln!("Corruped currency {} cache", stringify!($what));
                    currency_cache::invalidate();
                    return get_currencies(reference);
                }
                unsafe { res.unwrap_unchecked() }
            } else {
                let url = $url;
                println!("Fetching currency {} from {}", stringify!($what), url);
                let resp = get(url)?.text()?;
                from_str(&resp)?
            }
        };
    }
    let units: Map<String, Value> = get!(
        units,
        "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies.min.json",
    );
    let mut rates: Map<String, Value> = get!(
        rates,
        format!(
            "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/{}.min.json",
            reference,
        ),
    );
    if can_use_cached.is_none() {
        currency_cache::put(&to_string(&units).unwrap(), &to_string(&rates).unwrap());
    }
    let rates = match rates.remove(reference).unwrap() {
        Value::Object(rates) => rates,
        _ => unreachable!(),
    };
    for (code, name_val) in units {
        let name = unsafe { name_val.as_str().unwrap_unchecked() };
        let rate = unsafe {
            rates
                .get(&code)
                .unwrap_unchecked()
                .as_f64()
                .unwrap_unchecked()
        };
        let key = CURRENCIES.with_borrow_mut(|c| {
            c.insert(Currency {
                full_name: name.to_string(),
                //currency_code: code.to_string(),
                rate,
            })
        });
        CURRENCY_NAMES.with_borrow_mut(|c| {
            c.insert(name.to_string(), key);
            c.insert(name.to_ascii_lowercase(), key);
        });
        CURRENCY_CODES.with_borrow_mut(|c| {
            c.insert(code.to_string(), key);
            c.insert(code.to_ascii_lowercase(), key);
        });
    }
    Ok(())
}

fn add_currencties(default: &str, mapping: &mut HashMap<Unit, Unit>) {
    let default_key = CURRENCY_CODES.with_borrow(|c| c.get(default).copied().unwrap());
    DEFAULT.with_borrow_mut(|d| *d = default_key);
    CURRENCIES.with_borrow(|c| {
        for key in c.keys() {
            if key == default_key {
                let to = CURRENCY_CODES.with_borrow(|c| {
                    c.get(if default == "eur" { "usd" } else { "eur" })
                        .copied()
                        .unwrap()
                });
                mapping.insert(Unit::Currency(key), Unit::Currency(to));
            } else {
                mapping.insert(Unit::Currency(key), Unit::Currency(default_key));
            }
        }
    });
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Unit {
    Distance(Distance),
    Mass(Mass),
    Area(Area),
    Volume(Volume),
    Temperature(Temperature),
    Speed(Speed),
    Currency(CurrencyKey),
}

impl From<Distance> for Unit {
    fn from(d: Distance) -> Self {
        Unit::Distance(d)
    }
}

impl From<Mass> for Unit {
    fn from(m: Mass) -> Self {
        Unit::Mass(m)
    }
}

impl From<Area> for Unit {
    fn from(a: Area) -> Self {
        Unit::Area(a)
    }
}

impl From<Volume> for Unit {
    fn from(v: Volume) -> Self {
        Unit::Volume(v)
    }
}

impl From<Temperature> for Unit {
    fn from(t: Temperature) -> Self {
        Unit::Temperature(t)
    }
}

impl From<Speed> for Unit {
    fn from(s: Speed) -> Self {
        Unit::Speed(s)
    }
}

impl From<CurrencyKey> for Unit {
    fn from(c: CurrencyKey) -> Self {
        Unit::Currency(c)
    }
}

// XXX: due to the modular way of speeds we only have defaults for km/h, m/s,
// and mi/h; we would need a wrapper around the hashmap to return km/h as the
// default for any unit but I think we can just ingore it as well.

#[derive(Debug, Default)]
pub struct UnitMappingResult {
    pub mapping: HashMap<Unit, Unit>,
    pub currency_error: Option<Box<dyn std::error::Error>>,
}

pub fn default_unit_mapping(default_currency: &str) -> UnitMappingResult {
    let mut result = UnitMappingResult::default();
    for (l, r) in crate::static_units::PAIRS.into_iter().copied() {
        result.mapping.insert(l, r);
        result.mapping.insert(r, l);
    }
    for (from, to) in crate::static_units::ONE_WAY.into_iter().copied() {
        result.mapping.insert(from, to);
    }
    match get_currencies(default_currency) {
        Ok(_) => add_currencties(default_currency, &mut result.mapping),
        Err(e) => result.currency_error = Some(e),
    }
    result
}

impl Unit {
    pub fn from_str(s: &str) -> Option<Self> {
        if let Some(unit) = static_unit_from_str(s) {
            Some(unit)
        } else if let Some(currency) = currency(s) {
            Some(Unit::Currency(currency))
        } else {
            None
        }
    }

    pub fn valid_conversion(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
    }
}

pub fn convert(value: f64, from: Unit, to: Unit) -> f64 {
    match (from, to) {
        (Unit::Currency(from), Unit::Currency(to)) => convert_currency(value, from, to),
        (Unit::Distance(from), Unit::Distance(to)) => from.convert(value, to),
        (Unit::Mass(from), Unit::Mass(to)) => from.convert(value, to),
        (Unit::Area(from), Unit::Area(to)) => from.convert(value, to),
        (Unit::Volume(from), Unit::Volume(to)) => from.convert(value, to),
        (Unit::Temperature(from), Unit::Temperature(to)) => from.convert(value, to),
        (Unit::Speed(from), Unit::Speed(to)) => from.convert(value, to),
        _ => {
            eprintln!("Invalid or conversion: {:?} -> {:?}", from, to);
            0.0
        }
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Unit::Distance(d) => write!(f, "{}", d),
            Unit::Mass(m) => write!(f, "{}", m),
            Unit::Area(a) => write!(f, "{}", a),
            Unit::Volume(v) => write!(f, "{}", v),
            Unit::Temperature(t) => write!(f, "{}", t),
            Unit::Speed(s) => write!(f, "{}", s),
            Unit::Currency(c) => write!(f, "{}", c),
        }
    }
}
