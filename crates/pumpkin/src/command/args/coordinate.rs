use std::str::FromStr;

pub enum MaybeRelativeCoordinate<const IS_Y: bool> {
    Absolute(f64),
    Relative(f64),
}

impl<const IS_Y: bool> TryFrom<&str> for MaybeRelativeCoordinate<IS_Y> {
    type Error = <f64 as FromStr>::Err;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if let Some(s) = s.strip_prefix('~') {
            let offset = if s.is_empty() { 0.0 } else { s.parse()? };
            Ok(Self::Relative(offset))
        } else {
            let mut v = s.parse()?;

            // set position to block center if no decimal place is given
            if !IS_Y && !s.contains('.') {
                v += 0.5;
            }

            Ok(Self::Absolute(v))
        }
    }
}

impl<const IS_Y: bool> MaybeRelativeCoordinate<IS_Y> {
    pub fn into_absolute(self, origin: Option<f64>) -> Option<f64> {
        match self {
            Self::Absolute(v) => Some(v),
            Self::Relative(offset) => Some(origin? + offset),
        }
    }
}

#[derive(Debug)]
pub enum MaybeRelativeBlockCoordinate<const IS_Y: bool> {
    Absolute(i32),
    Relative(f64),
}

#[derive(Debug)]
pub enum ParseMaybeRelativeBlockCoordinateError {
    InvalidAbsolute,
    InvalidRelative,
}

impl<const IS_Y: bool> TryFrom<&str> for MaybeRelativeBlockCoordinate<IS_Y> {
    type Error = ParseMaybeRelativeBlockCoordinateError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if let Some(s) = s.strip_prefix('~') {
            let offset = if s.is_empty() {
                0.0
            } else {
                s.parse().map_err(|_| Self::Error::InvalidRelative)?
            };
            Ok(Self::Relative(offset))
        } else {
            Ok(Self::Absolute(
                s.parse().map_err(|_| Self::Error::InvalidAbsolute)?,
            ))
        }
    }
}

impl<const IS_Y: bool> MaybeRelativeBlockCoordinate<IS_Y> {
    pub fn into_absolute(self, origin: Option<f64>) -> Option<i32> {
        Some(match self {
            Self::Absolute(v) => v,
            Self::Relative(offset) => (origin? + offset).floor() as i32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_coordinate_centering_and_relative_offsets_parity() {
        // X/Z integer coordinates get centered (+0.5)
        let x_coord = MaybeRelativeCoordinate::<false>::try_from("100").unwrap();
        assert_eq!(x_coord.into_absolute(None), Some(100.5));

        // X/Z decimal coordinates are exact
        let x_decimal = MaybeRelativeCoordinate::<false>::try_from("100.25").unwrap();
        assert_eq!(x_decimal.into_absolute(None), Some(100.25));

        // Y coordinates do not get centered
        let y_coord = MaybeRelativeCoordinate::<true>::try_from("64").unwrap();
        assert_eq!(y_coord.into_absolute(None), Some(64.0));

        // Relative coordinates
        let rel_x = MaybeRelativeCoordinate::<false>::try_from("~5.5").unwrap();
        assert_eq!(rel_x.into_absolute(Some(10.0)), Some(15.5));

        let rel_empty = MaybeRelativeCoordinate::<false>::try_from("~").unwrap();
        assert_eq!(rel_empty.into_absolute(Some(10.0)), Some(10.0));

        // Block coordinates floor calculation
        let block_rel = MaybeRelativeBlockCoordinate::<false>::try_from("~0.6").unwrap();
        assert_eq!(block_rel.into_absolute(Some(10.2)), Some(10));
    }
}
