use crate::units::Unit;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SiPrefix {
    Yotta,
    Zetta,
    Exa,
    Peta,
    Tera,
    Giga,
    Mega,
    Kilo,
    Hecto,
    Deka,
    Deci,
    Centi,

    None,

    Milli,
    Micro,
    Nano,
    Pico,
    Femto,
    Atto,
    Zepto,
    Yocto,
}

impl SiPrefix {
    fn num(self) -> f64 {
        use SiPrefix::*;
        match self {
            Yotta => 1e24,
            Zetta => 1e21,
            Exa => 1e18,
            Peta => 1e15,
            Tera => 1e12,
            Giga => 1e9,
            Mega => 1e6,
            Kilo => 1e3,
            Hecto => 1e2,
            Deka => 1e1,
            Deci => 1e-1,
            Centi => 1e-2,
            None => 1.0,
            Milli => 1e-3,
            Micro => 1e-6,
            Nano => 1e-9,
            Pico => 1e-12,
            Femto => 1e-15,
            Atto => 1e-18,
            Zepto => 1e-21,
            Yocto => 1e-24,
        }
    }
}

impl SiPrefix {
    fn from_start_of_str(s: &str) -> Option<(Self, usize)> {
        use SiPrefix::*;
        // these must be sorted by length to avoid aliasing
        for (prefix, res) in [
            ("zepto", (Zepto, 5)),
            ("yocto", (Yocto, 5)),
            ("femto", (Femto, 5)),
            ("centi", (Centi, 5)),
            ("milli", (Milli, 5)),
            ("micro", (Micro, 5)),
            ("hecto", (Hecto, 5)),
            ("yotta", (Yotta, 5)),
            ("zetta", (Zetta, 5)),
            ("peta", (Peta, 4)),
            ("tera", (Tera, 4)),
            ("giga", (Giga, 4)),
            ("mega", (Mega, 4)),
            ("kilo", (Kilo, 4)),
            ("deka", (Deka, 4)),
            ("deci", (Deci, 4)),
            ("nano", (Nano, 4)),
            ("pico", (Pico, 4)),
            ("atto", (Atto, 4)),
            ("exa", (Exa, 3)),
            ("da", (Deka, 2)),
            ("Y", (Yotta, 1)),
            ("Z", (Zetta, 1)),
            ("E", (Exa, 1)),
            ("P", (Peta, 1)),
            ("T", (Tera, 1)),
            ("G", (Giga, 1)),
            ("M", (Mega, 1)),
            ("k", (Kilo, 1)),
            ("h", (Hecto, 1)),
            ("d", (Deci, 1)),
            ("c", (Centi, 1)),
            ("m", (Milli, 1)),
            ("µ", (Micro, 1)), // I think some ISO keyboard layouts have this
            ("u", (Micro, 1)),
            ("n", (Nano, 1)),
            ("p", (Pico, 1)),
            ("f", (Femto, 1)),
            ("a", (Atto, 1)),
            ("z", (Zepto, 1)),
            ("y", (Yocto, 1)),
            ("", (None, 0)),
        ] {
            if s.starts_with(prefix) {
                return Some(res);
            }
        }
        Option::None
    }
}

impl std::fmt::Display for SiPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use SiPrefix::*;
        let s = match self {
            Yotta => "Y",
            Zetta => "Z",
            Exa => "E",
            Peta => "P",
            Tera => "T",
            Giga => "G",
            Mega => "M",
            Kilo => "k",
            Hecto => "h",
            Deka => "da",
            Deci => "d",
            Centi => "c",
            None => "",
            Milli => "m",
            Micro => "µ",
            Nano => "n",
            Pico => "p",
            Femto => "f",
            Atto => "a",
            Zepto => "z",
            Yocto => "y",
        };
        write!(f, "{}", s)
    }
}

// XXX: use some multi-precision library

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Distance {
    Meter(SiPrefix),
    Inch,
    Feet,
    Yard,
    Mile,
}

impl Distance {
    fn rate(self) -> f64 {
        match self {
            Distance::Meter(prefix) => prefix.num(),
            Distance::Inch => 0.0254,
            Distance::Feet => 0.3048,
            Distance::Yard => 0.9144,
            Distance::Mile => 1609.344,
        }
    }

    pub fn convert(self, amount: f64, to: Distance) -> f64 {
        amount * self.rate() / to.rate()
    }
}

