//! Unit conversion: categories of units, each converted through one base
//! unit. No GTK, so it can be unit-tested.
//!
//! Most units are a factor of the base unit. Temperatures also need an
//! offset, and fuel economy in distance per volume is the reciprocal of
//! volume per distance.

/// How a unit relates to its category's base unit.
#[derive(Debug, Clone, Copy)]
enum Rule {
    /// base = value × factor
    Factor(f64),
    /// base = value × scale + offset
    Affine(f64, f64),
    /// base = k ÷ value
    Reciprocal(f64),
}

#[derive(Debug)]
pub struct Unit {
    pub key: &'static str,
    pub name: &'static str,
    pub symbol: &'static str,
    rule: Rule,
}

#[derive(Debug)]
pub struct Category {
    pub key: &'static str,
    pub name: &'static str,
    pub units: &'static [Unit],
    /// The units shown when the category is first opened.
    pub default_pair: (&'static str, &'static str),
}

impl Category {
    pub fn unit(&self, key: &str) -> Option<&'static Unit> {
        self.units.iter().find(|u| u.key == key)
    }
}

const fn f(key: &'static str, name: &'static str, symbol: &'static str, factor: f64) -> Unit {
    Unit {
        key,
        name,
        symbol,
        rule: Rule::Factor(factor),
    }
}

const INCH: f64 = 0.0254;
const FOOT: f64 = 0.3048;
const YARD: f64 = 0.9144;
const MILE: f64 = 1609.344;
const US_GALLON: f64 = 3.785411784; // litres
const UK_GALLON: f64 = 4.54609; // litres
const POUND: f64 = 0.45359237; // kilograms

static LENGTH: [Unit; 11] = [
    f("mm", "Millimetre", "mm", 1e-3),
    f("cm", "Centimetre", "cm", 1e-2),
    f("m", "Metre", "m", 1.0),
    f("km", "Kilometre", "km", 1e3),
    f("um", "Micrometre", "µm", 1e-6),
    f("nm", "Nanometre", "nm", 1e-9),
    f("in", "Inch", "in", INCH),
    f("ft", "Foot", "ft", FOOT),
    f("yd", "Yard", "yd", YARD),
    f("mi", "Mile", "mi", MILE),
    f("nmi", "Nautical mile", "nmi", 1852.0),
];

static AREA: [Unit; 10] = [
    f("mm2", "Square millimetre", "mm²", 1e-6),
    f("cm2", "Square centimetre", "cm²", 1e-4),
    f("m2", "Square metre", "m²", 1.0),
    f("ha", "Hectare", "ha", 1e4),
    f("km2", "Square kilometre", "km²", 1e6),
    f("in2", "Square inch", "in²", INCH * INCH),
    f("ft2", "Square foot", "ft²", FOOT * FOOT),
    f("yd2", "Square yard", "yd²", YARD * YARD),
    f("acre", "Acre", "ac", 4046.8564224),
    f("mi2", "Square mile", "mi²", MILE * MILE),
];

static VOLUME: [Unit; 17] = [
    f("ml", "Millilitre", "ml", 1e-3),
    f("cl", "Centilitre", "cl", 1e-2),
    f("dl", "Decilitre", "dl", 1e-1),
    f("l", "Litre", "l", 1.0),
    f("m3", "Cubic metre", "m³", 1e3),
    f("tsp", "Teaspoon (US)", "tsp", US_GALLON / 768.0),
    f("tbsp", "Tablespoon (US)", "tbsp", US_GALLON / 256.0),
    f("floz", "Fluid ounce (US)", "fl oz", US_GALLON / 128.0),
    f("cup", "Cup (US)", "cup", US_GALLON / 16.0),
    f("pt", "Pint (US)", "pt", US_GALLON / 8.0),
    f("qt", "Quart (US)", "qt", US_GALLON / 4.0),
    f("gal", "Gallon (US)", "gal", US_GALLON),
    f("floz_uk", "Fluid ounce (UK)", "fl oz", UK_GALLON / 160.0),
    f("pt_uk", "Pint (UK)", "pt", UK_GALLON / 8.0),
    f("gal_uk", "Gallon (UK)", "gal", UK_GALLON),
    f("in3", "Cubic inch", "in³", INCH * INCH * INCH * 1e3),
    f("ft3", "Cubic foot", "ft³", FOOT * FOOT * FOOT * 1e3),
];

