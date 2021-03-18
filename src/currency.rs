use std::str::FromStr;

// indicates that more variants might be added in the future
// prevents that to break users code
// this is probably not practical and the "real" application would
// get supported currencies dynamically from a database
#[non_exhaustive]
#[derive(Clone)]
pub enum Currency {
    Btc,
    Eth,
}

impl Currency {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Btc => "btc",
            Self::Eth => "eth",
        }
    }
}

impl FromStr for Currency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "btc" => Ok(Self::Btc),
            "eth" => Ok(Self::Eth),
            _ => Err(format!("unsupported currency: `{}`", s)),
        }
    }
}

#[derive(Clone)]
pub struct Conversion {
    from: Currency,
    to: Currency,
}

impl Conversion {
    pub fn new(to: Currency, from: Currency) -> Self {
        Self { from, to }
    }
}

impl ToString for Conversion {
    fn to_string(&self) -> String {
        format!("{}{}", self.to.to_str(), self.from.to_str())
    }
}
