use egui::{ColorImage, Context, TextureHandle, TextureOptions};

pub struct AppAssets {
    pub icon_gmail: TextureHandle,
    pub icon_outlook: TextureHandle,
    pub icon_yahoo: TextureHandle,
    pub icon_icloud: TextureHandle,
}

impl AppAssets {
    pub fn load(ctx: &Context) -> Self {
        Self {
            icon_gmail: load_image_from_bytes(
                ctx,
                "gmail",
                include_bytes!("../../assets/google.png"),
            ),
            icon_outlook: load_image_from_bytes(
                ctx,
                "outlook",
                include_bytes!("../../assets/MicrosoftOutlook.png"),
            ),
            icon_yahoo: load_image_from_bytes(
                ctx,
                "yahoo",
                include_bytes!("../../assets/yahoo.png"),
            ),
            icon_icloud: load_image_from_bytes(
                ctx,
                "icloud",
                include_bytes!("../../assets/icloud.png"),
            ),
        }
    }
}

fn load_image_from_bytes(ctx: &Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let image = image::load_from_memory(bytes)
        .expect("Failed to load image from memory")
        .to_rgba8();

    let size = [image.width() as _, image.height() as _];
    let pixels = image.as_flat_samples();

    let color_image = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

    ctx.load_texture(name, color_image, TextureOptions::LINEAR)
}