static MASS: [Unit; 7] = [
    f("mg", "Milligram", "mg", 1e-6),
    f("g", "Gram", "g", 1e-3),
    f("kg", "Kilogram", "kg", 1.0),
    f("t", "Tonne", "t", 1e3),
    f("oz", "Ounce", "oz", POUND / 16.0),
    f("lb", "Pound", "lb", POUND),
    f("st", "Stone", "st", POUND * 14.0),
];

static TEMPERATURE: [Unit; 3] = [
    Unit {
        key: "c",
        name: "Celsius",
        symbol: "°C",
        rule: Rule::Affine(1.0, 273.15),
    },
    Unit {
        key: "f",
        name: "Fahrenheit",
        symbol: "°F",
        rule: Rule::Affine(5.0 / 9.0, 273.15 - 32.0 * 5.0 / 9.0),
    },
    Unit {
        key: "k",
        name: "Kelvin",
        symbol: "K",
        rule: Rule::Factor(1.0),
    },
];

static SPEED: [Unit; 5] = [
    f("kmh", "Kilometres per hour", "km/h", 1.0 / 3.6),
    f("ms", "Metres per second", "m/s", 1.0),
    f("mph", "Miles per hour", "mph", MILE / 3600.0),
    f("kn", "Knot", "kn", 1852.0 / 3600.0),
    f("fts", "Feet per second", "ft/s", FOOT),
];

static TIME: [Unit; 7] = [
    f("ms", "Millisecond", "ms", 1e-3),
    f("s", "Second", "s", 1.0),
    f("min", "Minute", "min", 60.0),
    f("h", "Hour", "h", 3600.0),
    f("d", "Day", "d", 86400.0),
    f("wk", "Week", "wk", 604_800.0),
    f("yr", "Year (365.25 days)", "yr", 31_557_600.0),
];

static DATA: [Unit; 10] = [
    f("bit", "Bit", "bit", 0.125),
    f("b", "Byte", "B", 1.0),
    f("kb", "Kilobyte", "kB", 1e3),
    f("mb", "Megabyte", "MB", 1e6),
    f("gb", "Gigabyte", "GB", 1e9),
    f("tb", "Terabyte", "TB", 1e12),
    f("kib", "Kibibyte", "KiB", 1024.0),
    f("mib", "Mebibyte", "MiB", 1_048_576.0),
    f("gib", "Gibibyte", "GiB", 1_073_741_824.0),
    f("tib", "Tebibyte", "TiB", 1_099_511_627_776.0),
];

static PRESSURE: [Unit; 6] = [
    f("pa", "Pascal", "Pa", 1.0),
    f("kpa", "Kilopascal", "kPa", 1e3),
    f("bar", "Bar", "bar", 1e5),
    f("atm", "Atmosphere", "atm", 101_325.0),
    f(
        "psi",
        "Pounds per square inch",
        "psi",
        POUND * 9.80665 / (INCH * INCH),
    ),
    f("mmhg", "Millimetre of mercury", "mmHg", 101_325.0 / 760.0),
];

static ENERGY: [Unit; 7] = [
    f("j", "Joule", "J", 1.0),
    f("kj", "Kilojoule", "kJ", 1e3),
    f("cal", "Calorie", "cal", 4.184),
    f("kcal", "Kilocalorie", "kcal", 4184.0),
    f("wh", "Watt-hour", "Wh", 3600.0),
    f("kwh", "Kilowatt-hour", "kWh", 3.6e6),
    f("btu", "British thermal unit", "BTU", 1055.05585262),
];

