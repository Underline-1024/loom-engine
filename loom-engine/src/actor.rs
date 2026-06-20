use serde::{Serialize, Deserialize};
use anyhow::Result;
use crate::lim::Lim;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Stat {
    Numeric(Lim<i64>),
    Tag,
}
impl Stat {
    pub fn value(&self) -> Option<i64> {
        match self {
            Self::Numeric(n) => Some(**n),
            Self::Tag => None
        }
    }
    pub fn add(&mut self, num: i64) -> Result<i64> {
        match self {
            Self::Numeric(n) => {
                n.set_value(n.value() + num)?;
                Ok(**n)
            },
            Self::Tag => anyhow::bail!("Cannot add to Tag type")
        }
    }
    pub fn sub(&mut self, num: i64) -> Result<i64> {
        match self {
            Self::Numeric(n) => {
                n.set_value(n.value() - num)?;
                Ok(**n)
            },
            Self::Tag => anyhow::bail!("Cannot subtract from Tag type.")
        }
    }
    pub fn set_numeric(&mut self, lim_num: Lim<i64>) -> Result<()> {
        match self {
            Self::Numeric(_) => {
                *self = Self::Numeric(lim_num);
                Ok(())
            }
            Self::Tag => anyhow::bail!("Cannot set numeric value to Tag"),
        }
    }
    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Numeric(_))
    }
    pub fn is_tag(&self) -> bool {
        matches!(self, Self::Tag)
    }
}