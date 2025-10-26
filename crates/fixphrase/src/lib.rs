//! FixPhrase implementation for converting between GPS coordinates and
//! memorable phrases.

mod wordlist;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::wordlist::WORDLIST;
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum FixPhraseError {
    #[error("Latitude must be between -90 and 90")]
    InvalidLatitude,
    #[error("Longitude must be between -180 and 180")]
    InvalidLongitude,
    #[error("Not enough words in phrase (need at least 2)")]
    NotEnoughWords,
    #[error("Invalid phrase format")]
    InvalidPhrase,
}

/// Main FixPhrase implementation
pub struct FixPhrase;

impl FixPhrase {
    /// Encode latitude/longitude coordinates into a 4-word phrase
    ///
    /// # Arguments
    /// * `latitude` - Between -90.0 and 90.0
    /// * `longitude` - Between -180.0 and 180.0
    ///
    /// # Example
    /// ```
    /// use fixphrase::FixPhrase;
    /// let phrase = FixPhrase::encode(42.3601, -71.0589).unwrap();
    /// ```
    pub fn encode(latitude: f64, longitude: f64) -> Result<String, FixPhraseError> {
        // Validate coordinates
        if !(-90.0..=90.0).contains(&latitude) {
            return Err(FixPhraseError::InvalidLatitude);
        }
        if !(-180.0..=180.0).contains(&longitude) {
            return Err(FixPhraseError::InvalidLongitude);
        }

        // Round to 4 decimal places (~10m accuracy)
        let lat = (latitude * 10000.0).round() as i32 + 90 * 10000;
        let lon = (longitude * 10000.0).round() as i32 + 180 * 10000;

        // Format as 7-digit strings
        let lat_str = format!("{:07}", lat);
        let lon_str = format!("{:07}", lon);

        // Split into coordinate chunks
        let lat1dec = lat_str[0..4].parse::<usize>().map_err(|_| FixPhraseError::InvalidPhrase)?;
        let lon1dec = lon_str[0..4].parse::<usize>().map_err(|_| FixPhraseError::InvalidPhrase)?;
        let latlon2dec = format!("{}{}", &lat_str[4..6], &lon_str[4..5])
            .parse::<usize>()
            .map_err(|_| FixPhraseError::InvalidPhrase)?;
        let latlon4dec = format!("{}{}", &lat_str[6..7], &lon_str[5..7])
            .parse::<usize>()
            .map_err(|_| FixPhraseError::InvalidPhrase)?;

        // Add offsets to ensure unique words
        let groups = [lat1dec, lon1dec + 2000, latlon2dec + 5610, latlon4dec + 6610];

        // Get words from wordlist
        let words: Vec<&str> = groups.iter().filter_map(|&i| WORDLIST.get(i)).copied().collect();

        if words.len() != 4 {
            return Err(FixPhraseError::InvalidPhrase);
        }

        Ok(words.join(" "))
    }

    /// Decode a phrase back into coordinates
    ///
    /// # Arguments
    /// * `phrase` - A string containing 2-4 words from the wordlist
    ///
    /// # Returns
    /// Tuple of (latitude, longitude, accuracy, canonical_phrase)
    ///
    /// # Example
    /// ```
    /// use fixphrase::FixPhrase;
    /// let phrase = FixPhrase::encode(42.3601, -71.0589).unwrap();
    /// let (lat, lon, acc, _) = FixPhrase::decode(&phrase).unwrap();
    /// ```
    pub fn decode(phrase: &str) -> Result<(f64, f64, f64, String), FixPhraseError> {
        let mut indexes = [-1; 4];
        let mut canonical_phrase = [""; 4];

        let words: Vec<&str> = phrase.split_whitespace().collect();

        if words.len() < 2 {
            return Err(FixPhraseError::NotEnoughWords);
        }

        for (_i, word) in words.iter().enumerate().take(4) {
            if let Some(pos) = WORDLIST.iter().position(|w| w.eq_ignore_ascii_case(word)) {
                if pos < 2000 {
                    indexes[0] = pos as i32;
                    canonical_phrase[0] = WORDLIST[pos];
                } else if pos < 5610 {
                    indexes[1] = (pos - 2000) as i32;
                    canonical_phrase[1] = WORDLIST[pos];
                } else if pos < 6610 {
                    indexes[2] = (pos - 5610) as i32;
                    canonical_phrase[2] = WORDLIST[pos];
                } else if pos < 7610 {
                    indexes[3] = (pos - 6610) as i32;
                    canonical_phrase[3] = WORDLIST[pos];
                }
            }
        }

        if indexes[0] == -1 || indexes[1] == -1 {
            return Err(FixPhraseError::InvalidPhrase);
        }

        // Reconstruct coordinates
        let mut divby = 10.0;
        let mut lat = format!("{:04}", indexes[0]);
        let mut lon = format!("{:04}", indexes[1]);

        if indexes[2] != -1 {
            divby = 100.0;
            let latlon2dec = format!("{:03}", indexes[2]);
            lat.push_str(&latlon2dec[0..1]);
            lon.push_str(&latlon2dec[2..3]);
        }

        if indexes[2] != -1 && indexes[3] != -1 {
            divby = 10000.0;
            let latlon4dec = format!("{:03}", indexes[3]);
            let latlon2dec = format!("{:03}", indexes[2]);
            lat.push_str(&format!("{}{}", &latlon2dec[1..2], &latlon4dec[0..1]));
            lon.push_str(&latlon4dec[1..3]);
        }

        let latitude =
            (lat.parse::<f64>().map_err(|_| FixPhraseError::InvalidPhrase)? / divby) - 90.0;
        let longitude =
            (lon.parse::<f64>().map_err(|_| FixPhraseError::InvalidPhrase)? / divby) - 180.0;

        let accuracy = match divby {
            10.0 => 0.1,
            100.0 => 0.01,
            _ => 0.0001,
        };

        Ok((latitude, longitude, accuracy, canonical_phrase.join(" ").trim().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let lat = 42.3601;
        let lon = -71.0589;

        let phrase = FixPhrase::encode(lat, lon).unwrap();
        let (decoded_lat, decoded_lon, accuracy, _) = FixPhrase::decode(&phrase).unwrap();

        assert!((decoded_lat - lat).abs() < accuracy);
        assert!((decoded_lon - lon).abs() < accuracy);
    }

    #[test]
    fn test_invalid_coords() {
        assert!(matches!(FixPhrase::encode(91.0, 0.0), Err(FixPhraseError::InvalidLatitude)));
        assert!(matches!(FixPhrase::encode(0.0, 181.0), Err(FixPhraseError::InvalidLongitude)));
    }

    #[test]
    fn test_invalid_phrase() {
        assert!(matches!(FixPhrase::decode(""), Err(FixPhraseError::NotEnoughWords)));
        assert!(matches!(
            FixPhrase::decode("invalid words here"),
            Err(FixPhraseError::InvalidPhrase)
        ));
    }

    #[test]
    fn test_correct_encode_decode() {
        let lat = 42.1409;
        let lon = -76.8518;

        assert_eq!(
            FixPhrase::encode(lat, lon).unwrap(),
            "corrode ground slacks washbasin".to_string()
        );

        let (decoded_lat, decoded_lon, accuracy, phrase) =
            FixPhrase::decode("corrode ground slacks washbasin").unwrap();

        assert!((decoded_lat - lat).abs() < accuracy);
        assert!((decoded_lon - lon).abs() < accuracy);
        assert_eq!(phrase, "corrode ground slacks washbasin");
    }
}
