use common_utils::errors::{ApiModelsError, CustomResult};
use error_stack::{IntoReport, ResultExt};
use serde::{de::Visitor, Deserialize, Deserializer};
use utoipa::ToSchema;

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, ToSchema)]
pub struct Percentage<const PRECISION: u8> {
    // this value will range from 0 to 100, decimal length defined by precision macro
    /// Percentage value ranging between 0 and 100
    #[schema(example = 2.5)]
    percentage: f32,
}

fn get_invalid_percentage_error_message(precision: u8) -> String {
    format!(
        "value should be a string representation float between 0 to 100 and precise to only upto {} decimal digits",
        precision
    )
}

impl<const PRECISION: u8> Percentage<PRECISION> {
    pub fn from_string(value: String) -> CustomResult<Self, ApiModelsError> {
        if Self::is_valid_string_value(&value)? {
            Ok(Self {
                percentage: value
                    .parse()
                    .into_report()
                    .change_context(ApiModelsError::InvalidPercentageValue)?,
            })
        } else {
            Err(ApiModelsError::InvalidPercentageValue.into())
                .attach_printable(get_invalid_percentage_error_message(PRECISION))
        }
    }
    pub fn get_percentage(&self) -> f32 {
        self.percentage
    }
    fn is_valid_string_value(value: &str) -> CustomResult<bool, ApiModelsError> {
        let float_value = Self::is_valid_float_string(value)?;
        Ok(Self::is_valid_range(float_value) && Self::is_valid_precision_length(value))
    }
    fn is_valid_float_string(value: &str) -> CustomResult<f32, ApiModelsError> {
        value
            .parse()
            .into_report()
            .change_context(ApiModelsError::InvalidPercentageValue)
    }
    fn is_valid_range(value: f32) -> bool {
        (0.0..=100.0).contains(&value)
    }
    fn is_valid_precision_length(value: &str) -> bool {
        if value.contains('.') {
            // if string has '.' then take the decimal part and verify precision length
            value.split('.').last().is_some_and(|deciaml_part| {
                deciaml_part.trim_end_matches('0').len() <= PRECISION.into()
            })
        } else {
            // if there is no '.' then it is a whole number with no decimal part. So return true
            true
        }
    }
}

// custom serde deserialization function
struct PercentageVisitor<const PRECISION: u8> {}
impl<'de, const PRECISION: u8> Visitor<'de> for PercentageVisitor<PRECISION> {
    type Value = Percentage<PRECISION>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Percentage object")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut percentage_value = None;
        while let Some(key) = map.next_key::<String>()? {
            if key.eq("percentage") {
                if percentage_value.is_some() {
                    return Err(serde::de::Error::duplicate_field("percentage"));
                }
                percentage_value = Some(map.next_value::<String>()?);
            } else {
                // Ignore unknown fields
                let _: serde::de::IgnoredAny = map.next_value()?;
            }
        }
        if let Some(value) = percentage_value {
            Ok(Percentage::from_string(value.clone()).map_err(|_| {
                serde::de::Error::invalid_value(
                    serde::de::Unexpected::Other(&format!("percentage value \"{}\"", value)),
                    &&*get_invalid_percentage_error_message(PRECISION),
                )
            })?)
        } else {
            Err(serde::de::Error::missing_field("percentage"))
        }
    }
}

impl<'de, const PRECISION: u8> Deserialize<'de> for Percentage<PRECISION> {
    fn deserialize<D>(data: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        data.deserialize_map(PercentageVisitor::<PRECISION> {})
    }
}
