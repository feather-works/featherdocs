//! APP-005: verifica inserção não destrutiva e prévia RGBA comum do Docs.
//! cargo run --manifest-path featherdocs/crates/featherdocs-pelicano/Cargo.toml --example validacao_imagem_docs -j 2 -- /tmp/docs-imagem
use featherdocs_pelicano::imagem_compartilhada::{preparar_insercao, preparar_previa, area_desenho};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::{env, error::Error, fs, io::Cursor, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let pasta = PathBuf::from(env::args().nth(1).unwrap_or_else(|| "/tmp/docs-imagem".into()));
    fs::create_dir_all(&pasta)?;
    let imagem = RgbaImage::from_fn(128, 96, |x, y| Rgba([
        (x * 2).min(255) as u8,
        (y * 2).min(255) as u8,
        90,
        if x < 64 { 100 } else { 255 },
    ]));
    let mut bytes = Vec::new();
    DynamicImage::ImageRgba8(imagem).write_to(
        &mut Cursor::new(&mut bytes), ImageFormat::Png,
    )?;
    let (url, largura, altura) = preparar_insercao(&bytes)?;
    let (_, originais) = featherdocs_io::base64::de_data_url(&url)
        .ok_or("data URL de inserção inválida")?;
    assert_eq!(originais, bytes, "inserção deve preservar os bytes originais");
    let (w, h, rgba) = preparar_previa(&url)
        .ok_or("prévia não pôde ser construída")?;
    let previa = RgbaImage::from_raw(w, h, rgba)
        .ok_or("pixels RGBA inválidos")?;
    let area = area_desenho([20.0, 30.0, largura, altura])
        .ok_or("geometria inválida")?;
    let destino_original = pasta.join("imagem_original.png");
    let destino_previa = pasta.join("imagem_previa.png");
    fs::write(&destino_original, &bytes)?;
    previa.save(&destino_previa)?;
    println!("Original intacto: {}", destino_original.display());
    println!("Prévia Image: {} ({}x{})", destino_previa.display(), w, h);
    println!("Área Drawing: {}x{} pontos", area.largura, area.altura);
    Ok(())
}
