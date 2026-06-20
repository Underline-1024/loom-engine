use std::{fmt::Debug, ops::Deref};
use anyhow::{bail, Result};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Deserialize)]
struct LimHelper<T> {
    value: T,
    min: T,
    max: T,
    include_min: bool,
    include_max: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct Lim<T: PartialOrd + Clone + Debug> {
    value: T,
    min: T,
    max: T,
    include_min: bool,
    include_max: bool,
}
impl<T: Clone + Copy + PartialOrd + Debug> Copy for Lim<T> {}
impl<T: Clone + PartialOrd + Debug> Lim<T> {
    pub fn new(value: T, min: T, max: T, include_min: bool, include_max: bool) -> Result<Self> {
        let min_ok = if include_min { 
            value >= min 
        } else { 
            value > min 
        };
        let max_ok = if include_max { 
            value <= max 
        } else { 
            value < max 
        };
        if min_ok && max_ok {
            Ok(Self {
                value,
                min,
                max,
                include_min,
                include_max,
            })
        } else {
            bail!("Value out of bounds.")
        }
    }
    pub fn value(&self) -> &T {
        &self.value
    }
    pub fn min(&self) -> &T {
        &self.min
    }
    pub fn max(&self) -> &T {
        &self.max
    }
    pub fn include_min(&self) -> bool {
        self.include_min
    }
    pub fn include_max(&self) -> bool {
        self.include_max
    }
    pub fn contains(&self, value: &T) -> bool {
        let min_ok = if self.include_min { 
            value >= &self.min 
        } else { 
            value > &self.min 
        };
        let max_ok = if self.include_max { 
            value <= &self.max 
        } else { 
            value < &self.max 
        };
        min_ok && max_ok
    }
    pub fn set_value(&mut self, value: T) -> Result<()> {
        if self.contains(&value) {
            self.value = value;
        } else {
            bail!("Value out of bounds.")
        }
        Ok(())
    }
    pub fn set_min(&mut self, min: T, include_min: bool, clamp: bool) -> Result<()> {
        if min > self.max {
            bail!("New min {:?} > current max {:?}.", min, self.max);
        }

        let value_ok = if include_min {
            self.value >= min
        } else {
            self.value > min
        };

        if value_ok {
            self.min = min;
            self.include_min = include_min;
            Ok(())
        } else if include_min && clamp {
            self.value = min.clone();
            self.min = min;
            self.include_min = include_min;
            Ok(())
        } else {
            if clamp {
                if self.value == min && !include_min {
                    bail!("Ambiguous clamp: value equals new exclusive min.");
                } else {
                    bail!("Cannot clamp to exclusive min.");
                }
            } else if !include_min {
                bail!("Value not in exclusive range and clamping disabled.");
            } else {
                bail!("Value below min and clamping disabled.");
            }
        }
    }
    pub fn set_max(&mut self, max: T, include_max: bool, clamp: bool) -> Result<()> {
        if max < self.min {
            bail!("New max {:?} is below current min {:?}.", max, self.min);
        }

        let value_ok = if include_max {
            self.value <= max
        } else {
            self.value < max
        };

        if value_ok {
            self.max = max;
            self.include_max = include_max;
            Ok(())
        } else if include_max && clamp {
            self.value = max.clone();
            self.max = max;
            self.include_max = include_max;
            Ok(())
        } else {
            if clamp {
                if self.value == max && !include_max {
                    bail!("Ambiguous clamp: value equals new exclusive max.");
                } else {
                    bail!("Cannot clamp to exclusive max.");
                }
            } else if !include_max {
                bail!("Value not in exclusive range and clamping disabled.");
            } else {
                bail!("Value above max and clamping disabled.");
            }
        }
    }
    
    pub fn set_bounds(
        &mut self,
        min: T,
        max: T,
        include_min: bool,
        include_max: bool,
        clamp: bool
    ) -> Result<()> {
        if min > max {
            bail!("Invalid bounds: min {:?} > max {:?}.", min, max);
        }
    
        let min_ok = if include_min {
            self.value >= min
        } else {
            self.value > min
        };
        let max_ok = if include_max {
            self.value <= max
        } else {
            self.value < max
        };
        let value_ok = min_ok && max_ok;
    
        if value_ok {
            self.min = min;
            self.max = max;
            self.include_min = include_min;
            self.include_max = include_max;
            Ok(())
        } else if clamp {
            let mut new_value = self.value.clone();
    
            // 处理下界
            if !min_ok {
                if self.value == min && !include_min {
                    bail!("Ambiguous clamp: value equals new exclusive min.");
                }
                if !include_min {
                    bail!("Cannot clamp to exclusive min.");
                }
                new_value = min.clone();
            }
    
            // 处理上界
            if !max_ok {
                if new_value == max && !include_max {
                    bail!("Ambiguous clamp: value equals new exclusive max.");
                }
                if !include_max {
                    bail!("Cannot clamp to exclusive max.");
                }
                if new_value > max || (!include_max && new_value == max) {
                    if new_value == max && !include_max {
                        bail!("Ambiguous clamp: clamped to min but equals new exclusive max.");
                    }
                    new_value = max.clone();
                }
            }
    
            self.value = new_value;
            self.min = min;
            self.max = max;
            self.include_min = include_min;
            self.include_max = include_max;
            Ok(())
        } else {
            if !min_ok && !max_ok {
                bail!("Value out of bounds on both sides and clamping disabled.");
            } else if !min_ok {
                if !include_min && self.value == min {
                    bail!("Value equals new exclusive min and clamping disabled.");
                } else {
                    bail!("Value below new min and clamping disabled.");
                }
            } else {
                if !include_max && self.value == max {
                    bail!("Value equals new exclusive max and clamping disabled.");
                } else {
                    bail!("Value above new max and clamping disabled.");
                }
            }
        }
    }
}
impl<T: PartialOrd + Clone + Debug> Deref for Lim<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<'de, T> Deserialize<'de> for Lim<T>
where
    T: Deserialize<'de> + PartialOrd + Clone + Debug {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: Deserializer<'de> {
        let helper = LimHelper::deserialize(deserializer)?;
        Lim::new(
            helper.value,
            helper.min,
            helper.max,
            helper.include_min,
            helper.include_max,
        )
        .map_err(|e| serde::de::Error::custom(format!("Lim invariant violated: {}", e)))
    }
}