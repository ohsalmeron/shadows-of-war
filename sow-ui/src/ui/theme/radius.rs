use egui::CornerRadius;

    pub const XS: u8 = 4;
    pub const SM: u8 = 6;
    pub const MD: u8 = 8;
    pub const LG: u8 = 12;

    #[inline]
    pub fn xs() -> CornerRadius {
        CornerRadius::same(XS)
    }
    #[inline]
    pub fn sm() -> CornerRadius {
        CornerRadius::same(SM)
    }
    #[inline]
    pub fn md() -> CornerRadius {
        CornerRadius::same(MD)
    }
    #[inline]
    pub fn lg() -> CornerRadius {
        CornerRadius::same(LG)
    }
    #[inline]
    pub fn tab_top() -> CornerRadius {
        CornerRadius {
            nw: SM,
            ne: SM,
            sw: 0,
            se: 0,
        }
    }
    /// Bottom HUD docked to screen edge on portrait viewports — rounded top only.
    #[inline]
    pub fn dock_top() -> CornerRadius {
        CornerRadius {
            nw: LG,
            ne: LG,
            sw: 0,
            se: 0,
        }
    }
    #[inline]
    pub fn content_bottom() -> CornerRadius {
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: LG,
            se: LG,
        }
    }
