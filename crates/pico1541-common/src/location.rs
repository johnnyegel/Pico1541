use heapless::String;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DriveLocation {
    /// Top-level root container presenting "INTERNAL" and "SD CARD".
    #[default]
    Root,

    /// Virtual directory listing within either Internal flash (/files) or SD card.
    VirtualDirectory {
        path: String<128>,
        use_internal: bool,
    },

    /// A mounted disk image file (.D64, .G64, .NIB).
    DiskImage {
        path: String<128>,
        use_internal: bool,
    },
}

impl DriveLocation {
    pub fn is_root(&self) -> bool {
        matches!(self, Self::Root)
    }

    pub fn use_internal(&self) -> bool {
        match self {
            Self::Root => false,
            Self::VirtualDirectory { use_internal, .. }
            | Self::DiskImage { use_internal, .. } => *use_internal,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::Root => "/",
            Self::VirtualDirectory { path, .. }
            | Self::DiskImage { path, .. } => path.as_str(),
        }
    }
}