impl std::fmt::Display for Distance {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Distance::*;
        // Not sure if it's better to avoid using a `String` or to have all
        // these invidual `write!` calls; I guess it boils down to code size
        // vs speed, but that still leaves me unsure.
        match self {
            Meter(prefix) => write!(f, "{}m", prefix),
            Inch => write!(f, "in"),
            Feet => write!(f, "ft"),
            Yard => write!(f, "yd"),
            Mile => write!(f, "mi"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Mass {
    Gram(SiPrefix),
    Ounce,
    Pound,
    Stone,
}

impl Mass {
    fn rate(self) -> f64 {
        match self {
            Mass::Gram(prefix) => prefix.num(),
            Mass::Ounce => 28.349523125,
            Mass::Pound => 453.59237,
            Mass::Stone => 6350.29318,
        }
    }

    pub fn convert(self, amount: f64, to: Mass) -> f64 {
        amount * self.rate() / to.rate()
    }
}

impl std::fmt::Display for Mass {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Mass::*;
        match self {
            Gram(SiPrefix::Mega) => write!(f, "ton"),
            Gram(prefix) => write!(f, "{}g", prefix),
            Ounce => write!(f, "oz"),
            Pound => write!(f, "lb"),
            Stone => write!(f, "st"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Area {
    SquareMeter(SiPrefix),
    SquareInch,
    SquareFeet,
    SquareYard,
    SquareMile,
    Hectare,
    Acre,
}

impl Area {
    fn rate(self) -> f64 {
        match self {
            Area::SquareMeter(prefix) => prefix.num(),
            Area::SquareInch => 0.00064516,
            Area::SquareFeet => 0.09290304,
            Area::SquareYard => 0.83612736,
            Area::SquareMile => 2589988.110336,
            Area::Hectare => 10000.0,
            Area::Acre => 4046.8564224,
        }
    }

    pub fn convert(self, amount: f64, to: Area) -> f64 {
        amount * self.rate() / to.rate()
    }
}

impl std::fmt::Display for Area {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Area::*;
        match self {
            SquareMeter(prefix) => write!(f, "{}m²", prefix),
            SquareInch => write!(f, "in²"),
            SquareFeet => write!(f, "ft²"),
            SquareYard => write!(f, "yd²"),
            SquareMile => write!(f, "mi²"),
            Hectare => write!(f, "ha"),
            Acre => write!(f, "ac"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Volume {
    Liter(SiPrefix),
    Gallon,
    Quart,
    Pint,
    Cup,
    FluidOunce,
    Tablespoon,
    Teaspoon,
}

impl Volume {
    // XXX: AI generated, now another AI has told me that the US and UK have
    //      different definitions for some of these; no clue what is used here.
    fn rate(self) -> f64 {
        match self {
            Volume::Liter(prefix) => prefix.num(),
            Volume::Gallon => 3.785411784,
            Volume::Quart => 0.946352946,
            Volume::Pint => 0.473176473,
            Volume::Cup => 0.2365882365,
            Volume::FluidOunce => 0.0295735295625,
            Volume::Tablespoon => 0.01478676478125,
            Volume::Teaspoon => 0.00492892159375,
        }
    }

    pub fn convert(self, amount: f64, to: Volume) -> f64 {
        amount * self.rate() / to.rate()
    }
}

impl std::fmt::Display for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Volume::*;
        match self {
            Liter(prefix) => write!(f, "{}L", prefix),
            Gallon => write!(f, "gal"),
            Quart => write!(f, "qt"),
            Pint => write!(f, "pt"),
            Cup => write!(f, "cup"),
            FluidOunce => write!(f, "floz"),
            Tablespoon => write!(f, "tbsp"),
            Teaspoon => write!(f, "tsp"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Temperature {
    Celsius,
    Fahrenheit,
    Kelvin,
}

impl Temperature {
    pub fn convert(self, amount: f64, to: Temperature) -> f64 {
        match self {
            Temperature::Celsius => match to {
                Temperature::Celsius => amount,
                Temperature::Fahrenheit => amount * 9.0 / 5.0 + 32.0,
                Temperature::Kelvin => amount + 273.15,
            },
            Temperature::Fahrenheit => match to {
                Temperature::Celsius => (amount - 32.0) * 5.0 / 9.0,
                Temperature::Fahrenheit => amount,
                Temperature::Kelvin => (amount + 459.67) * 5.0 / 9.0,
            },
            Temperature::Kelvin => match to {
                Temperature::Celsius => amount - 273.15,
                Temperature::Fahrenheit => amount * 9.0 / 5.0 - 459.67,
                Temperature::Kelvin => amount,
            },
        }
    }
}

impl std::fmt::Display for Temperature {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Temperature::*;
        match self {
            Celsius => write!(f, "°C"),
            Fahrenheit => write!(f, "°F"),
            Kelvin => write!(f, "K"),
        }
    }
}

/// Time used as denominator for speed.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SpeedTime {
    Second(SiPrefix),
    Minute,
    Hour,
}

impl SpeedTime {
    fn from_str(s: &str) -> Option<Self> {
        use SpeedTime::*;
        match s {
            "s" => Some(Second(SiPrefix::None)),
            "min" => Some(Minute),
            "h" => Some(Hour),
            _ => Option::None,
        }
    }

    fn rate(self) -> f64 {
        use SpeedTime::*;
        match self {
            Second(prefix) => prefix.num(),
            Minute => 60.0,
            Hour => 3600.0,
        }
    }
}

impl std::fmt::Display for SpeedTime {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use SpeedTime::*;
        match self {
            Second(prefix) => write!(f, "{}s", prefix),
            Minute => write!(f, "min"),
            Hour => write!(f, "h"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Speed {
    pub distance: Distance,
    pub time: SpeedTime,
}

impl Speed {
    pub fn rate(self) -> f64 {
        self.distance.rate() / self.time.rate()
    }

    pub fn convert(self, amount: f64, to: Speed) -> f64 {
        amount * self.rate() / to.rate()
    }

    pub const fn kph() -> Speed {
        Speed {
            distance: Distance::Meter(SiPrefix::Kilo),
            time: SpeedTime::Hour,
        }
    }

    pub const fn mph() -> Speed {
        Speed {
            distance: Distance::Mile,
            time: SpeedTime::Hour,
        }
    }

    pub const fn mps() -> Speed {
        Speed {
            distance: Distance::Meter(SiPrefix::None),
            time: SpeedTime::Second(SiPrefix::None),
        }
    }
}

impl std::fmt::Display for Speed {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}/{}", self.distance, self.time)
    }
}

/// Bi-directional mapping of static units and their default counterparts for
/// conversions.
pub static PAIRS: &[(Unit, Unit)] = {
    use self::{Area::*, Distance::*, Mass::*, Temperature::*, Volume::*};
    use SiPrefix::*;
    use Unit::*;
    &[
        // Distance
        (Distance(Inch), Distance(Meter(Centi))),
        (Distance(Feet), Distance(Meter(None))),
        (Distance(Yard), Distance(Meter(None))),
        (Distance(Mile), Distance(Meter(Kilo))),
        // Mass
        (Mass(Ounce), Mass(Gram(None))),
        (Mass(Pound), Mass(Gram(Kilo))),
        // Area
        (Area(SquareInch), Area(SquareMeter(Centi))),
        (Area(SquareFeet), Area(SquareMeter(None))),
        (Area(SquareMile), Area(SquareMeter(Kilo))),
        // Volume
        (Volume(Gallon), Volume(Liter(None))),
        (Volume(Tablespoon), Volume(Liter(Milli))),
        // Temperature
        (Temperature(Fahrenheit), Temperature(Celsius)),
        // Speed
        (Speed(self::Speed::kph()), Speed(self::Speed::mph())),
    ]
};

// (from, to)
pub static ONE_WAY: &[(Unit, Unit)] = {
    use self::{Area::*, Distance::*, Mass::*, Temperature::*, Volume::*};
    use SiPrefix::*;
    use Unit::*;
    &[
        // Distance
        (Distance(Meter(Milli)), Distance(Inch)),
        // Mass
        (Mass(Stone), Mass(Gram(Kilo))),
        (Mass(Gram(Mega)), Mass(Pound)),
        // Area
        (Area(SquareYard), Area(SquareMeter(None))),
        (Area(Hectare), Area(SquareMeter(Kilo))),
        (Area(Acre), Area(SquareMeter(Kilo))),
        // Volume
        (Volume(Quart), Volume(Liter(None))),
        (Volume(Pint), Volume(Liter(None))),
        (Volume(Cup), Volume(Liter(Milli))),
        (Volume(FluidOunce), Volume(Liter(Milli))),
        (Volume(Teaspoon), Volume(Liter(Milli))),
        // Temperature
        (Temperature(Kelvin), Temperature(Celsius)),
        // Speed
        (Speed(self::Speed::mps()), Speed(self::Speed::kph())),
    ]
};

pub fn static_unit_from_str(s: &str) -> Option<Unit> {
    use self::{Area::*, Distance::*, Mass::*, Temperature::*, Volume::*};
    use Unit::*;
    // XXX: this currently allows `kinch`, where the SI prefix would just be
    //      discarded.
    let candidates: &[(&str, SiPrefix)] =
        if let Some((prefix, len)) = SiPrefix::from_start_of_str(s) {
            &[(s, SiPrefix::None), (&s[len..], prefix)]
        } else {
            &[(s, SiPrefix::None)]
        };
    for (s, prefix) in candidates.into_iter().cloned() {
        match s {
            // Distance
            "m" => return Some(Distance(Meter(prefix))),
            "meter" => return Some(Distance(Meter(prefix))),
            "meters" => return Some(Distance(Meter(prefix))),
            "in" => return Some(Distance(Inch)),
            "inch" => return Some(Distance(Inch)),
            "inches" => return Some(Distance(Inch)),
            "ft" => return Some(Distance(Feet)),
            "foot" => return Some(Distance(Feet)),
            "feet" => return Some(Distance(Feet)),
            "yd" => return Some(Distance(Yard)),
            "yard" => return Some(Distance(Yard)),
            "yards" => return Some(Distance(Yard)),
            "mi" => return Some(Distance(Mile)),
            "mile" => return Some(Distance(Mile)),
            "miles" => return Some(Distance(Mile)),
            // Mass
            "g" => return Some(Mass(Gram(prefix))),
            "gram" => return Some(Mass(Gram(prefix))),
            "grams" => return Some(Mass(Gram(prefix))),
            "ton" => return Some(Mass(Gram(SiPrefix::Mega))),
            "tons" => return Some(Mass(Gram(SiPrefix::Mega))),
            "tonne" => return Some(Mass(Gram(SiPrefix::Mega))),
            "tonnes" => return Some(Mass(Gram(SiPrefix::Mega))),
            "oz" => return Some(Mass(Ounce)),
            "ounce" => return Some(Mass(Ounce)),
            "ounces" => return Some(Mass(Ounce)),
            "lb" => return Some(Mass(Pound)),
            "pound" => return Some(Mass(Pound)),
            "pounds" => return Some(Mass(Pound)),
            "st" => return Some(Mass(Stone)),
            "stone" => return Some(Mass(Stone)),
            "stones" => return Some(Mass(Stone)),
            // Area
            "m2" => return Some(Area(SquareMeter(prefix))),
            "meter2" => return Some(Area(SquareMeter(prefix))),
            "in2" => return Some(Area(SquareInch)),
            "inch2" => return Some(Area(SquareInch)),
            "ft2" => return Some(Area(SquareFeet)),
            "feet2" => return Some(Area(SquareFeet)),
            "yd2" => return Some(Area(SquareYard)),
            "yard2" => return Some(Area(SquareYard)),
            "mi2" => return Some(Area(SquareMile)),
            "mile2" => return Some(Area(SquareMile)),
            "miles2" => return Some(Area(SquareMile)),
            "ha" => return Some(Area(Hectare)),
            "hectare" => return Some(Area(Hectare)),
            "ac" => return Some(Area(Acre)),
            "acre" => return Some(Area(Acre)),
            // Volume
            "l" => return Some(Volume(Liter(prefix))),
            "liter" => return Some(Volume(Liter(prefix))),
            "liters" => return Some(Volume(Liter(prefix))),
            "gal" => return Some(Volume(Gallon)),
            "gallon" => return Some(Volume(Gallon)),
            "gallons" => return Some(Volume(Gallon)),
            "qt" => return Some(Volume(Quart)),
            "quart" => return Some(Volume(Quart)),
            "quarts" => return Some(Volume(Quart)),
            "pt" => return Some(Volume(Pint)),
            "pint" => return Some(Volume(Pint)),
            "pints" => return Some(Volume(Pint)),
            "cup" => return Some(Volume(Cup)),
            "cups" => return Some(Volume(Cup)),
            // XXX: should be `fl oz`, but don't allow spaces
            "floz" => return Some(Volume(FluidOunce)),
            "fluidounce" => return Some(Volume(FluidOunce)),
            "fluidounces" => return Some(Volume(FluidOunce)),
            "tbsp" => return Some(Volume(Tablespoon)),
            "tablespoon" => return Some(Volume(Tablespoon)),
            "tablespoons" => return Some(Volume(Tablespoon)),
            "tsp" => return Some(Volume(Teaspoon)),
            "teaspoon" => return Some(Volume(Teaspoon)),
            "teaspoons" => return Some(Volume(Teaspoon)),
            // Temperature
            "C" => return Some(Temperature(Celsius)),
            "F" => return Some(Temperature(Fahrenheit)),
            "K" => return Some(Temperature(Kelvin)),

            _ => {}
        }
    }
    match s {
        "kph" => return Some(self::Speed::kph().into()),
        "mph" => return Some(self::Speed::mph().into()),
        _ => {}
    }
    if let Some(middle) = s.find('/') {
        let diststr = &s[..middle];
        let timestr = &s[middle + 1..];
        let dist = match static_unit_from_str(diststr) {
            Some(Unit::Distance(dist)) => Some(dist),
            _ => None,
        };
        let time = SpeedTime::from_str(timestr);
        if let Some((distance, time)) = dist.zip(time) {
            return Some(Speed(self::Speed { distance, time }));
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::{static_unit_from_str, SiPrefix::*};
    #[allow(unused_imports)]
    use super::{Area::*, Distance::*, Mass::*, Temperature::*, Volume::*};
    use crate::units::Unit::*;

    #[test]
    fn distance() {
        assert_eq!(static_unit_from_str("cm"), Some(Distance(Meter(Centi))));
        assert_eq!(
            static_unit_from_str("centimeter"),
            Some(Distance(Meter(Centi)))
        );
    }
}
