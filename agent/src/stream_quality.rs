use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamQuality {
    Balanced,
    #[default]
    High,
    Native,
}

impl StreamQuality {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "balanced" => Ok(Self::Balanced),
            "high" => Ok(Self::High),
            "native" => Ok(Self::Native),
            _ => bail!("STREAM_QUALITY must be balanced, high, or native"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::High => "high",
            Self::Native => "native (up to 4K)",
        }
    }

    pub fn fps(self) -> u32 {
        match self {
            Self::Balanced => 15,
            Self::High | Self::Native => 30,
        }
    }

    pub fn bitrate(self) -> u32 {
        match self {
            Self::Balanced => 2_000_000,
            Self::High => 8_000_000,
            Self::Native => 20_000_000,
        }
    }

    pub fn dimensions(self, width: usize, height: usize) -> Result<(usize, usize)> {
        if width < 2 || height < 2 {
            bail!("display dimensions must be at least 2 pixels");
        }
        let (long, short) = match self {
            Self::Balanced => (1280.0, 720.0),
            Self::High => (1920.0, 1080.0),
            Self::Native => (3840.0, 2160.0),
        };
        let (max_width, max_height) = if width >= height {
            (long, short)
        } else {
            (short, long)
        };
        let scale = (max_width / width as f64)
            .min(max_height / height as f64)
            .min(1.0);
        let output = (
            ((width as f64 * scale) as usize) & !1,
            ((height as f64 * scale) as usize) & !1,
        );
        if output.0 < 2 || output.1 < 2 {
            bail!("display aspect ratio is unsupported");
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_native_1080p_and_portrait_in_high_mode() {
        assert_eq!(
            StreamQuality::High.dimensions(1920, 1080).unwrap(),
            (1920, 1080)
        );
        assert_eq!(
            StreamQuality::High.dimensions(1080, 1920).unwrap(),
            (1080, 1920)
        );
        assert_eq!(
            StreamQuality::Native.dimensions(3840, 2160).unwrap(),
            (3840, 2160)
        );
    }

    #[test]
    fn scales_large_screens_without_upscaling_or_odd_encoder_dimensions() {
        assert_eq!(
            StreamQuality::High.dimensions(3840, 2160).unwrap(),
            (1920, 1080)
        );
        assert_eq!(
            StreamQuality::High.dimensions(1025, 769).unwrap(),
            (1024, 768)
        );
        assert_eq!(
            StreamQuality::High.dimensions(3440, 1440).unwrap(),
            (1920, 802)
        );
        assert_eq!(
            StreamQuality::Native.dimensions(7680, 4320).unwrap(),
            (3840, 2160)
        );
        assert!(StreamQuality::High.dimensions(0, 1080).is_err());
        assert!(StreamQuality::High.dimensions(100_000, 2).is_err());
        assert!(StreamQuality::parse("auto").is_err());
    }
}
