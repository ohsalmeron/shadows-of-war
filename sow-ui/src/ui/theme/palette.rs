use egui::Color32;

    #[inline]
    pub fn backdrop() -> Color32 {
        Color32::from_black_alpha(120)
    } // Glassmorphism backdrop
    #[inline]
    pub fn surface() -> Color32 {
        Color32::from_rgba_unmultiplied(12, 12, 14, 240)
    } // Translucent glass base
    #[inline]
    pub fn surface_transparent() -> Color32 {
        Color32::from_rgba_unmultiplied(12, 12, 14, 50)
    }

    #[inline]
    pub fn neon_cyan() -> Color32 {
        Color32::from_rgb(6, 182, 212)
    } // var(--cosmic-cyan)
    #[inline]
    pub fn neon_cyan_hover() -> Color32 {
        Color32::from_rgb(34, 211, 238)
    } // Brighter cyan
    #[inline]
    pub fn neon_cyan_glow() -> Color32 {
        Color32::from_rgba_unmultiplied(6, 182, 212, 80)
    } // Electric blue glow

    #[inline]
    pub fn neon_gold() -> Color32 {
        Color32::from_rgb(234, 179, 8)
    } // var(--cosmic-yellow)
    #[inline]
    pub fn neon_gold_hover() -> Color32 {
        Color32::from_rgb(250, 204, 21)
    } // Brighter yellow

    #[inline]
    pub fn button_inactive() -> Color32 {
        Color32::from_rgba_unmultiplied(22, 22, 24, 120)
    }
    #[inline]
    pub fn button_hovered() -> Color32 {
        Color32::from_rgba_unmultiplied(40, 40, 44, 170)
    }

    #[inline]
    pub fn field_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(8, 8, 10, 150)
    } // Deep inset glass
    #[inline]
    pub fn field_border() -> Color32 {
        Color32::from_rgba_unmultiplied(75, 85, 99, 80)
    } // Subtle slate border

    #[inline]
    pub fn danger() -> Color32 {
        Color32::from_rgb(239, 68, 68)
    } // var(--cosmic-red)
    #[inline]
    pub fn danger_border() -> Color32 {
        Color32::from_rgb(220, 38, 38)
    }

    #[inline]
    pub fn pink() -> Color32 {
        Color32::from_rgb(236, 72, 153)
    } // var(--cosmic-pink)

    #[inline]
    pub fn text_normal() -> Color32 {
        Color32::from_rgb(243, 244, 246)
    }
    #[inline]
    pub fn text_muted() -> Color32 {
        Color32::from_rgb(156, 163, 175)
    } // var(--cosmic-gray)