static POWER: [Unit; 3] = [
    f("w", "Watt", "W", 1.0),
    f("kw", "Kilowatt", "kW", 1e3),
    f("hp", "Horsepower", "hp", 745.699_871_582_270_2),
];

static ANGLE: [Unit; 4] = [
    f("deg", "Degree", "°", 1.0),
    f("rad", "Radian", "rad", 180.0 / std::f64::consts::PI),
    f("grad", "Gradian", "gon", 0.9),
    f("turn", "Turn", "turn", 360.0),
];

static FUEL: [Unit; 4] = [
    f("l100km", "Litres per 100 km", "L/100 km", 1.0),
    Unit {
        key: "kml",
        name: "Kilometres per litre",
        symbol: "km/L",
        rule: Rule::Reciprocal(100.0),
    },
    Unit {
        key: "mpg",
        name: "Miles per gallon (US)",
        symbol: "mpg",
        rule: Rule::Reciprocal(100.0 * US_GALLON / (MILE / 1000.0)),
    },
    Unit {
        key: "mpg_uk",
        name: "Miles per gallon (UK)",
        symbol: "mpg",
        rule: Rule::Reciprocal(100.0 * UK_GALLON / (MILE / 1000.0)),
    },
];

static CATEGORIES: [Category; 13] = [
    Category {
        key: "length",
        name: "Length",
        units: &LENGTH,
        default_pair: ("cm", "in"),
    },
    Category {
        key: "mass",
        name: "Weight",
        units: &MASS,
        default_pair: ("kg", "lb"),
    },
    Category {
        key: "temperature",
        name: "Temperature",
        units: &TEMPERATURE,
        default_pair: ("c", "f"),
    },
    Category {
        key: "volume",
        name: "Volume",
        units: &VOLUME,
        default_pair: ("l", "gal"),
    },
    Category {
        key: "area",
        name: "Area",
        units: &AREA,
        default_pair: ("m2", "ft2"),
    },
    Category {
        key: "speed",
        name: "Speed",
        units: &SPEED,
        default_pair: ("kmh", "mph"),
    },
    Category {
        key: "time",
        name: "Time",
        units: &TIME,
        default_pair: ("h", "min"),
    },
    Category {
        key: "data",
        name: "Data",
        units: &DATA,
        default_pair: ("gb", "gib"),
    },
    Category {
        key: "pressure",
        name: "Pressure",
        units: &PRESSURE,
        default_pair: ("bar", "psi"),
    },
    Category {
        key: "energy",
        name: "Energy",
        units: &ENERGY,
        default_pair: ("kcal", "kj"),
    },
    Category {
        key: "power",
        name: "Power",
        units: &POWER,
        default_pair: ("kw", "hp"),
    },
    Category {
        key: "angle",
        name: "Angle",
        units: &ANGLE,
        default_pair: ("deg", "rad"),
    },
    Category {
        key: "fuel",
        name: "Fuel economy",
        units: &FUEL,
        default_pair: ("l100km", "mpg"),
    },
];

pub fn categories() -> &'static [Category] {
    &CATEGORIES
}

pub fn category(key: &str) -> Option<&'static Category> {
    CATEGORIES.iter().find(|c| c.key == key)
}

fn to_base(value: f64, unit: &Unit) -> Option<f64> {
    match unit.rule {
        Rule::Factor(factor) => Some(value * factor),
        Rule::Affine(scale, offset) => Some(value * scale + offset),
        Rule::Reciprocal(k) => (value != 0.0).then(|| k / value),
    }
}

fn from_base(base: f64, unit: &Unit) -> Option<f64> {
    match unit.rule {
        Rule::Factor(factor) => Some(base / factor),
        Rule::Affine(scale, offset) => Some((base - offset) / scale),
        Rule::Reciprocal(k) => (base != 0.0).then(|| k / base),
    }
}

