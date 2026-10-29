/// External tools Monax drives as sidecar processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Qpdf,
    Tesseract,
    /// WordCraft's `wordcraft-cli`: Office documents to PDF; bundled.
    Wordcraft,
}

impl Tool {
    /// Executable file name.
    pub fn bin(self) -> &'static str {
        match self {
            Tool::Qpdf => "qpdf",
            Tool::Tesseract => "tesseract",
            Tool::Wordcraft => "wordcraft-cli",
        }
    }

    /// Variables of this process the tool must never inherit: WordCraft
    /// reads a document password from `WORDCRAFT_PASSWORD`.
    pub(crate) fn env_removed(self) -> &'static [&'static str] {
        match self {
            Tool::Wordcraft => &["WORDCRAFT_PASSWORD"],
            _ => &[],
        }
    }

    /// Whether `PATH` is searched after `MONAX_BIN_DIR` and the bundle.
    pub(crate) fn on_path(self) -> bool {
        self != Tool::Wordcraft
    }
}

#[cfg(test)]
mod tests {
    use super::Tool;

    #[test]
    fn bin_names() {
        assert_eq!(Tool::Qpdf.bin(), "qpdf");
        assert_eq!(Tool::Tesseract.bin(), "tesseract");
        assert_eq!(Tool::Wordcraft.bin(), "wordcraft-cli");
    }

    #[test]
    fn only_the_dev_tools_are_looked_up_on_path() {
        assert!(Tool::Qpdf.on_path() && Tool::Tesseract.on_path());
        assert!(!Tool::Wordcraft.on_path());
    }
}