/// `value` in `from` expressed in `to`; None where it has no answer (zero
/// fuel economy).
pub fn convert(value: f64, from: &Unit, to: &Unit) -> Option<f64> {
    let result = from_base(to_base(value, from)?, to)?;
    // Round away the last bits of floating-point noise: 1 in → 2.54 cm
    // rather than 2.5400000000000005.
    let rounded: f64 = format!("{result:.14e}").parse().unwrap_or(result);
    rounded.is_finite().then_some(rounded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(category: &str, value: f64, from: &str, to: &str) -> f64 {
        let c = super::category(category).unwrap();
        convert(value, c.unit(from).unwrap(), c.unit(to).unwrap()).unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
    }

    #[test]
    fn exact_definitions() {
        assert_eq!(conv("length", 1.0, "in", "cm"), 2.54);
        assert_eq!(conv("length", 1.0, "m", "cm"), 100.0);
        assert_eq!(conv("length", 10.0, "mm", "cm"), 1.0);
        assert_eq!(conv("length", 1.0, "mi", "km"), 1.609344);
        assert_eq!(conv("length", 1.0, "ft", "in"), 12.0);
        assert_eq!(conv("mass", 1.0, "lb", "kg"), 0.45359237);
        assert_eq!(conv("mass", 1.0, "st", "lb"), 14.0);
        assert_eq!(conv("volume", 1.0, "gal", "l"), 3.785411784);
        assert_eq!(conv("volume", 1.0, "cup", "floz"), 8.0);
        assert_eq!(conv("data", 1.0, "gib", "mib"), 1024.0);
        assert_eq!(conv("data", 1.0, "gb", "mb"), 1000.0);
        assert_eq!(conv("time", 1.5, "h", "min"), 90.0);
        assert_eq!(conv("speed", 36.0, "kmh", "ms"), 10.0);
        assert_eq!(conv("area", 1.0, "ha", "m2"), 10000.0);
        assert_eq!(conv("energy", 1.0, "kwh", "kj"), 3600.0);
        assert!(close(
            conv("angle", 180.0, "deg", "rad"),
            std::f64::consts::PI
        ));
    }

    #[test]
    fn temperatures_use_offsets() {
        assert_eq!(conv("temperature", 100.0, "c", "f"), 212.0);
        assert_eq!(conv("temperature", 32.0, "f", "c"), 0.0);
        assert_eq!(conv("temperature", -40.0, "c", "f"), -40.0);
        assert_eq!(conv("temperature", 0.0, "c", "k"), 273.15);
        assert_eq!(conv("temperature", 0.0, "k", "f"), -459.67);
    }

    #[test]
    fn fuel_economy_is_reciprocal() {
        assert!(close(conv("fuel", 10.0, "l100km", "kml"), 10.0));
        assert!(close(conv("fuel", 5.0, "l100km", "mpg"), 47.042_916_666_7));
        assert!(close(conv("fuel", 30.0, "mpg", "l100km"), 7.840_486_111_1));
        let fuel = category("fuel").unwrap();
        assert_eq!(
            convert(0.0, fuel.unit("mpg").unwrap(), fuel.unit("l100km").unwrap()),
            None
        );
    }

    #[test]
    fn every_unit_round_trips() {
        for category in categories() {
            for a in category.units {
                for b in category.units {
                    let there = convert(12.5, a, b).unwrap();
                    let back = convert(there, b, a).unwrap();
                    assert!(
                        close(back, 12.5),
                        "{} {} -> {} -> {back}",
                        category.key,
                        a.key,
                        b.key
                    );
                }
            }
        }
    }

    #[test]
    fn keys_are_unique_and_defaults_exist() {
        for category in categories() {
            let (from, to) = category.default_pair;
            assert!(
                category.unit(from).is_some() && category.unit(to).is_some(),
                "{}",
                category.key
            );
            for (i, unit) in category.units.iter().enumerate() {
                assert!(
                    category.units[i + 1..].iter().all(|u| u.key != unit.key),
                    "{}",
                    unit.key
                );
            }
        }
    }
}
